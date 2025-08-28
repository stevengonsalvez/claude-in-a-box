// ABOUTME: Interactive terminal component combining WebSocket client and terminal emulator
// Provides full terminal interaction with expand/collapse functionality

use crate::terminal::{
    protocol::{ConnectionState, Message, OutputMessage, ParsedOutput},
    terminal_emulator::TerminalEmulatorWidget,
    websocket_client::WebSocketTerminalClient,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// View modes for the terminal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Normal mode - terminal in right panel
    Normal,
    /// Expanded mode - fullscreen terminal
    Expanded,
    /// Minimized mode - status bar only
    Minimized,
}

/// Interactive terminal component
pub struct InteractiveTerminalComponent {
    /// WebSocket client for PTY communication
    ws_client: Arc<WebSocketTerminalClient>,

    /// Terminal emulator widget
    terminal: Arc<Mutex<TerminalEmulatorWidget>>,

    /// Current view mode
    pub view_mode: ViewMode,

    /// Input buffer for typing
    input_buffer: String,
    input_cursor: usize,

    /// Session information
    session_id: Uuid,
    session_name: String,
    container_id: String,

    /// Connection status
    connected: bool,
    connection_error: Option<String>,

    /// Permission prompt state
    awaiting_permission: bool,
    permission_options: Vec<String>,

    /// Message receiver channel
    message_receiver: Arc<Mutex<mpsc::UnboundedReceiver<Message>>>,

    /// Focus state
    is_focused: bool,

    /// Terminal dimensions
    terminal_cols: u16,
    terminal_rows: u16,
}

impl InteractiveTerminalComponent {
    /// Create a new interactive terminal component
    pub async fn new(
        session_id: Uuid,
        session_name: String,
        container_id: String,
        port: u16,
    ) -> Result<Self> {
        info!("Creating interactive terminal for session {}", session_id);

        // Create WebSocket client using container ID as hostname
        let ws_client = Arc::new(WebSocketTerminalClient::new(&container_id, port));

        Self::create_with_client(session_id, session_name, container_id, ws_client).await
    }

    /// Create a new interactive terminal component with a specific host port
    pub async fn new_with_host_port(
        session_id: Uuid,
        session_name: String,
        container_id: String,
        host_port: u16,
    ) -> Result<Self> {
        info!(
            "Creating interactive terminal for session {} with host port {}",
            session_id, host_port
        );

        // Create WebSocket client using localhost and mapped port
        let ws_client = Arc::new(WebSocketTerminalClient::new("localhost", host_port));

        Self::create_with_client(session_id, session_name, container_id, ws_client).await
    }

    /// Internal method to create component with a WebSocket client
    async fn create_with_client(
        session_id: Uuid,
        session_name: String,
        container_id: String,
        ws_client: Arc<WebSocketTerminalClient>,
    ) -> Result<Self> {
        // Create terminal emulator (default size, will be resized)
        let terminal = Arc::new(Mutex::new(TerminalEmulatorWidget::new(120, 40)));

        // Create message receiver channel
        let (msg_sender, msg_receiver) = mpsc::unbounded_channel();

        let component = Self {
            ws_client: ws_client.clone(),
            terminal: terminal.clone(),
            view_mode: ViewMode::Normal,
            input_buffer: String::new(),
            input_cursor: 0,
            session_id,
            session_name,
            container_id,
            connected: false,
            connection_error: None,
            awaiting_permission: false,
            permission_options: Vec::new(),
            message_receiver: Arc::new(Mutex::new(msg_receiver)),
            is_focused: false,
            terminal_cols: 120,
            terminal_rows: 40,
        };

        // Spawn message handler task
        let ws_client_clone = ws_client.clone();
        let terminal_clone = terminal.clone();
        let msg_sender_clone = msg_sender.clone();

        tokio::spawn(async move {
            Self::message_handler_loop(ws_client_clone, terminal_clone, msg_sender_clone).await;
        });

        Ok(component)
    }

    /// Connect to the container PTY service
    pub async fn connect(&mut self) -> Result<()> {
        info!("InteractiveTerminalComponent: Starting connection to PTY service");
        info!(
            "Session: {}, Container: {}",
            self.session_name, self.container_id
        );

        match self.ws_client.connect().await {
            Ok(_) => {
                info!("WebSocket client connected successfully");
                self.connected = true;
                self.connection_error = None;

                // Send initial resize
                info!(
                    "Sending initial terminal resize: {}x{}",
                    self.terminal_cols, self.terminal_rows
                );
                match self.ws_client.resize(self.terminal_cols, self.terminal_rows).await {
                    Ok(_) => info!("Terminal resize sent successfully"),
                    Err(e) => warn!("Failed to send initial resize: {}", e),
                }

                info!(
                    "PTY connection fully established for session {}",
                    self.session_name
                );
                Ok(())
            }
            Err(e) => {
                self.connected = false;
                self.connection_error = Some(e.to_string());
                error!(
                    "Failed to connect to PTY service for session {}: {}",
                    self.session_name, e
                );
                Err(e)
            }
        }
    }

    /// Message handler loop
    async fn message_handler_loop(
        ws_client: Arc<WebSocketTerminalClient>,
        terminal: Arc<Mutex<TerminalEmulatorWidget>>,
        msg_sender: mpsc::UnboundedSender<Message>,
    ) {
        loop {
            if let Some(msg) = ws_client.receive().await {
                // Forward message to component
                let _ = msg_sender.send(msg.clone());

                // Process specific message types
                match msg {
                    Message::Output(output) => {
                        let mut term = terminal.lock().await;
                        term.process_output(&output.data);
                    }
                    Message::SessionInit(init) => {
                        info!("Session initialized: {}", init.session_id);

                        // Process buffered output
                        let mut term = terminal.lock().await;
                        for output_msg in init.buffer {
                            term.process_output(&output_msg.data);
                        }
                    }
                    Message::SessionEnded(_) => {
                        warn!("Session ended");
                        break;
                    }
                    Message::Error(err) => {
                        error!("PTY error: {}", err.error);
                    }
                    _ => {}
                }
            } else {
                // Connection closed
                warn!("WebSocket connection closed");
                break;
            }
        }
    }

    /// Handle keyboard input
    pub async fn handle_input(&mut self, key: KeyEvent) -> Result<bool> {
        debug!("InteractiveTerminal received key: {:?}", key);

        if !self.connected {
            debug!("Not connected, ignoring input");
            return Ok(false);
        }

        // Handle special key combinations
        match (key.code, key.modifiers) {
            // Expand/collapse terminal (x key)
            (KeyCode::Char('x'), KeyModifiers::NONE) if !self.awaiting_permission => {
                info!("Toggling terminal view mode");
                self.toggle_view_mode();
                return Ok(true);
            }

            // Scroll controls
            (KeyCode::PageUp, _) => {
                let mut term = self.terminal.lock().await;
                term.scroll_up(10);
                return Ok(true);
            }
            (KeyCode::PageDown, _) => {
                let mut term = self.terminal.lock().await;
                term.scroll_down(10);
                return Ok(true);
            }
            (KeyCode::Home, KeyModifiers::CONTROL) => {
                let mut term = self.terminal.lock().await;
                term.scroll_to_top();
                return Ok(true);
            }
            (KeyCode::End, KeyModifiers::CONTROL) => {
                let mut term = self.terminal.lock().await;
                term.scroll_to_bottom();
                return Ok(true);
            }

            // Clear terminal
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                let mut term = self.terminal.lock().await;
                term.clear();
                return Ok(true);
            }

            _ => {}
        }

        // Handle permission prompt input
        if self.awaiting_permission {
            return self.handle_permission_input(key).await;
        }

        // Forward input to PTY in expanded mode
        if self.view_mode == ViewMode::Expanded {
            return self.forward_input_to_pty(key).await;
        }

        Ok(false)
    }

    /// Handle permission prompt input
    async fn handle_permission_input(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let option = c.to_string();
                self.ws_client.send_permission_response(option).await?;
                self.awaiting_permission = false;
                self.permission_options.clear();
                Ok(true)
            }
            KeyCode::Esc => {
                // Cancel permission prompt
                self.awaiting_permission = false;
                self.permission_options.clear();
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Forward keyboard input directly to PTY
    async fn forward_input_to_pty(&mut self, key: KeyEvent) -> Result<bool> {
        let data = match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Send control character
                    let ctrl_char = (c as u8) & 0x1f;
                    vec![ctrl_char]
                } else if key.modifiers.contains(KeyModifiers::ALT) {
                    // Send Alt+key sequence
                    let mut data = vec![0x1b]; // ESC
                    data.push(c as u8);
                    data
                } else {
                    // Regular character
                    c.to_string().into_bytes()
                }
            }
            KeyCode::Enter => vec![b'\r'],
            KeyCode::Tab => vec![b'\t'],
            KeyCode::Backspace => vec![0x7f],
            KeyCode::Esc => vec![0x1b],
            KeyCode::Up => vec![0x1b, b'[', b'A'],
            KeyCode::Down => vec![0x1b, b'[', b'B'],
            KeyCode::Right => vec![0x1b, b'[', b'C'],
            KeyCode::Left => vec![0x1b, b'[', b'D'],
            KeyCode::Home => vec![0x1b, b'[', b'H'],
            KeyCode::End => vec![0x1b, b'[', b'F'],
            KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],
            KeyCode::Insert => vec![0x1b, b'[', b'2', b'~'],
            KeyCode::F(n) => {
                // F1-F12 keys
                match n {
                    1 => vec![0x1b, b'O', b'P'],
                    2 => vec![0x1b, b'O', b'Q'],
                    3 => vec![0x1b, b'O', b'R'],
                    4 => vec![0x1b, b'O', b'S'],
                    5 => vec![0x1b, b'[', b'1', b'5', b'~'],
                    6 => vec![0x1b, b'[', b'1', b'7', b'~'],
                    7 => vec![0x1b, b'[', b'1', b'8', b'~'],
                    8 => vec![0x1b, b'[', b'1', b'9', b'~'],
                    9 => vec![0x1b, b'[', b'2', b'0', b'~'],
                    10 => vec![0x1b, b'[', b'2', b'1', b'~'],
                    11 => vec![0x1b, b'[', b'2', b'3', b'~'],
                    12 => vec![0x1b, b'[', b'2', b'4', b'~'],
                    _ => return Ok(false),
                }
            }
            _ => return Ok(false),
        };

        // Send to PTY
        self.ws_client.send_input(String::from_utf8_lossy(&data).to_string()).await?;
        Ok(true)
    }

    /// Toggle view mode
    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Normal => ViewMode::Expanded,
            ViewMode::Expanded => ViewMode::Normal,
            ViewMode::Minimized => ViewMode::Normal,
        };

        debug!("Toggled view mode to {:?}", self.view_mode);
    }

    /// Set view mode
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.is_focused = focused;
    }

    /// Resize terminal
    pub async fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.terminal_cols = cols;
        self.terminal_rows = rows;

        // Update terminal emulator
        let mut term = self.terminal.lock().await;
        term.resize(cols, rows);

        // Send resize to PTY if connected
        if self.connected {
            self.ws_client.resize(cols, rows).await?;
        }

        Ok(())
    }

    /// Send input to the PTY
    pub async fn send_input(&self, data: &str) -> Result<()> {
        if !self.connected {
            return Err(anyhow::anyhow!("Not connected to PTY service"));
        }
        self.ws_client.send_input(data.to_string()).await
    }

    /// Process pending messages
    pub async fn process_messages(&mut self) {
        let mut receiver = self.message_receiver.lock().await;

        while let Ok(msg) = receiver.try_recv() {
            match msg {
                Message::PermissionRequired(perm) => {
                    self.awaiting_permission = true;
                    self.permission_options = perm.options;
                    debug!("Permission required: {}", perm.question);
                }
                Message::SessionEnded(_) => {
                    self.connected = false;
                    warn!("Session ended");
                }
                Message::Error(err) => {
                    self.connection_error = Some(err.error);
                }
                _ => {}
            }
        }
    }

    /// Render the component
    pub async fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        // Process any pending messages
        self.process_messages().await;

        match self.view_mode {
            ViewMode::Normal => self.render_normal(frame, area).await,
            ViewMode::Expanded => self.render_expanded(frame, area).await,
            ViewMode::Minimized => self.render_minimized(frame, area),
        }
    }

    /// Render normal view (terminal in panel)
    async fn render_normal(&mut self, frame: &mut Frame<'_>, area: Rect) {
        // Adjust size if needed
        let inner = Block::default().borders(Borders::ALL).inner(area);
        if inner.width != self.terminal_cols || inner.height != self.terminal_rows {
            let _ = self.resize(inner.width, inner.height).await;
        }

        // Update and render terminal
        {
            let mut term = self.terminal.lock().await;

            // Update terminal properties
            term.set_title(format!("📺 {} - Interactive Terminal", self.session_name));
            term.set_focused(self.is_focused);

            // Render terminal
            let term_widget = std::mem::replace(&mut *term, TerminalEmulatorWidget::new(120, 40));
            frame.render_widget(term_widget, area);

            // Restore terminal (ugly but necessary due to Widget consuming self)
            *term = TerminalEmulatorWidget::new(self.terminal_cols, self.terminal_rows);
        }

        // Render permission prompt if needed
        if self.awaiting_permission {
            self.render_permission_prompt(frame, area);
        }

        // Render connection status if not connected
        if !self.connected {
            self.render_connection_status(frame, area);
        }
    }

    /// Render expanded view (fullscreen terminal)
    async fn render_expanded(&mut self, frame: &mut Frame<'_>, area: Rect) {
        // Clear the area first for fullscreen effect
        frame.render_widget(Clear, area);

        // Use full area
        let inner = Block::default().borders(Borders::ALL).inner(area);
        if inner.width != self.terminal_cols || inner.height != self.terminal_rows {
            let _ = self.resize(inner.width, inner.height).await;
        }

        // Update and render terminal
        {
            let mut term = self.terminal.lock().await;

            // Update terminal properties
            term.set_title(format!(
                "📺 {} - Interactive Terminal (Expanded - Press 'x' to minimize)",
                self.session_name
            ));
            term.set_focused(true);

            // Render terminal
            let term_widget = std::mem::replace(&mut *term, TerminalEmulatorWidget::new(120, 40));
            frame.render_widget(term_widget, area);

            // Restore terminal
            *term = TerminalEmulatorWidget::new(self.terminal_cols, self.terminal_rows);
        }

        // Render permission prompt if needed
        if self.awaiting_permission {
            self.render_permission_prompt(frame, area);
        }
    }

    /// Render minimized view (status bar only)
    fn render_minimized(&self, frame: &mut Frame<'_>, area: Rect) {
        let status = if self.connected {
            format!("📺 {} - Connected (Press 'x' to expand)", self.session_name)
        } else {
            format!("📺 {} - Disconnected", self.session_name)
        };

        let block = Block::default().borders(Borders::ALL).border_style(Style::default().fg(
            if self.is_focused {
                Color::Cyan
            } else {
                Color::Gray
            },
        ));

        let paragraph =
            Paragraph::new(status).block(block).style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, area);
    }

    /// Render permission prompt overlay
    fn render_permission_prompt(&self, frame: &mut Frame<'_>, area: Rect) {
        let popup_area = Self::centered_rect(60, 40, area);

        frame.render_widget(Clear, popup_area);

        let mut lines = vec![
            Line::from("Permission Required")
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Line::from(""),
        ];

        for option in &self.permission_options {
            lines.push(Line::from(format!("  {}", option)));
        }

        lines.push(Line::from(""));
        lines.push(
            Line::from("Press number to select or ESC to cancel")
                .style(Style::default().fg(Color::Gray)),
        );

        let block = Block::default()
            .title("⚠️ Action Required")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });

        frame.render_widget(paragraph, popup_area);
    }

    /// Render connection status overlay
    fn render_connection_status(&self, frame: &mut Frame<'_>, area: Rect) {
        if let Some(error) = &self.connection_error {
            let popup_area = Self::centered_rect(60, 20, area);

            frame.render_widget(Clear, popup_area);

            let lines = vec![
                Line::from("Connection Error")
                    .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Line::from(""),
                Line::from(error.clone()),
                Line::from(""),
                Line::from("Attempting to reconnect...").style(Style::default().fg(Color::Gray)),
            ];

            let block = Block::default()
                .title("❌ Connection Failed")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red));

            let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });

            frame.render_widget(paragraph, popup_area);
        }
    }

    /// Create a centered rectangle
    fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(area);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    /// Disconnect from PTY service
    pub async fn disconnect(&mut self) -> Result<()> {
        if self.connected {
            self.ws_client.disconnect().await?;
            self.connected = false;
        }
        Ok(())
    }
}
