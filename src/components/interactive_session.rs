// ABOUTME: Interactive session component that replaces the old attached terminal
// Uses WebSocket to connect directly to the PTY service in the container

use crate::terminal::{InteractiveTerminalComponent, ViewMode};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};
use uuid::Uuid;

pub struct InteractiveSessionComponent {
    terminal: Option<InteractiveTerminalComponent>,
    session_id: Uuid,
    session_name: String,
    container_id: String,
    connection_status: ConnectionStatus,
}

#[derive(Debug, Clone)]
enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

impl InteractiveSessionComponent {
    /// Create a new interactive session component
    pub async fn new(session_id: Uuid, session_name: String, container_id: String) -> Result<Self> {
        Ok(Self {
            terminal: None,
            session_id,
            session_name,
            container_id,
            connection_status: ConnectionStatus::Disconnected,
        })
    }

    /// Connect to the container's PTY service via WebSocket
    pub async fn connect(&mut self) -> Result<()> {
        use tracing::{error, info, warn};

        info!(
            "Attempting to connect to container {} PTY service",
            self.container_id
        );
        self.connection_status = ConnectionStatus::Connecting;

        // First, check if the container has the PTY service running
        let has_pty = self.check_pty_service().await;
        if !has_pty {
            warn!(
                "Container {} does not have PTY service running. It may be an older container.",
                self.container_id
            );
            self.connection_status = ConnectionStatus::Failed(
                "PTY service not available. This container was created before PTY support. Please create a new session.".to_string()
            );
            return Err(anyhow::anyhow!("PTY service not available in container"));
        }

        // Get the host port mapping for port 8080
        let host_port = match self.get_host_port_for_container(8080).await {
            Ok(port) => {
                info!(
                    "Container {} has port 8080 mapped to host port {}",
                    self.container_id, port
                );
                port
            }
            Err(e) => {
                error!(
                    "Failed to get port mapping for container {}: {}",
                    self.container_id, e
                );
                self.connection_status =
                    ConnectionStatus::Failed(format!("Failed to get port mapping: {}", e));
                return Err(e);
            }
        };

        info!(
            "Creating WebSocket terminal component for localhost:{}",
            host_port
        );

        // Create the terminal component with localhost and mapped port
        match InteractiveTerminalComponent::new_with_host_port(
            self.session_id,
            self.session_name.clone(),
            self.container_id.clone(),
            host_port,
        )
        .await
        {
            Ok(mut terminal) => {
                info!("Terminal component created, attempting to connect...");
                // Try to connect
                match terminal.connect().await {
                    Ok(_) => {
                        info!(
                            "Successfully connected to PTY service on localhost:{}",
                            host_port
                        );
                        self.connection_status = ConnectionStatus::Connected;

                        // Send an initial input to ensure prompt appears
                        // This helps when attaching to a new session
                        if let Err(e) = terminal.send_input("").await {
                            warn!("Failed to send initial input to trigger prompt: {}", e);
                        }

                        self.terminal = Some(terminal);
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to connect to PTY service: {}", e);
                        self.connection_status = ConnectionStatus::Failed(e.to_string());
                        Err(e)
                    }
                }
            }
            Err(e) => {
                error!("Failed to create terminal component: {}", e);
                self.connection_status = ConnectionStatus::Failed(e.to_string());
                Err(e)
            }
        }
    }

    /// Check if the container has PTY service running
    async fn check_pty_service(&self) -> bool {
        use crate::docker::ContainerManager;
        use tracing::{error, info, warn};

        // Try multiple methods to detect PTY service
        match ContainerManager::new().await {
            Ok(manager) => {
                // Method 1: Check for node process running index.js (most reliable)
                let check_cmd = vec![
                    "bash".to_string(),
                    "-c".to_string(),
                    "ps aux | grep 'node.*index.js' | grep -v grep".to_string(),
                ];

                match manager.exec_command(&self.container_id, check_cmd).await {
                    Ok(output_bytes) => {
                        let output = String::from_utf8_lossy(&output_bytes);
                        info!(
                            "PTY process check output for container {}: {}",
                            self.container_id,
                            output.trim()
                        );

                        if !output.trim().is_empty()
                            && output.contains("node")
                            && output.contains("index.js")
                        {
                            info!(
                                "✓ PTY service detected via process check in container {}",
                                self.container_id
                            );
                            return true;
                        } else {
                            warn!("Process check failed, trying directory check...");
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to check PTY process in container {}: {}",
                            self.container_id, e
                        );
                    }
                }

                // Method 2: Check if PTY service directory exists
                let check_dir = vec![
                    "test".to_string(),
                    "-d".to_string(),
                    "/app/pty-service".to_string(),
                ];

                match manager.exec_command(&self.container_id, check_dir).await {
                    Ok(_) => {
                        // Exit code 0 means directory exists
                        info!(
                            "✓ PTY service directory found in container {}",
                            self.container_id
                        );

                        // Also try to check if we can curl the service locally
                        let curl_check = vec![
                            "bash".to_string(),
                            "-c".to_string(),
                            "curl -f -s http://localhost:8080 2>&1 || echo 'CURL_FAILED'"
                                .to_string(),
                        ];

                        match manager.exec_command(&self.container_id, curl_check).await {
                            Ok(curl_output) => {
                                let output = String::from_utf8_lossy(&curl_output);
                                if output.contains("Upgrade Required") {
                                    info!(
                                        "✓ PTY service responding on port 8080 in container {}",
                                        self.container_id
                                    );
                                    return true;
                                } else {
                                    warn!(
                                        "PTY service directory exists but service not responding: {}",
                                        output.trim()
                                    );
                                }
                            }
                            Err(e) => {
                                warn!("Curl check failed: {}", e);
                                // Directory exists, assume service might be starting up
                                return true;
                            }
                        }

                        return true;
                    }
                    Err(e) => {
                        error!(
                            "PTY service directory not found in container {}: {}",
                            self.container_id, e
                        );
                    }
                }

                // If all checks fail, PTY is not available
                error!(
                    "❌ PTY service not detected in container {} after all checks",
                    self.container_id
                );
                false
            }
            Err(e) => {
                error!("Failed to create container manager for PTY check: {}", e);
                // If we can't check, assume it might be available
                true
            }
        }
    }

    /// Get the host port mapping for a container port
    async fn get_host_port_for_container(&self, container_port: u16) -> Result<u16> {
        use crate::docker::ContainerManager;
        use tracing::info;

        let container_manager = ContainerManager::new().await?;
        let port_mappings =
            container_manager.get_container_port_mappings(&self.container_id).await?;

        // Log all port mappings for debugging
        info!(
            "Container {} port mappings: {:?}",
            self.container_id, port_mappings
        );

        port_mappings
            .get(&container_port)
            .copied()
            .ok_or_else(|| anyhow::anyhow!(
                "Port {} is not mapped for container. This container may have been created before PTY support was added. Please create a new session.",
                container_port
            ))
    }

    /// Handle keyboard input
    pub async fn handle_input(&mut self, key: KeyEvent) -> Result<bool> {
        // Handle escape to return to session list
        if key.code == KeyCode::Esc
            && !matches!(
                self.terminal.as_ref().map(|t| t.view_mode),
                Some(ViewMode::Expanded)
            )
        {
            return Ok(false); // Signal to return to session list
        }

        // Forward to terminal if connected
        if let Some(terminal) = &mut self.terminal {
            terminal.handle_input(key).await
        } else {
            // Handle connection attempt
            if key.code == KeyCode::Char('c')
                && !matches!(
                    self.connection_status,
                    ConnectionStatus::Connected | ConnectionStatus::Connecting
                )
            {
                self.connect().await?;
            }
            Ok(true)
        }
    }

    /// Render the component
    pub async fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        match &self.connection_status {
            ConnectionStatus::Connected => {
                if let Some(terminal) = &mut self.terminal {
                    terminal.render(frame, area).await;
                } else {
                    self.render_error(frame, area, "Terminal component not initialized");
                }
            }
            ConnectionStatus::Connecting => {
                self.render_connecting(frame, area);
            }
            ConnectionStatus::Disconnected => {
                self.render_disconnected(frame, area);
            }
            ConnectionStatus::Failed(error) => {
                self.render_error(frame, area, error);
            }
        }
    }

    /// Render connecting state
    fn render_connecting(&self, frame: &mut Frame<'_>, area: Rect) {
        let _chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        let title = format!("🔗 Connecting to: {}", self.session_name);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let content = vec![
            "",
            "⏳ Establishing WebSocket connection to PTY service...",
            "",
            "This may take a moment while the container initializes.",
        ]
        .join("\n");

        let paragraph = Paragraph::new(content)
            .block(block)
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render disconnected state
    fn render_disconnected(&self, frame: &mut Frame<'_>, area: Rect) {
        let title = format!("📺 Session: {}", self.session_name);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray));

        let content = vec![
            "",
            "Not connected to PTY service",
            "",
            "Press [c] to connect",
            "Press [Esc] to return to session list",
        ]
        .join("\n");

        let paragraph = Paragraph::new(content)
            .block(block)
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render error state
    fn render_error(&self, frame: &mut Frame<'_>, area: Rect, error: &str) {
        let title = format!("❌ Session: {}", self.session_name);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let content = vec![
            "",
            "Failed to connect to PTY service",
            "",
            &format!("Error: {}", error),
            "",
            "Press [c] to retry connection",
            "Press [Esc] to return to session list",
        ]
        .join("\n");

        let paragraph = Paragraph::new(content)
            .block(block)
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Check if terminal is in expanded mode
    pub fn is_expanded(&self) -> bool {
        self.terminal
            .as_ref()
            .map(|t| t.view_mode == ViewMode::Expanded)
            .unwrap_or(false)
    }

    /// Disconnect from PTY service
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut terminal) = self.terminal.take() {
            terminal.disconnect().await?;
        }
        self.connection_status = ConnectionStatus::Disconnected;
        Ok(())
    }
}
