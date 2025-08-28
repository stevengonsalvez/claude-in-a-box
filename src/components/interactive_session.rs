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
        use tracing::{info, warn};

        // Try to check if the PTY service is running by checking for the process
        match ContainerManager::new().await {
            Ok(manager) => {
                // Check if node process is running on port 8080
                let check_cmd = vec![
                    "bash".to_string(),
                    "-c".to_string(),
                    "netstat -tulpn 2>/dev/null | grep :8080 || lsof -i :8080 2>/dev/null || echo 'PTY_NOT_FOUND'".to_string(),
                ];

                match manager.exec_command(&self.container_id, check_cmd).await {
                    Ok(output_bytes) => {
                        let output = String::from_utf8_lossy(&output_bytes);
                        let has_pty = !output.contains("PTY_NOT_FOUND") && !output.is_empty();
                        if has_pty {
                            info!("PTY service detected in container {}", self.container_id);
                        } else {
                            warn!("PTY service not found in container {}", self.container_id);
                        }
                        has_pty
                    }
                    Err(e) => {
                        warn!(
                            "Failed to check PTY service in container {}: {}",
                            self.container_id, e
                        );
                        // Assume it might be available if we can't check
                        true
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create container manager: {}", e);
                // Assume it might be available if we can't check
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
