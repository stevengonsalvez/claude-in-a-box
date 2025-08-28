// ABOUTME: WebSocket client for connecting to container PTY service
// Manages WebSocket connection lifecycle, message handling, and reconnection

use crate::terminal::protocol::{ConnectionState, ConnectionStatus, Message};
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite};
use tracing::{debug, error, info, warn};

pub struct WebSocketTerminalClient {
    /// WebSocket URL for the container PTY service
    url: String,
    
    /// Current connection status
    status: Arc<RwLock<ConnectionStatus>>,
    
    /// Channel to send messages to the WebSocket
    tx_sender: mpsc::UnboundedSender<Message>,
    tx_receiver: Arc<Mutex<mpsc::UnboundedReceiver<Message>>>,
    
    /// Channel to receive messages from the WebSocket
    rx_sender: mpsc::UnboundedSender<Message>,
    rx_receiver: Arc<Mutex<mpsc::UnboundedReceiver<Message>>>,
    
    /// Reconnection configuration
    reconnect_interval: Duration,
    max_reconnect_attempts: u32,
    
    /// Task handle for the connection loop
    connection_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    
    /// Heartbeat configuration
    heartbeat_interval: Duration,
    heartbeat_timeout: Duration,
    last_heartbeat: Arc<Mutex<Instant>>,
}

impl WebSocketTerminalClient {
    /// Create a new WebSocket terminal client
    pub fn new(container_id: &str, port: u16) -> Self {
        let url = format!("ws://{}:{}/pty", container_id, port);
        
        let (tx_sender, tx_receiver) = mpsc::unbounded_channel();
        let (rx_sender, rx_receiver) = mpsc::unbounded_channel();
        
        Self {
            url,
            status: Arc::new(RwLock::new(ConnectionStatus {
                state: ConnectionState::Disconnected,
                session_id: None,
                last_error: None,
                reconnect_attempts: 0,
            })),
            tx_sender,
            tx_receiver: Arc::new(Mutex::new(tx_receiver)),
            rx_sender,
            rx_receiver: Arc::new(Mutex::new(rx_receiver)),
            reconnect_interval: Duration::from_secs(2),
            max_reconnect_attempts: 10,
            connection_handle: Arc::new(Mutex::new(None)),
            heartbeat_interval: Duration::from_secs(30),
            heartbeat_timeout: Duration::from_secs(60),
            last_heartbeat: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Connect to the WebSocket server
    pub async fn connect(&self) -> Result<()> {
        info!("Starting WebSocket connection to {}", self.url);
        debug!("WebSocket URL: {}", self.url);
        
        // Update status
        {
            let mut status = self.status.write().await;
            status.state = ConnectionState::Connecting;
            status.reconnect_attempts = 0;
            info!("Set connection state to Connecting");
        }
        
        // Start connection loop
        info!("Spawning connection loop task");
        let handle = self.spawn_connection_loop().await;
        
        // Store the handle
        {
            let mut connection_handle = self.connection_handle.lock().await;
            *connection_handle = Some(handle);
            info!("Connection loop task spawned successfully");
        }
        
        // Wait for initial connection
        info!("Waiting for initial connection (timeout: 5 seconds)");
        let timeout = tokio::time::timeout(Duration::from_secs(5), async {
            let mut check_count = 0;
            loop {
                check_count += 1;
                let status = self.status.read().await;
                let current_state = status.state.clone();
                let last_error = status.last_error.clone();
                drop(status);
                
                if check_count % 10 == 0 {
                    debug!("Connection check #{}: state = {:?}", check_count, current_state);
                }
                
                if current_state == ConnectionState::Connected {
                    info!("WebSocket connection established successfully!");
                    return Ok(());
                }
                if current_state == ConnectionState::Error {
                    error!("Connection failed with error: {:?}", last_error);
                    return Err(anyhow!("Connection failed: {:?}", last_error));
                }
                
                sleep(Duration::from_millis(100)).await;
            }
        });
        
        match timeout.await {
            Ok(result) => result,
            Err(_) => {
                error!("WebSocket connection timeout after 5 seconds");
                Err(anyhow!("Connection timeout"))
            }
        }
    }

    /// Spawn the connection loop that handles WebSocket lifecycle
    async fn spawn_connection_loop(&self) -> tokio::task::JoinHandle<()> {
        let url = self.url.clone();
        let status = self.status.clone();
        let tx_receiver = self.tx_receiver.clone();
        let rx_sender = self.rx_sender.clone();
        let reconnect_interval = self.reconnect_interval;
        let max_reconnect_attempts = self.max_reconnect_attempts;
        let heartbeat_interval = self.heartbeat_interval;
        let last_heartbeat = self.last_heartbeat.clone();
        
        tokio::spawn(async move {
            loop {
                match Self::connection_handler(
                    &url,
                    status.clone(),
                    tx_receiver.clone(),
                    rx_sender.clone(),
                    heartbeat_interval,
                    last_heartbeat.clone(),
                ).await {
                    Ok(_) => {
                        info!("WebSocket connection closed normally");
                    }
                    Err(e) => {
                        error!("WebSocket connection error: {}", e);
                        
                        // Update error status
                        {
                            let mut status_guard = status.write().await;
                            status_guard.state = ConnectionState::Error;
                            status_guard.last_error = Some(e.to_string());
                        }
                    }
                }
                
                // Check if we should reconnect
                let should_reconnect = {
                    let mut status_guard = status.write().await;
                    if status_guard.reconnect_attempts >= max_reconnect_attempts {
                        warn!("Max reconnection attempts reached");
                        status_guard.state = ConnectionState::Disconnected;
                        false
                    } else {
                        status_guard.reconnect_attempts += 1;
                        status_guard.state = ConnectionState::Connecting;
                        true
                    }
                };
                
                if !should_reconnect {
                    break;
                }
                
                // Wait before reconnecting
                sleep(reconnect_interval).await;
                info!("Attempting to reconnect...");
            }
        })
    }

    /// Handle a single WebSocket connection
    async fn connection_handler(
        url: &str,
        status: Arc<RwLock<ConnectionStatus>>,
        tx_receiver: Arc<Mutex<mpsc::UnboundedReceiver<Message>>>,
        rx_sender: mpsc::UnboundedSender<Message>,
        heartbeat_interval: Duration,
        last_heartbeat: Arc<Mutex<Instant>>,
    ) -> Result<()> {
        info!("Connection handler starting for URL: {}", url);
        debug!("Attempting WebSocket handshake...");
        
        // Connect to WebSocket
        let ws_result = connect_async(url).await;
        
        match &ws_result {
            Ok(_) => info!("WebSocket handshake successful"),
            Err(e) => {
                error!("WebSocket handshake failed: {}", e);
                // Try to extract more details about the error
                if e.to_string().contains("refused") {
                    error!("Connection refused - is the PTY service running on the target?");
                } else if e.to_string().contains("timeout") {
                    error!("Connection timeout - is the port accessible?");
                } else if e.to_string().contains("lookup") {
                    error!("DNS/hostname lookup failed - check the URL: {}", url);
                }
            }
        }
        
        let (ws_stream, response) = ws_result
            .map_err(|e| anyhow!("Failed to connect: {}", e))?;
        
        info!("WebSocket connected successfully to {}", url);
        debug!("WebSocket response status: {:?}", response.status());
        
        // Update status
        {
            let mut status_guard = status.write().await;
            status_guard.state = ConnectionState::Connected;
            status_guard.last_error = None;
            info!("Updated connection state to Connected");
        }
        
        // Split the WebSocket stream
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        // Create channel for outgoing messages (including heartbeats)
        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<tungstenite::Message>();
        
        // Spawn task to handle outgoing messages
        let send_task = tokio::spawn(async move {
            while let Some(msg) = outgoing_rx.recv().await {
                if let Err(e) = ws_sender.send(msg).await {
                    error!("Failed to send WebSocket message: {}", e);
                    break;
                }
            }
        });
        
        // Spawn heartbeat task
        let heartbeat_tx = outgoing_tx.clone();
        let heartbeat_handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(heartbeat_interval);
            ticker.tick().await; // Skip first immediate tick
            
            loop {
                ticker.tick().await;
                
                // Send heartbeat
                let heartbeat_msg = Message::heartbeat();
                if let Ok(json) = serde_json::to_string(&heartbeat_msg) {
                    if heartbeat_tx.send(tungstenite::Message::Text(json)).is_err() {
                        break;
                    }
                    debug!("Heartbeat sent");
                }
            }
        });
        
        // Handle incoming and outgoing messages
        let mut tx_guard = tx_receiver.lock().await;
        
        loop {
            tokio::select! {
                // Handle outgoing messages
                Some(msg) = tx_guard.recv() => {
                    let json = serde_json::to_string(&msg)?;
                    if outgoing_tx.send(tungstenite::Message::Text(json)).is_err() {
                        error!("Failed to queue outgoing message");
                        break;
                    }
                    
                    debug!("Sent message: {:?}", msg);
                }
                
                // Handle incoming messages
                Some(ws_msg) = ws_receiver.next() => {
                    match ws_msg {
                        Ok(tungstenite::Message::Text(text)) => {
                            match serde_json::from_str::<Message>(&text) {
                                Ok(msg) => {
                                    debug!("Received message: {:?}", msg);
                                    
                                    // Update last heartbeat on any message
                                    *last_heartbeat.lock().await = Instant::now();
                                    
                                    // Handle special messages
                                    match &msg {
                                        Message::SessionInit(init) => {
                                            let mut status_guard = status.write().await;
                                            status_guard.session_id = Some(init.session_id.clone());
                                            info!("Session initialized: {}", init.session_id);
                                        }
                                        Message::HeartbeatResponse(_) => {
                                            debug!("Heartbeat acknowledged");
                                        }
                                        _ => {}
                                    }
                                    
                                    // Forward to receiver channel
                                    if let Err(e) = rx_sender.send(msg) {
                                        warn!("Failed to forward message: {}", e);
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse message: {}", e);
                                }
                            }
                        }
                        Ok(tungstenite::Message::Close(_)) => {
                            info!("WebSocket closed by server");
                            break;
                        }
                        Ok(_) => {
                            // Ignore other message types (Binary, Ping, Pong)
                        }
                        Err(e) => {
                            error!("WebSocket error: {}", e);
                            return Err(anyhow!("WebSocket error: {}", e));
                        }
                    }
                }
                
                // Both channels closed
                else => {
                    info!("Channels closed, ending connection");
                    break;
                }
            }
        }
        
        // Clean up tasks
        heartbeat_handle.abort();
        send_task.abort();
        
        // Update status
        {
            let mut status_guard = status.write().await;
            status_guard.state = ConnectionState::Disconnected;
        }
        
        Ok(())
    }


    /// Send a message to the PTY
    pub async fn send(&self, message: Message) -> Result<()> {
        // Check connection status
        {
            let status = self.status.read().await;
            if status.state != ConnectionState::Connected {
                return Err(anyhow!("Not connected"));
            }
        }
        
        self.tx_sender.send(message)
            .map_err(|e| anyhow!("Failed to send message: {}", e))
    }

    /// Receive messages from the PTY
    pub async fn receive(&self) -> Option<Message> {
        let mut rx = self.rx_receiver.lock().await;
        rx.recv().await
    }

    /// Send input to the PTY
    pub async fn send_input(&self, data: String) -> Result<()> {
        self.send(Message::input(data)).await
    }

    /// Resize the terminal
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.send(Message::resize(cols, rows)).await
    }

    /// Send a permission response
    pub async fn send_permission_response(&self, response: String) -> Result<()> {
        self.send(Message::permission_response(response)).await
    }

    /// Reset the session
    pub async fn reset_session(&self) -> Result<()> {
        self.send(Message::reset()).await
    }

    /// Get connection status
    pub async fn get_status(&self) -> ConnectionStatus {
        self.status.read().await.clone()
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        self.status.read().await.state == ConnectionState::Connected
    }

    /// Disconnect from the WebSocket server
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting WebSocket client");
        
        // Update status to prevent reconnection
        {
            let mut status = self.status.write().await;
            status.state = ConnectionState::Disconnected;
            status.reconnect_attempts = self.max_reconnect_attempts;
        }
        
        // Cancel the connection task
        if let Some(handle) = self.connection_handle.lock().await.take() {
            handle.abort();
        }
        
        Ok(())
    }
}