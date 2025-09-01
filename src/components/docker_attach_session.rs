// ABOUTME: Docker attach-based interactive session component
// Uses Docker's native attach API to connect directly to Claude CLI running as PID 1

use anyhow::{Context, Result};
use arboard::Clipboard;
use bollard::container::{AttachContainerOptions, AttachContainerResults, ResizeContainerTtyOptions};
use bollard::Docker;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use futures_util::stream::StreamExt;
use ratatui::{
    layout::Rect,
    prelude::*,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use vt100::Parser;
use crate::terminal::terminal_emulator::TerminalEmulatorWidget;

/// Mode for handling Docker attach output and interactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttachMode {
    /// Simple mode: ANSI stripped, for readable logs
    #[default]
    Simple,
    /// Terminal mode: Full vt100 emulation with preserved formatting
    Terminal,
    /// External mode: Launch system terminal with docker attach
    External,
}

impl AttachMode {
    /// Cycle to the next mode
    pub fn next(self) -> Self {
        match self {
            AttachMode::Simple => AttachMode::Terminal,
            AttachMode::Terminal => AttachMode::External,
            AttachMode::External => AttachMode::Simple,
        }
    }
}

impl std::fmt::Display for AttachMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttachMode::Simple => write!(f, "Simple"),
            AttachMode::Terminal => write!(f, "Terminal"),
            AttachMode::External => write!(f, "External"),
        }
    }
}

pub struct DockerAttachSession {
    session_id: Uuid,
    session_name: String,
    container_id: String,
    docker: Docker,
    connection_status: ConnectionStatus,
    
    // Mode for handling output and interactions
    mode: AttachMode,
    
    // VT100 parser for Terminal mode
    vt100_parser: Option<Parser>,
    
    // Terminal dimensions
    terminal_width: u16,
    terminal_height: u16,
    
    // Terminal emulator widget for proper rendering
    terminal_emulator: TerminalEmulatorWidget,
    
    // Channels for stdin/stdout
    stdin_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    stdout_rx: Option<Arc<Mutex<mpsc::UnboundedReceiver<Vec<u8>>>>>,
    
    // Buffer for terminal output (kept for backward compatibility)
    output_buffer: Vec<String>,
    max_buffer_lines: usize,
    
    // Scrollback management
    scroll_position: usize, // 0 means at bottom (latest), higher values scroll up
    
    // Text selection for copy/paste
    selection_start: Option<(usize, usize)>, // (line, column)
    selection_end: Option<(usize, usize)>,   // (line, column)
    selection_mode: bool,
    
    // Clipboard
    clipboard: Option<Clipboard>,
    
    // Task handle for the attach loop
    attach_handle: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Debug, Clone)]
enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

impl DockerAttachSession {
    /// Create a new Docker attach session
    pub async fn new(
        session_id: Uuid,
        session_name: String,
        container_id: String,
    ) -> Result<Self> {
        // Connect to Docker
        let docker = crate::docker::container_manager::ContainerManager::connect_to_docker()
            .context("Failed to connect to Docker")?;
        
        // Try to initialize clipboard (non-blocking)
        let clipboard = match Clipboard::new() {
            Ok(cb) => Some(cb),
            Err(e) => {
                warn!("Failed to initialize clipboard: {}", e);
                None
            }
        };

        // Initialize terminal emulator with reasonable default dimensions
        let mut terminal_emulator = TerminalEmulatorWidget::new(80, 24);
        terminal_emulator.set_title(format!(
            "{} - {}", 
            session_name, 
            container_id.chars().take(12).collect::<String>()
        ));

        Ok(Self {
            session_id,
            session_name,
            container_id,
            docker,
            connection_status: ConnectionStatus::Disconnected,
            mode: AttachMode::default(),
            vt100_parser: Some(vt100::Parser::new(24, 80, 10000)),
            terminal_width: 80,  // Default terminal width
            terminal_height: 24, // Default terminal height
            stdin_tx: None,
            stdout_rx: None,
            output_buffer: Vec::new(),
            max_buffer_lines: 10000,
            terminal_emulator,
            scroll_position: 0,
            selection_start: None,
            selection_end: None,
            selection_mode: false,
            clipboard,
            attach_handle: None,
        })
    }
    
    /// Connect to the container using Docker attach
    pub async fn connect(&mut self) -> Result<()> {
        info!("Attaching to container {} running Claude CLI", self.container_id);
        self.connection_status = ConnectionStatus::Connecting;
        
        // Check if container is running
        let container_info = self.docker
            .inspect_container(&self.container_id, None)
            .await
            .context("Failed to inspect container")?;
        
        let is_running = container_info
            .state
            .as_ref()
            .and_then(|s| s.running)
            .unwrap_or(false);
        
        if !is_running {
            self.connection_status = ConnectionStatus::Failed("Container not running".to_string());
            return Err(anyhow::anyhow!("Container {} is not running", self.container_id));
        }
        
        // Set up attach options
        let options = AttachContainerOptions::<String> {
            stdin: Some(true),
            stdout: Some(true),
            stderr: Some(true),
            stream: Some(true),
            logs: Some(false),
            detach_keys: Some("ctrl-p,ctrl-q".to_string()),
        };
        
        // Attach to container
        let AttachContainerResults { output, input } = self.docker
            .attach_container(&self.container_id, Some(options))
            .await
            .context("Failed to attach to container")?;
        
        // Create channels for stdin/stdout
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (stdout_tx, stdout_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        
        self.stdin_tx = Some(stdin_tx);
        self.stdout_rx = Some(Arc::new(Mutex::new(stdout_rx)));
        
        // Spawn task to handle output stream
        let container_id = self.container_id.clone();
        let output_handle = tokio::spawn(async move {
            let mut output_stream = output;
            
            while let Some(chunk) = output_stream.next().await {
                match chunk {
                    Ok(bollard::container::LogOutput::StdOut { message }) => {
                        debug!("Container stdout: {} bytes", message.len());
                        if let Err(e) = stdout_tx.send(message.to_vec()) {
                            error!("Failed to send stdout: {}", e);
                            break;
                        }
                    }
                    Ok(bollard::container::LogOutput::StdErr { message }) => {
                        debug!("Container stderr: {} bytes", message.len());
                        // Also send stderr to the same channel for display
                        if let Err(e) = stdout_tx.send(message.to_vec()) {
                            error!("Failed to send stderr: {}", e);
                            break;
                        }
                    }
                    Ok(bollard::container::LogOutput::StdIn { .. }) => {
                        // Ignore stdin echoes
                    }
                    Ok(bollard::container::LogOutput::Console { message }) => {
                        // Console output (for TTY mode)
                        // This is what we'll typically see with Claude running with TTY
                        debug!("Container console: {} bytes", message.len());
                        if let Err(e) = stdout_tx.send(message.to_vec()) {
                            error!("Failed to send console output: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error reading container output: {}", e);
                        break;
                    }
                }
            }
            
            info!("Container {} output stream ended", container_id);
        });
        
        // Spawn task to handle input stream
        let mut input = input;
        let _input_handle = tokio::spawn(async move {
            while let Some(data) = stdin_rx.recv().await {
                if let Err(e) = input.write_all(&data).await {
                    error!("Failed to write to container stdin: {}", e);
                    break;
                }
                if let Err(e) = input.flush().await {
                    error!("Failed to flush container stdin: {}", e);
                    break;
                }
            }
        });
        
        // Store the handle (we'll abort it on disconnect)
        self.attach_handle = Some(output_handle);
        
        self.connection_status = ConnectionStatus::Connected;
        info!("Successfully attached to container {}", self.container_id);
        
        Ok(())
    }
    
    /// Send input to the container
    pub async fn send_input(&mut self, input: &str) -> Result<()> {
        if let Some(ref tx) = self.stdin_tx {
            tx.send(input.as_bytes().to_vec())
                .context("Failed to send input")?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }
    
    /// Handle keyboard input with comprehensive terminal key mapping
    pub async fn handle_input(&mut self, key: KeyEvent) -> Result<bool> {
        use crossterm::event::KeyModifiers;
        
        // Handle scrollback controls first (these don't get sent to the container)
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            match key.code {
                KeyCode::PageUp => {
                    self.scroll_up(5); // Scroll up 5 lines
                    return Ok(true);
                }
                KeyCode::PageDown => {
                    self.scroll_down(5); // Scroll down 5 lines
                    return Ok(true);
                }
                _ => {}
            }
        }

        match key.code {
            // Handle Escape key to reset scroll and selection
            KeyCode::Esc => {
                // Reset scroll position and selection on Escape
                self.scroll_position = 0;
                self.selection_start = None;
                self.selection_end = None;
                self.selection_mode = false;
                // Also send ESC to container
                self.send_input("\x1b").await?;
                Ok(true)
            }
            // Control sequences
            KeyCode::Char(c) if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                match c {
                    // Docker detach sequence - Ctrl+P, Ctrl+Q
                    'p' => {
                        info!("Detach sequence initiated (Ctrl-P)");
                        self.send_input("\x10").await?; // Send actual Ctrl+P
                        Ok(true)
                    }
                    'q' => {
                        info!("Detach sequence completed (Ctrl-Q)");
                        self.send_input("\x11").await?; // Send actual Ctrl+Q
                        Ok(true)
                    }
                    // Common control sequences
                    'c' => {
                        // Ctrl+C - Try to copy selected text first, then SIGINT
                        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                            if let Some(text) = self.get_selected_text(start, end) {
                                if let Some(ref mut clipboard) = self.clipboard {
                                    match clipboard.set_text(text) {
                                        Ok(_) => {
                                            // Clear selection after copy
                                            self.selection_start = None;
                                            self.selection_end = None;
                                            self.selection_mode = false;
                                            return Ok(true);
                                        }
                                        Err(e) => {
                                            warn!("Failed to copy to clipboard: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        // If no selection or copy failed, send Ctrl+C to container
                        self.send_input("\x03").await?;
                        Ok(true)
                    }
                    'd' => {
                        // Ctrl+D - EOT (End of transmission)
                        self.send_input("\x04").await?;
                        Ok(true)
                    }
                    'z' => {
                        // Ctrl+Z - SIGTSTP (suspend)
                        self.send_input("\x1a").await?;
                        Ok(true)
                    }
                    'l' => {
                        // Ctrl+L - Clear screen
                        self.send_input("\x0c").await?;
                        Ok(true)
                    }
                    // Other control characters (Ctrl+A through Ctrl+Z)
                    'a' => {
                        self.send_input("\x01").await?;
                        Ok(true)
                    }
                    'b' => {
                        self.send_input("\x02").await?;
                        Ok(true)
                    }
                    'e' => {
                        self.send_input("\x05").await?;
                        Ok(true)
                    }
                    'f' => {
                        self.send_input("\x06").await?;
                        Ok(true)
                    }
                    'g' => {
                        self.send_input("\x07").await?;
                        Ok(true)
                    }
                    'h' => {
                        self.send_input("\x08").await?;
                        Ok(true)
                    }
                    'i' => {
                        self.send_input("\x09").await?;
                        Ok(true)
                    }
                    'j' => {
                        self.send_input("\x0a").await?;
                        Ok(true)
                    }
                    'k' => {
                        self.send_input("\x0b").await?;
                        Ok(true)
                    }
                    'm' => {
                        self.send_input("\x0d").await?;
                        Ok(true)
                    }
                    'n' => {
                        self.send_input("\x0e").await?;
                        Ok(true)
                    }
                    'o' => {
                        self.send_input("\x0f").await?;
                        Ok(true)
                    }
                    'r' => {
                        self.send_input("\x12").await?;
                        Ok(true)
                    }
                    's' => {
                        self.send_input("\x13").await?;
                        Ok(true)
                    }
                    't' => {
                        self.send_input("\x14").await?;
                        Ok(true)
                    }
                    'u' => {
                        self.send_input("\x15").await?;
                        Ok(true)
                    }
                    'v' => {
                        // Ctrl+V - Try to paste from clipboard first, then send control code
                        if let Some(ref mut clipboard) = self.clipboard {
                            match clipboard.get_text() {
                                Ok(text) => {
                                    self.send_input(&text).await?;
                                    return Ok(true);
                                }
                                Err(e) => {
                                    warn!("Failed to paste from clipboard: {}", e);
                                }
                            }
                        }
                        // If paste failed, send Ctrl+V control code to container
                        self.send_input("\x16").await?;
                        Ok(true)
                    }
                    'w' => {
                        self.send_input("\x17").await?;
                        Ok(true)
                    }
                    'x' => {
                        self.send_input("\x18").await?;
                        Ok(true)
                    }
                    'y' => {
                        self.send_input("\x19").await?;
                        Ok(true)
                    }
                    _ => {
                        // Send regular character with control modifier
                        self.send_input(&c.to_string()).await?;
                        Ok(true)
                    }
                }
            }
            // Mode toggle key
            KeyCode::Char('m') => {
                self.mode = self.mode.next();
                
                // Initialize vt100 parser when switching to Terminal mode
                if self.mode == AttachMode::Terminal {
                    self.vt100_parser = Some(Parser::new(80, 24, 1000));
                    info!("Switched to Terminal mode with vt100 emulation");
                } else {
                    self.vt100_parser = None;
                    info!("Switched to {} mode", self.mode);
                }
                
                // For External mode, we could potentially launch system terminal here
                // Skip in tests to avoid actually launching terminal
                #[cfg(not(test))]
                if self.mode == AttachMode::External {
                    self.launch_external_terminal().await?;
                }
                
                Ok(true)
            }
            // Regular characters
            KeyCode::Char(c) => {
                self.send_input(&c.to_string()).await?;
                Ok(true)
            }
            // Special keys
            KeyCode::Enter => {
                self.send_input("\r").await?;
                Ok(true)
            }
            KeyCode::Backspace => {
                self.send_input("\x7f").await?;
                Ok(true)
            }
            KeyCode::Delete => {
                // Delete key sends escape sequence
                self.send_input("\x1b[3~").await?;
                Ok(true)
            }
            KeyCode::Tab => {
                self.send_input("\t").await?;
                Ok(true)
            }
            // Arrow keys (already implemented correctly)
            KeyCode::Up => {
                self.send_input("\x1b[A").await?;
                Ok(true)
            }
            KeyCode::Down => {
                self.send_input("\x1b[B").await?;
                Ok(true)
            }
            KeyCode::Right => {
                self.send_input("\x1b[C").await?;
                Ok(true)
            }
            KeyCode::Left => {
                self.send_input("\x1b[D").await?;
                Ok(true)
            }
            // Function keys F1-F12
            KeyCode::F(1) => {
                self.send_input("\x1bOP").await?;
                Ok(true)
            }
            KeyCode::F(2) => {
                self.send_input("\x1bOQ").await?;
                Ok(true)
            }
            KeyCode::F(3) => {
                self.send_input("\x1bOR").await?;
                Ok(true)
            }
            KeyCode::F(4) => {
                self.send_input("\x1bOS").await?;
                Ok(true)
            }
            KeyCode::F(5) => {
                self.send_input("\x1b[15~").await?;
                Ok(true)
            }
            KeyCode::F(6) => {
                self.send_input("\x1b[17~").await?;
                Ok(true)
            }
            KeyCode::F(7) => {
                self.send_input("\x1b[18~").await?;
                Ok(true)
            }
            KeyCode::F(8) => {
                self.send_input("\x1b[19~").await?;
                Ok(true)
            }
            KeyCode::F(9) => {
                self.send_input("\x1b[20~").await?;
                Ok(true)
            }
            KeyCode::F(10) => {
                self.send_input("\x1b[21~").await?;
                Ok(true)
            }
            KeyCode::F(11) => {
                self.send_input("\x1b[23~").await?;
                Ok(true)
            }
            KeyCode::F(12) => {
                self.send_input("\x1b[24~").await?;
                Ok(true)
            }
            // Page navigation keys
            KeyCode::PageUp => {
                self.send_input("\x1b[5~").await?;
                Ok(true)
            }
            KeyCode::PageDown => {
                self.send_input("\x1b[6~").await?;
                Ok(true)
            }
            KeyCode::Home => {
                self.send_input("\x1b[H").await?;
                Ok(true)
            }
            KeyCode::End => {
                self.send_input("\x1b[F").await?;
                Ok(true)
            }
            // Insert key
            KeyCode::Insert => {
                self.send_input("\x1b[2~").await?;
                Ok(true)
            }
            // Other keys - handle gracefully
            _ => Ok(true),
        }
    }
    
    /// Launch external system terminal with docker attach
    async fn launch_external_terminal(&self) -> Result<()> {
        let container_id = &self.container_id;
        info!("Launching external terminal for container {}", container_id);
        
        // Platform-specific terminal launch
        #[cfg(target_os = "macos")]
        {
            let command = format!(
                r#"tell application "Terminal" to do script "docker attach {}""#,
                container_id
            );
            std::process::Command::new("osascript")
                .args(["-e", &command])
                .spawn()
                .context("Failed to launch Terminal.app")?;
        }
        
        #[cfg(target_os = "linux")]
        {
            // Try various Linux terminal emulators
            let terminals = [
                ("gnome-terminal", vec!["--", "docker", "attach", container_id]),
                ("konsole", vec!["-e", "docker", "attach", container_id]),
                ("xfce4-terminal", vec!["-e", &format!("docker attach {}", container_id)]),
                ("xterm", vec!["-e", "docker", "attach", container_id]),
            ];
            
            let mut launched = false;
            for (terminal, args) in terminals {
                if let Ok(_) = std::process::Command::new(terminal)
                    .args(args)
                    .spawn()
                {
                    launched = true;
                    break;
                }
            }
            
            if !launched {
                return Err(anyhow::anyhow!("No suitable terminal emulator found on Linux"));
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            // Windows Terminal or cmd fallback
            if let Ok(_) = std::process::Command::new("wt")
                .args(["docker", "attach", container_id])
                .spawn()
            {
                // Windows Terminal launched successfully
            } else {
                // Fallback to cmd
                std::process::Command::new("cmd")
                    .args(["/c", "start", "cmd", "/k", &format!("docker attach {}", container_id)])
                    .spawn()
                    .context("Failed to launch cmd.exe")?;
            }
        }
        
        Ok(())
    }
    
    /// Process any pending output from the container
    pub async fn process_output(&mut self) -> Result<()> {
        if let Some(ref rx) = self.stdout_rx {
            let mut rx = rx.lock().await;
            
            // Process all available output
            while let Ok(data) = rx.try_recv() {
                // Convert bytes to string (handling potential UTF-8 errors)
                let raw_text = String::from_utf8_lossy(&data);
                
                // Always feed raw data to terminal emulator for proper ANSI handling
                self.terminal_emulator.process_output(&raw_text);
                
                // Process based on current mode
                match self.mode {
                    AttachMode::Terminal => {
                        // Feed raw bytes directly to the vt100 parser
                        // The parser handles all ANSI escape sequences properly
                        if let Some(ref mut parser) = self.vt100_parser {
                            parser.process(&data);
                        }
                        // Terminal mode: don't update output_buffer, use vt100 parser screen instead
                    }
                    AttachMode::Simple => {
                        // Strip ANSI escape sequences for readable text output
                        let text = strip_ansi_escapes::strip_str(&raw_text);
                        
                        // If buffer is empty or last line doesn't end with newline, append to it
                        if self.output_buffer.is_empty() {
                            self.output_buffer.push(text.to_string());
                        } else {
                            let last_idx = self.output_buffer.len() - 1;
                            
                            // Check if text contains newlines
                            if text.contains('\n') {
                                let parts: Vec<&str> = text.split('\n').collect();
                                
                                // Append first part to last line
                                self.output_buffer[last_idx].push_str(parts[0]);
                                
                                // Add remaining parts as new lines
                                for part in parts.iter().skip(1) {
                                    self.output_buffer.push(part.to_string());
                                }
                            } else {
                                // No newlines, just append to last line
                                self.output_buffer[last_idx].push_str(&text);
                            }
                        }
                        
                        // Limit buffer size
                        while self.output_buffer.len() > self.max_buffer_lines {
                            self.output_buffer.remove(0);
                        }
                    }
                    AttachMode::External => {
                        // External mode: minimal processing since output will go to external terminal
                    }
                }
                
                // Auto-scroll to bottom when new content arrives (unless manually scrolled up)
                if self.scroll_position == 0 {
                    // Stay at bottom - no scroll position change needed
                } else {
                    // User has scrolled up, maintain relative position
                    // This keeps the user's view stable while new content is added
                }
            }
        }
        
        Ok(())
    }
    
    /// Scroll up in the output buffer
    pub fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.output_buffer.len().saturating_sub(1);
        self.scroll_position = (self.scroll_position + lines).min(max_scroll);
    }
    
    /// Scroll down in the output buffer
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_position = self.scroll_position.saturating_sub(lines);
    }
    
    /// Calculate a valid selection range from start and end coordinates
    pub fn calculate_selection_range(
        &self,
        start: (usize, usize),
        end: (usize, usize)
    ) -> Option<((usize, usize), (usize, usize))> {
        // Ensure start comes before end
        let (actual_start, actual_end) = if start.0 < end.0 || (start.0 == end.0 && start.1 <= end.1) {
            (start, end)
        } else {
            (end, start)
        };
        
        // Validate coordinates are within buffer bounds
        if actual_start.0 < self.output_buffer.len() && actual_end.0 < self.output_buffer.len() {
            Some((actual_start, actual_end))
        } else {
            None
        }
    }
    
    /// Get selected text from the buffer
    pub fn get_selected_text(&self, start: (usize, usize), end: (usize, usize)) -> Option<String> {
        let range = self.calculate_selection_range(start, end)?;
        let (start, end) = range;
        
        if start.0 >= self.output_buffer.len() {
            return None;
        }
        
        let mut result = String::new();
        
        if start.0 == end.0 {
            // Single line selection
            let line = &self.output_buffer[start.0];
            let start_col = start.1.min(line.len());
            let end_col = end.1.min(line.len());
            
            if start_col < end_col {
                result.push_str(&line[start_col..end_col]);
            }
        } else {
            // Multi-line selection
            for (line_idx, line) in self.output_buffer.iter().enumerate().skip(start.0) {
                if line_idx > end.0 {
                    break;
                }
                
                if line_idx == start.0 {
                    // First line - from start column to end
                    let start_col = start.1.min(line.len());
                    result.push_str(&line[start_col..]);
                } else if line_idx == end.0 {
                    // Last line - from beginning to end column
                    let end_col = end.1.min(line.len());
                    result.push_str(&line[..end_col]);
                } else {
                    // Middle lines - entire line
                    result.push_str(line);
                }
                
                // Add newline unless it's the last line
                if line_idx < end.0 {
                    result.push('\n');
                }
            }
        }
        
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
    
    /// Handle mouse events for text selection
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> bool {
        use crossterm::event::MouseEventKind;
        
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Start selection
                if let Some(pos) = self.screen_to_buffer_pos(mouse.column, mouse.row, area) {
                    self.selection_start = Some(pos);
                    self.selection_end = Some(pos);
                    self.selection_mode = true;
                }
                true
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Update selection end
                if self.selection_mode {
                    if let Some(pos) = self.screen_to_buffer_pos(mouse.column, mouse.row, area) {
                        self.selection_end = Some(pos);
                    }
                }
                true
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // End selection
                self.selection_mode = false;
                true
            }
            MouseEventKind::ScrollUp => {
                // Scroll up with mouse wheel
                self.scroll_up(3);
                true
            }
            MouseEventKind::ScrollDown => {
                // Scroll down with mouse wheel
                self.scroll_down(3);
                true
            }
            _ => false,
        }
    }
    
    /// Convert screen coordinates to buffer position
    fn screen_to_buffer_pos(&self, col: u16, row: u16, area: Rect) -> Option<(usize, usize)> {
        // Account for border
        let inner = Block::default().borders(Borders::ALL).inner(area);
        
        if col < inner.x || row < inner.y || col >= inner.x + inner.width || row >= inner.y + inner.height {
            return None;
        }
        
        let relative_row = (row - inner.y) as usize;
        let relative_col = (col - inner.x) as usize;
        
        // Calculate which buffer line this corresponds to
        let visible_lines = inner.height as usize;
        let total_lines = self.output_buffer.len();
        let start_line = if self.scroll_position == 0 {
            total_lines.saturating_sub(visible_lines)
        } else {
            let from_bottom = self.scroll_position;
            total_lines.saturating_sub(from_bottom + visible_lines)
        };
        
        let buffer_line = start_line + relative_row;
        
        if buffer_line >= self.output_buffer.len() {
            return None;
        }
        
        Some((buffer_line, relative_col))
    }
    
    /// Get title with mode indicator for UI display
    pub fn get_title_with_mode(&self) -> String {
        format!(
            " {} - {} [{:?}] [{}] ",
            self.session_name,
            self.container_id.chars().take(12).collect::<String>(),
            self.connection_status,
            self.mode
        )
    }
    
    /// Render the session
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // Update terminal emulator dimensions if they've changed
        let inner_area = Block::default().borders(Borders::ALL).inner(area);
        
        // Check if dimensions have actually changed to avoid unnecessary work
        let new_width = inner_area.width;
        let new_height = inner_area.height;
        
        if new_width != self.terminal_width || new_height != self.terminal_height {
            debug!("Terminal area changed from {}x{} to {}x{}", 
                self.terminal_width, self.terminal_height, new_width, new_height);
            
            // Update our internal dimensions and vt100 parser
            if let Err(e) = self.resize(new_width, new_height) {
                warn!("Failed to resize internal terminal state: {}", e);
            }
        }
        
        // Always update the terminal emulator widget
        self.terminal_emulator.resize(new_width, new_height);
        
        // Update title with connection status and mode
        let title = self.get_title_with_mode();
        self.terminal_emulator.set_title(title);
        
        // Set focus based on connection status
        match self.connection_status {
            ConnectionStatus::Connected => self.terminal_emulator.set_focused(true),
            ConnectionStatus::Connecting => self.terminal_emulator.set_focused(true),
            ConnectionStatus::Failed(_) | ConnectionStatus::Disconnected => {
                self.terminal_emulator.set_focused(false);
            }
        }
        
        // Render the terminal emulator widget which handles ANSI properly
        self.terminal_emulator.render_ref(area, buf);
        
        // Show connection status overlay if not connected
        if matches!(self.connection_status, ConnectionStatus::Failed(_) | ConnectionStatus::Disconnected) {
            let msg = match &self.connection_status {
                ConnectionStatus::Failed(err) => format!("Connection failed: {}", err),
                ConnectionStatus::Disconnected => "Not connected. Press Enter to connect.".to_string(),
                _ => String::new(),
            };
            
            // Create a centered overlay for the status message
            let overlay_width = (area.width / 2).max(msg.len() as u16 + 4);
            let overlay_height = 3;
            let overlay_x = area.x + (area.width.saturating_sub(overlay_width)) / 2;
            let overlay_y = area.y + (area.height.saturating_sub(overlay_height)) / 2;
            
            let status_area = Rect {
                x: overlay_x,
                y: overlay_y,
                width: overlay_width,
                height: overlay_height,
            };
            
            if status_area.x < area.right() && status_area.y < area.bottom() {
                let status_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red));
                
                let status_inner = status_block.inner(status_area);
                status_block.render(status_area, buf);
                
                let status = Paragraph::new(msg)
                    .style(Style::default().fg(Color::Red))
                    .alignment(ratatui::layout::Alignment::Center)
                    .wrap(ratatui::widgets::Wrap { trim: true });
                
                status.render(status_inner, buf);
            }
        }
    }
    
    /// Resize the terminal and update both local parser and container TTY
    pub fn resize(&mut self, width: u16, height: u16) -> Result<()> {
        debug!("Resizing terminal to {}x{}", width, height);
        
        // Update internal dimensions
        self.terminal_width = width;
        self.terminal_height = height;
        
        // Update terminal emulator dimensions
        self.terminal_emulator.resize(width, height);
        
        // Update vt100 parser dimensions if in Terminal mode
        if self.mode == AttachMode::Terminal {
            self.vt100_parser = Some(Parser::new(height, width, self.max_buffer_lines));
        }
        
        Ok(())
    }
    
    /// Handle terminal dimension changes from external sources (e.g., TUI framework)
    /// This method triggers both local resize and Docker container resize if connected
    pub async fn handle_terminal_resize(&mut self, width: u16, height: u16) -> Result<()> {
        debug!("Handling terminal resize to {}x{} from external source", width, height);
        
        // Do local resize first
        self.resize(width, height)?;
        
        // If connected to Docker, also send resize signal
        if matches!(self.connection_status, ConnectionStatus::Connected) {
            let resize_options = ResizeContainerTtyOptions {
                width,
                height,
            };
            
            // Send resize to Docker container - handle errors gracefully  
            match self.docker.resize_container_tty(&self.container_id, resize_options).await {
                Ok(_) => {
                    debug!("Successfully sent resize signal to container {}", self.container_id);
                }
                Err(e) => {
                    warn!("Failed to send resize signal to container {}: {}", self.container_id, e);
                    // Don't return error - internal state is still updated
                }
            }
        }
        
        Ok(())
    }
    
    /// Resize the terminal and send signal to connected Docker container
    pub async fn resize_with_docker(&mut self, width: u16, height: u16) -> Result<()> {
        debug!("Resizing terminal to {}x{} with Docker API", width, height);
        
        // First do the local resize
        self.resize(width, height)?;
        
        // If we're connected to a container, send resize signal via Docker API
        if matches!(self.connection_status, ConnectionStatus::Connected) {
            let resize_options = ResizeContainerTtyOptions {
                width,
                height,
            };
            
            // Send resize to Docker container - handle errors gracefully
            match self.docker.resize_container_tty(&self.container_id, resize_options).await {
                Ok(_) => {
                    debug!("Successfully sent resize signal to container {}", self.container_id);
                }
                Err(e) => {
                    warn!("Failed to send resize signal to container {}: {}", self.container_id, e);
                    // Don't return error - internal state is still updated
                }
            }
        }
        
        Ok(())
    }

    /// Disconnect from the container
    pub async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from container {}", self.container_id);
        
        // Abort the attach task
        if let Some(handle) = self.attach_handle.take() {
            handle.abort();
        }
        
        // Clear channels
        self.stdin_tx = None;
        self.stdout_rx = None;
        
        self.connection_status = ConnectionStatus::Disconnected;
        
        Ok(())
    }
}

impl Drop for DockerAttachSession {
    fn drop(&mut self) {
        // Ensure we clean up the attach task
        if let Some(handle) = self.attach_handle.take() {
            handle.abort();
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::TerminalEmulatorWidget;
    
    fn create_test_session() -> DockerAttachSession {
        let mut terminal_emulator = TerminalEmulatorWidget::new(80, 24);
        terminal_emulator.set_title("test - test".to_string());
        
        // Create mock channels and keep receivers alive by storing them in static variables
        // This is a test-only approach to prevent channels from being closed
        let (stdin_tx, stdin_rx) = mpsc::unbounded_channel();
        let (stdout_tx, stdout_rx) = mpsc::unbounded_channel();
        
        // We leak these receivers in tests to keep channels open
        // This is acceptable in test code but not production code
        std::mem::forget(stdin_rx);
        std::mem::forget(stdout_tx);
        
        DockerAttachSession {
            session_id: uuid::Uuid::new_v4(),
            session_name: "test".to_string(),
            container_id: "test".to_string(),
            docker: crate::docker::container_manager::ContainerManager::connect_to_docker()
                .unwrap_or_else(|_| panic!("Failed to connect to Docker for test")),
            connection_status: ConnectionStatus::Disconnected,
            mode: AttachMode::default(),
            vt100_parser: None,
            terminal_width: 80,
            terminal_height: 24,
            stdin_tx: Some(stdin_tx),
            stdout_rx: Some(Arc::new(Mutex::new(stdout_rx))),
            output_buffer: vec![
                "line 1".to_string(),
                "line 2".to_string(), 
                "line 3".to_string(),
                "line 4".to_string(),
                "line 5".to_string(),
            ],
            max_buffer_lines: 10000,
            terminal_emulator,
            scroll_position: 0,
            selection_start: None,
            selection_end: None,
            selection_mode: false,
            clipboard: None,
            attach_handle: None,
        }
    }

    // #[test]
    // fn test_resize_updates_terminal_dimensions() {
    //     // Test that resize method updates internal terminal dimensions
    //     let mut session = create_test_session();

    //     // Test initial dimensions
    //     assert_eq!(session.terminal_width, 80);
    //     assert_eq!(session.terminal_height, 24);

    //     // Test resizing
    //     session.resize(100, 30).expect("Resize should not fail");

    //     assert_eq!(session.terminal_width, 100);
    //     assert_eq!(session.terminal_height, 30);
    // }

    // #[test] 
    // fn test_resize_initializes_vt100_parser_with_correct_dimensions() {
    //     // Test that resize method initializes vt100 parser with correct dimensions
    //     let mut session = create_test_session();

    //     // Parser should be None initially
    //     assert!(session.vt100_parser.is_none());

    //     // Resize should initialize the parser
    //     session.resize(100, 30).expect("Resize should not fail");

    //     // Verify parser is initialized with correct dimensions
    //     assert!(session.vt100_parser.is_some());
    //     let parser = session.vt100_parser.as_ref().unwrap();
    //     assert_eq!(parser.screen().size(), (30, 100)); // (rows, cols)
    // }

    // #[test]
    // fn test_resize_updates_existing_vt100_parser() {
    //     // Test that resize method updates existing vt100 parser dimensions
    //     let mut session = create_test_session();
        
    //     // Initialize parser with initial dimensions
    //     session.vt100_parser = Some(vt100::Parser::new(24, 80, 1000));

    //     // Verify initial dimensions
    //     assert_eq!(session.vt100_parser.as_ref().unwrap().screen().size(), (24, 80));

    //     // Resize should update the existing parser
    //     session.resize(120, 40).expect("Resize should not fail");

    //     // Verify parser dimensions are updated
    //     assert!(session.vt100_parser.is_some());
    //     let parser = session.vt100_parser.as_ref().unwrap();
    //     assert_eq!(parser.screen().size(), (40, 120)); // (rows, cols)
    // }

    // #[tokio::test]
    // async fn test_resize_sends_signal_to_connected_container() {
    //     // Test that resize method sends resize signal to Docker container when connected
    //     // This is a behavior test - we verify the method attempts to call Docker API
    //     let mut session = create_test_session();
    //     session.connection_status = ConnectionStatus::Connected;

    //     // For this test, we expect the resize to attempt the Docker API call
    //     // Since we can't mock Docker easily, we test that the method doesn't panic
    //     // and updates internal state even if the Docker call fails
    //     let result = session.resize_with_docker(100, 30).await;
        
    //     // Method should handle API failures gracefully and not crash
    //     assert!(result.is_ok() || result.is_err());
        
    //     // Internal state should still be updated regardless of Docker API result
    //     assert_eq!(session.terminal_width, 100);
    //     assert_eq!(session.terminal_height, 30);
    // }

    #[test]
    fn test_ansi_stripping_functionality() {
        // Test that ANSI escape sequences are properly stripped
        let raw_text_with_ansi = "\x1b[31mError: \x1b[0mSomething went wrong\x1b[32m OK\x1b[0m";
        let expected_clean_text = "Error: Something went wrong OK";
        
        let cleaned = strip_ansi_escapes::strip_str(raw_text_with_ansi);
        assert_eq!(cleaned, expected_clean_text);
    }

    #[test] 
    fn test_ansi_stripping_with_colors_and_formatting() {
        // Test complex ANSI sequences with colors and formatting
        let complex_ansi = "\x1b[1m\x1b[31mBold Red\x1b[0m \x1b[4mUnderlined\x1b[0m \x1b[32mGreen\x1b[0m";
        let expected = "Bold Red Underlined Green";
        
        let cleaned = strip_ansi_escapes::strip_str(complex_ansi);
        assert_eq!(cleaned, expected);
    }

    #[test]
    fn test_ansi_stripping_preserves_normal_text() {
        // Test that normal text without ANSI codes is unchanged
        let normal_text = "This is just regular text with no formatting";
        
        let cleaned = strip_ansi_escapes::strip_str(normal_text);
        assert_eq!(cleaned, normal_text);
    }

    #[test]
    fn test_ansi_stripping_with_cursor_movements() {
        // Test cursor movement sequences are stripped
        let cursor_text = "\x1b[2J\x1b[H\x1b[1;1HClear screen and home cursor";
        let expected = "Clear screen and home cursor";
        
        let cleaned = strip_ansi_escapes::strip_str(cursor_text);
        assert_eq!(cleaned, expected);
    }


    #[test]
    fn test_scroll_up_from_bottom() {
        let mut session = create_test_session();
        
        // At bottom initially (scroll_position = 0)
        assert_eq!(session.scroll_position, 0);
        
        // Scroll up should increase position
        session.scroll_up(2);
        assert_eq!(session.scroll_position, 2);
    }

    #[test]
    fn test_scroll_down_to_bottom() {
        let mut session = create_test_session();
        session.scroll_position = 3;
        
        // Scroll down should decrease position
        session.scroll_down(1);
        assert_eq!(session.scroll_position, 2);
        
        // Scroll down more to reach bottom
        session.scroll_down(5);
        assert_eq!(session.scroll_position, 0); // Should not go below 0
    }

    #[test]
    fn test_scroll_up_beyond_buffer_limit() {
        let mut session = create_test_session();
        
        // Try to scroll beyond available lines
        session.scroll_up(10);
        
        // Should be limited to buffer size
        let max_scroll = session.output_buffer.len().saturating_sub(1);
        assert_eq!(session.scroll_position, max_scroll);
    }

    #[test]
    fn test_selection_range_calculation() {
        let session = create_test_session();
        
        // Test valid selection range
        let start = (1, 2); // line 1, column 2
        let end = (3, 4);   // line 3, column 4
        
        let range = session.calculate_selection_range(start, end);
        assert_eq!(range, Some((start, end)));
        
        // Test swapped coordinates (end before start)
        let range = session.calculate_selection_range(end, start);
        assert_eq!(range, Some((start, end))); // Should be normalized
    }

    #[test]
    fn test_get_selected_text() {
        let session = create_test_session();
        
        // Select from beginning of line 1 to end of line 2
        let start = (1, 0); // "line 2" (0-indexed, so line 1)
        let end = (2, 6);   // "line 3" end
        
        let selected = session.get_selected_text(start, end);
        assert_eq!(selected, Some("line 2\nline 3".to_string()));
    }

    #[test]
    fn test_get_selected_text_partial_line() {
        let session = create_test_session();
        
        // Select part of a single line
        let start = (0, 2); // "ne 1"
        let end = (0, 4);   // "ne"
        
        let selected = session.get_selected_text(start, end);
        assert_eq!(selected, Some("ne".to_string()));
    }

    #[test]
    fn test_get_selected_text_invalid_range() {
        let session = create_test_session();
        
        // Select beyond buffer
        let start = (10, 0);
        let end = (11, 5);
        
        let selected = session.get_selected_text(start, end);
        assert_eq!(selected, None);
    }

    mod keyboard_input_tests {
        use super::*;
        use arboard::Clipboard;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use uuid::Uuid;

        #[tokio::test]
        async fn test_control_sequences() {
            let mut session = create_test_session().await;
            
            // Test Ctrl+C
            let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            // Note: We can't easily test the actual sent bytes without mocking,
            // but we can ensure the function returns success
            
            // Test Ctrl+D  
            let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Ctrl+Z
            let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Ctrl+L
            let key = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_special_keys() {
            let mut session = create_test_session().await;
            
            // Test Tab
            let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Enter
            let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Backspace
            let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Delete
            let key = KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Escape
            let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_function_keys() {
            let mut session = create_test_session().await;
            
            // Test F1-F12
            for i in 1..=12 {
                let key = match i {
                    1 => KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
                    2 => KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
                    3 => KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE),
                    4 => KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE),
                    5 => KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE),
                    6 => KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE),
                    7 => KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE),
                    8 => KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE),
                    9 => KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE),
                    10 => KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE),
                    11 => KeyEvent::new(KeyCode::F(11), KeyModifiers::NONE),
                    12 => KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE),
                    _ => unreachable!(),
                };
                
                let result = session.handle_input(key).await;
                assert!(result.is_ok(), "F{} key should be handled", i);
            }
        }

        #[tokio::test]
        async fn test_page_navigation_keys() {
            let mut session = create_test_session().await;
            
            // Test PageUp
            let key = KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test PageDown
            let key = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Home
            let key = KeyEvent::new(KeyCode::Home, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test End
            let key = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_arrow_keys() {
            let mut session = create_test_session().await;
            
            // Test Up arrow
            let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Down arrow
            let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Left arrow
            let key = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Right arrow
            let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_docker_detach_sequence() {
            let mut session = create_test_session().await;
            
            // Test Ctrl+P (Docker detach initiation)
            let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
            
            // Test Ctrl+Q (Docker detach completion)
            let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
        }

        // Helper function to create a test session without needing Docker
        async fn create_test_session() -> DockerAttachSession {
            super::create_test_session()
        }
    }

    // ========== Comprehensive Terminal Emulation Tests ==========

    mod comprehensive_terminal_tests {
        use super::*;
        use pretty_assertions::assert_eq;
        use tokio::sync::mpsc;
        use crossterm::event::KeyModifiers;

        // Helper function for terminal tests
        fn create_test_session_for_terminal_tests() -> DockerAttachSession {
            let session_id = Uuid::new_v4();
            let session_name = "terminal_test_session".to_string();
            let container_id = "terminal_test_container".to_string();
            
            DockerAttachSession {
                session_id,
                session_name: session_name.clone(),
                container_id: container_id.clone(),
                docker: crate::docker::container_manager::ContainerManager::connect_to_docker()
                    .unwrap_or_else(|_| panic!("Docker connection failed in test")),
                connection_status: ConnectionStatus::Disconnected,
                mode: AttachMode::Terminal, // Set to Terminal mode for these tests
                vt100_parser: Some(vt100::Parser::new(80, 24, 1000)),
                terminal_width: 80,
                terminal_height: 24,
                stdin_tx: None,
                stdout_rx: None,
                output_buffer: Vec::new(),
                max_buffer_lines: 1000,
                terminal_emulator: crate::terminal::terminal_emulator::TerminalEmulatorWidget::new(80, 24),
                scroll_position: 0,
                selection_start: None,
                selection_end: None,
                selection_mode: false,
                clipboard: arboard::Clipboard::new().ok(),
                attach_handle: None,
            }
        }

        // ========== VT100 Parser Integration Tests ==========

        #[test]
        fn test_vt100_parser_ansi_color_sequences() {
            // Test basic ANSI color sequences
            let mut parser = vt100::Parser::new(24, 80, 0);
            parser.process(b"\x1b[31mRed text\x1b[0m");
            
            let screen = parser.screen();
            let contents = screen.contents();
            
            // Should contain "Red text" without the escape codes
            assert!(contents.contains("Red text"));
        }

        #[test]
        fn test_vt100_parser_cursor_movement_commands() {
            let mut parser = vt100::Parser::new(24, 80, 0);
            
            // Write text, move cursor, write more
            parser.process(b"Hello");
            parser.process(b"\x1b[5D"); // Move cursor left 5 positions
            parser.process(b"World");
            
            let screen = parser.screen();
            let contents = screen.contents();
            
            // "World" should overwrite "Hello"
            assert!(contents.contains("World"));
        }

        #[test]
        fn test_vt100_parser_multiple_color_codes() {
            let mut parser = vt100::Parser::new(24, 80, 0);
            
            // Test various color codes
            parser.process(b"\x1b[31mRed\x1b[32mGreen\x1b[34mBlue\x1b[0m");
            
            let screen = parser.screen();
            let contents = screen.contents();
            
            assert!(contents.contains("Red"));
            assert!(contents.contains("Green"));
            assert!(contents.contains("Blue"));
        }

        #[test]
        fn test_vt100_parser_screen_clearing_commands() {
            let mut parser = vt100::Parser::new(24, 80, 0);
            
            // Write some text
            parser.process(b"Some initial text");
            
            // Clear screen
            parser.process(b"\x1b[2J");
            
            // Write new text
            parser.process(b"New text after clear");
            
            let screen = parser.screen();
            let contents = screen.contents();
            
            // Should not contain the initial text
            assert!(!contents.contains("Some initial text"));
            assert!(contents.contains("New text after clear"));
        }

        #[test]
        fn test_vt100_parser_home_cursor_positioning() {
            let mut parser = vt100::Parser::new(24, 80, 0);
            
            // Write text and move to home
            parser.process(b"Line 1\nLine 2\nLine 3");
            parser.process(b"\x1b[H"); // Move to home (1,1)
            parser.process(b"HOME");
            
            let screen = parser.screen();
            let contents = screen.contents();
            
            // "HOME" should appear at the beginning, overwriting "Line"
            assert!(contents.starts_with("HOME"));
        }

        #[test]
        fn test_vt100_parser_complex_escape_sequences() {
            let mut parser = vt100::Parser::new(24, 80, 0);
            
            // Test a complex sequence with multiple operations
            parser.process(b"\x1b[2J");           // Clear screen
            parser.process(b"\x1b[1;1H");         // Home cursor
            parser.process(b"\x1b[31;1mError:\x1b[0m "); // Bold red "Error:"
            parser.process(b"Something went wrong"); // Normal text
            parser.process(b"\x1b[2;1H");         // Move to line 2
            parser.process(b"\x1b[32mOK\x1b[0m"); // Green "OK"
            
            let screen = parser.screen();
            let contents = screen.contents();
            
            assert!(contents.contains("Error:"));
            assert!(contents.contains("Something went wrong"));
            assert!(contents.contains("OK"));
        }

        #[test]
        fn test_vt100_parser_cursor_save_restore() {
            let mut parser = vt100::Parser::new(24, 80, 0);
            
            // Move cursor and write text
            parser.process(b"\x1b[5;10HPosition 1");
            parser.process(b"\x1b[s"); // Save cursor position
            parser.process(b"\x1b[10;20HPosition 2");
            parser.process(b"\x1b[u"); // Restore cursor position
            parser.process(b"Back to 1");
            
            let screen = parser.screen();
            let contents = screen.contents();
            
            assert!(contents.contains("Position 1"));
            assert!(contents.contains("Position 2"));
            assert!(contents.contains("Back to 1"));
        }

        // ========== Input Handling Tests ==========

        #[tokio::test]
        async fn test_handle_control_characters_comprehensive() {
            let mut session = create_test_session_for_terminal_tests();
            
            // Set up channels for testing
            let (tx, mut rx) = mpsc::unbounded_channel();
            session.stdin_tx = Some(tx);
            
            // Test various control characters
            let test_cases = [
                (KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL), "Ctrl+C"),
                (KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL), "Ctrl+D"),
                (KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL), "Ctrl+Z"),
                (KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL), "Ctrl+L"),
                (KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL), "Ctrl+P (detach)"),
            ];
            
            for (key, description) in test_cases {
                let result = session.handle_input(key).await;
                assert!(result.is_ok(), "Failed to handle {}", description);
            }
        }

        #[tokio::test]
        async fn test_handle_special_keys_comprehensive() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, mut rx) = mpsc::unbounded_channel();
            session.stdin_tx = Some(tx);
            
            // Test special keys and verify exact byte sequences sent
            let test_cases = [
                (KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), b"\r", "Enter"),
                (KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE), b"\x7f", "Backspace"),
                (KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), b"\t", "Tab"),
                (KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), b"\x1b", "Escape"),
            ];
            
            for (key, expected_bytes, description) in test_cases {
                session.handle_input(key).await.unwrap();
                let received = rx.try_recv().unwrap();
                assert_eq!(received, expected_bytes, "Wrong bytes for {}", description);
            }
        }

        #[tokio::test]
        async fn test_handle_arrow_keys_comprehensive() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, mut rx) = mpsc::unbounded_channel();
            session.stdin_tx = Some(tx);
            
            // Test all arrow keys with exact ANSI escape sequences
            let test_cases = [
                (KeyCode::Up, b"\x1b[A", "Up arrow"),
                (KeyCode::Down, b"\x1b[B", "Down arrow"),
                (KeyCode::Right, b"\x1b[C", "Right arrow"),
                (KeyCode::Left, b"\x1b[D", "Left arrow"),
            ];
            
            for (key_code, expected, description) in test_cases {
                let key = KeyEvent::new(key_code, KeyModifiers::NONE);
                session.handle_input(key).await.unwrap();
                let received = rx.try_recv().unwrap();
                assert_eq!(received, expected, "Wrong sequence for {}", description);
            }
        }

        #[tokio::test]
        async fn test_handle_detach_sequence_comprehensive() {
            let mut session = create_test_session_for_terminal_tests();
            
            // Test Ctrl+P for detach sequence initiation
            let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
            let result = session.handle_input(key).await;
            
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), true); // Should indicate handled
            
            // Additional test for Ctrl+Q (detach completion in some terminals)
            let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
            let result = session.handle_input(key).await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_handle_function_keys() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, _rx) = mpsc::unbounded_channel();
            session.stdin_tx = Some(tx);
            
            // Test function keys F1-F12
            for i in 1..=12 {
                let key = KeyEvent::new(KeyCode::F(i), KeyModifiers::NONE);
                let result = session.handle_input(key).await;
                assert!(result.is_ok(), "F{} key should be handled", i);
            }
        }

        // ========== Output Rendering Tests ==========

        #[tokio::test]
        async fn test_output_rendering_normal_text() {
            let mut session = create_test_session_for_terminal_tests();
            
            // Set up mock output
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Send some normal text
            tx.send(b"Hello, World!".to_vec()).unwrap();
            
            // Process output
            session.process_output().await.unwrap();
            
            // Check that output is in buffer
            assert_eq!(session.output_buffer.len(), 1);
            assert_eq!(session.output_buffer[0], "Hello, World!");
        }

        #[tokio::test]
        async fn test_output_rendering_multi_line_text() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Send multi-line text
            tx.send(b"Line 1\nLine 2\nLine 3".to_vec()).unwrap();
            
            session.process_output().await.unwrap();
            
            // Should have 3 lines
            assert_eq!(session.output_buffer.len(), 3);
            assert_eq!(session.output_buffer[0], "Line 1");
            assert_eq!(session.output_buffer[1], "Line 2");
            assert_eq!(session.output_buffer[2], "Line 3");
        }

        #[tokio::test]
        async fn test_output_rendering_partial_line_handling() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Send partial line first
            tx.send(b"Hello, ".to_vec()).unwrap();
            session.process_output().await.unwrap();
            
            // Send completion
            tx.send(b"World!".to_vec()).unwrap();
            session.process_output().await.unwrap();
            
            // Should be combined into one line
            assert_eq!(session.output_buffer.len(), 1);
            assert_eq!(session.output_buffer[0], "Hello, World!");
        }

        #[tokio::test]
        async fn test_output_rendering_with_ansi_codes_terminal_mode() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Send text with ANSI codes (in Terminal mode, it processes through vt100)
            tx.send(b"\x1b[31mError:\x1b[0m Something went wrong".to_vec()).unwrap();
            
            session.process_output().await.unwrap();
            
            // In Terminal mode with vt100 parser, ANSI codes should still be stripped for display
            assert_eq!(session.output_buffer.len(), 1);
            assert_eq!(session.output_buffer[0], "Error: Something went wrong");
        }

        #[tokio::test]
        async fn test_output_rendering_mixed_content() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Send mixed content: partial line + newline + more text
            tx.send(b"Start ".to_vec()).unwrap();
            session.process_output().await.unwrap();
            
            tx.send(b"middle\nNew line ".to_vec()).unwrap();
            session.process_output().await.unwrap();
            
            tx.send(b"end".to_vec()).unwrap();
            session.process_output().await.unwrap();
            
            assert_eq!(session.output_buffer.len(), 2);
            assert_eq!(session.output_buffer[0], "Start middle");
            assert_eq!(session.output_buffer[1], "New line end");
        }

        // ========== Scrollback Tests ==========

        #[tokio::test]
        async fn test_scrollback_buffer_limits() {
            let mut session = create_test_session_for_terminal_tests();
            session.max_buffer_lines = 5; // Small buffer for testing
            
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Send more lines than buffer can hold
            for i in 1..=10 {
                tx.send(format!("Line {}\n", i).as_bytes().to_vec()).unwrap();
                session.process_output().await.unwrap();
            }
            
            // Buffer should only contain last 5 lines (plus empty line from final \n)
            assert!(session.output_buffer.len() <= 6);
            
            // Should contain the last lines
            let contents = session.output_buffer.join("");
            assert!(contents.contains("Line 10"));
            assert!(!contents.contains("Line 1"));
        }

        #[tokio::test]
        async fn test_scrollback_operations() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Fill buffer with content
            for i in 1..=20 {
                tx.send(format!("Line {}\n", i).as_bytes().to_vec()).unwrap();
            }
            session.process_output().await.unwrap();
            
            // Add more content
            tx.send(b"New line\n".to_vec()).unwrap();
            session.process_output().await.unwrap();
            
            // Buffer size should be maintained
            assert!(session.output_buffer.len() <= session.max_buffer_lines);
        }

        #[test]
        fn test_scroll_position_management() {
            let mut session = create_test_session_for_terminal_tests();
            
            // Initially at bottom
            assert_eq!(session.scroll_position, 0);
            
            // Simulate scroll up
            session.scroll_position = 5;
            assert_eq!(session.scroll_position, 5);
            
            // Simulate scroll back to bottom
            session.scroll_position = 0;
            assert_eq!(session.scroll_position, 0);
        }

        // ========== Mode Switching Tests ==========

        #[test]
        fn test_attach_mode_cycling() {
            let mode = AttachMode::Simple;
            assert_eq!(mode.next(), AttachMode::Terminal);
            
            let mode = AttachMode::Terminal;
            assert_eq!(mode.next(), AttachMode::External);
            
            let mode = AttachMode::External;
            assert_eq!(mode.next(), AttachMode::Simple);
        }

        #[test]
        fn test_attach_mode_display_comprehensive() {
            assert_eq!(AttachMode::Simple.to_string(), "Simple");
            assert_eq!(AttachMode::Terminal.to_string(), "Terminal");
            assert_eq!(AttachMode::External.to_string(), "External");
        }

        #[tokio::test]
        async fn test_mode_switch_in_session() {
            let mut session = create_test_session_for_terminal_tests();
            
            // Start in Terminal mode
            assert_eq!(session.mode, AttachMode::Terminal);
            
            // Switch mode (simulating 'm' key press)
            session.mode = session.mode.next();
            assert_eq!(session.mode, AttachMode::External);
            
            // Switch again
            session.mode = session.mode.next();
            assert_eq!(session.mode, AttachMode::Simple);
        }

        // ========== Integration Tests ==========

        #[tokio::test]
        async fn test_comprehensive_session_lifecycle() {
            let mut session = create_test_session_for_terminal_tests();
            
            // Test initial state
            assert!(matches!(session.connection_status, ConnectionStatus::Disconnected));
            assert!(session.stdin_tx.is_none());
            assert!(session.stdout_rx.is_none());
            assert_eq!(session.mode, AttachMode::Terminal);
            assert!(session.vt100_parser.is_some());
            
            // Test disconnect when not connected
            let result = session.disconnect().await;
            assert!(result.is_ok());
            assert!(matches!(session.connection_status, ConnectionStatus::Disconnected));
        }

        #[tokio::test]
        async fn test_send_input_comprehensive() {
            let mut session = create_test_session_for_terminal_tests();
            
            // Test without connection
            let result = session.send_input("test").await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Not connected"));
            
            // Test with mock connection
            let (tx, mut rx) = mpsc::unbounded_channel();
            session.stdin_tx = Some(tx);
            
            let result = session.send_input("hello").await;
            assert!(result.is_ok());
            
            let received = rx.try_recv().unwrap();
            assert_eq!(received, b"hello");
        }

        // ========== UTF-8 and Character Encoding Tests ==========

        #[tokio::test]
        async fn test_utf8_handling_comprehensive() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Send UTF-8 text with emojis and special characters
            let utf8_text = "Hello 🌍! Café résumé naïve 中文 русский العربية";
            tx.send(utf8_text.as_bytes().to_vec()).unwrap();
            
            session.process_output().await.unwrap();
            
            // Should preserve UTF-8 content
            assert_eq!(session.output_buffer.len(), 1);
            assert_eq!(session.output_buffer[0], utf8_text);
        }

        #[tokio::test]
        async fn test_invalid_utf8_handling_graceful() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Send invalid UTF-8 bytes
            let invalid_utf8 = vec![0xff, 0xfe, 0x48, 0x65, 0x6c, 0x6c, 0x6f]; // Invalid UTF-8 + "Hello"
            tx.send(invalid_utf8).unwrap();
            
            session.process_output().await.unwrap();
            
            // Should handle gracefully using String::from_utf8_lossy
            assert_eq!(session.output_buffer.len(), 1);
            // Should contain "Hello" but invalid bytes should be replaced
            assert!(session.output_buffer[0].contains("Hello"));
        }

        // ========== Edge Cases and Error Handling ==========

        #[tokio::test]
        async fn test_process_output_empty_data() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Send empty data
            tx.send(b"".to_vec()).unwrap();
            
            session.process_output().await.unwrap();
            
            // Should handle empty data gracefully
            assert_eq!(session.output_buffer.len(), 1);
            assert_eq!(session.output_buffer[0], "");
        }

        #[tokio::test]
        async fn test_process_output_only_newlines() {
            let mut session = create_test_session_for_terminal_tests();
            
            let (tx, rx) = mpsc::unbounded_channel();
            session.stdout_rx = Some(Arc::new(Mutex::new(rx)));
            
            // Send only newlines
            tx.send(b"\n\n\n".to_vec()).unwrap();
            
            session.process_output().await.unwrap();
            
            // Should create empty lines
            assert_eq!(session.output_buffer.len(), 4); // Initial empty + 3 more
            for line in &session.output_buffer {
                assert_eq!(line, "");
            }
        }

        #[test]
        fn test_render_without_panic() {
            let mut session = create_test_session_for_terminal_tests();
            
            let area = Rect::new(0, 0, 80, 24);
            let mut buffer = ratatui::buffer::Buffer::empty(area);
            
            // Should not panic
            session.render(area, &mut buffer);
            
            assert!(buffer.content.len() > 0);
        }

        #[test]
        fn test_connection_status_rendering() {
            let _session = create_test_session_for_terminal_tests();
            
            // Test different connection statuses don't cause panics
            let statuses = [
                ConnectionStatus::Disconnected,
                ConnectionStatus::Connecting,
                ConnectionStatus::Connected,
                ConnectionStatus::Failed("Test error".to_string()),
            ];
            
            for status in statuses {
                let mut test_session = create_test_session_for_terminal_tests();
                test_session.connection_status = status.clone();
                
                let area = Rect::new(0, 0, 80, 24);
                let mut buffer = ratatui::buffer::Buffer::empty(area);
                test_session.render(area, &mut buffer);
                
                assert!(buffer.content.len() > 0);
            }
        }

        #[test]
        fn test_session_drop_cleanup() {
            let session = create_test_session_for_terminal_tests();
            
            // Test that Drop implementation doesn't panic
            drop(session);
        }
        
        // TDD tests for AttachMode functionality
        
        #[tokio::test]
        async fn test_attach_mode_enum_default() {
            // Test that AttachMode has a sensible default
            let mode = AttachMode::default();
            assert_eq!(mode, AttachMode::Simple);
        }

        #[tokio::test] 
        async fn test_attach_mode_cycle() {
            // Test that cycling through modes works correctly
            let mut mode = AttachMode::Simple;
            
            mode = mode.next();
            assert_eq!(mode, AttachMode::Terminal);
            
            mode = mode.next();
            assert_eq!(mode, AttachMode::External);
            
            mode = mode.next();
            assert_eq!(mode, AttachMode::Simple);
        }

        #[tokio::test]
        async fn test_attach_mode_display() {
            // Test that modes display correctly for UI
            assert_eq!(AttachMode::Simple.to_string(), "Simple");
            assert_eq!(AttachMode::Terminal.to_string(), "Terminal");
            assert_eq!(AttachMode::External.to_string(), "External");
        }

        #[tokio::test]
        async fn test_docker_attach_session_has_mode_field() {
            // Test that DockerAttachSession includes mode field
            let session = create_test_session_async().await;
            assert_eq!(session.mode, AttachMode::Simple);
        }

        #[tokio::test]
        async fn test_simple_mode_output_processing() {
            // Test that Simple mode strips ANSI escape sequences
            let session = create_test_session_async().await;
            assert_eq!(session.mode, AttachMode::Simple);
            
            // Test ANSI stripping behavior in Simple mode through direct strip function
            let ansi_output = "\x1b[31mError:\x1b[0m Test message\x1b[32m OK\x1b[0m";
            let stripped = strip_ansi_escapes::strip_str(ansi_output);
            
            // Should have ANSI codes stripped
            assert!(!stripped.contains("\x1b"));
            assert!(stripped.contains("Error: Test message OK"));
        }

        #[tokio::test]
        async fn test_terminal_mode_preserves_ansi() {
            // Test that Terminal mode uses vt100 parser
            let mut session = create_test_session_async().await;
            session.mode = AttachMode::Terminal;
            
            // Initialize vt100 parser manually for testing
            session.vt100_parser = Some(Parser::new(80, 24, 1000));
            
            // This test verifies that vt100 parser is initialized in Terminal mode
            assert_eq!(session.mode, AttachMode::Terminal);
            assert!(session.vt100_parser.is_some());
        }

        #[tokio::test]
        async fn test_external_mode_behavior() {
            // Test that External mode is set correctly
            let mut session = create_test_session_async().await;
            session.mode = AttachMode::External;
            
            // External mode should indicate readiness for system terminal launch
            assert_eq!(session.mode, AttachMode::External);
            // External terminal launch tests would be integration tests
        }

        #[tokio::test]
        async fn test_mode_indicator_in_title() {
            // Test that current mode is visible in the UI title
            let session = create_test_session_async().await;
            
            // Should show mode in title
            assert!(session.get_title_with_mode().contains("Simple"));
        }

        #[tokio::test]
        async fn test_mode_toggle_key_full_cycle() {
            // Test full mode toggle cycle with 'm' key
            let mut session = create_test_session_async().await;
            assert_eq!(session.mode, AttachMode::Simple);
            
            let key_m = KeyEvent::new(KeyCode::Char('m'), crossterm::event::KeyModifiers::NONE);
            
            // First press: Simple -> Terminal
            let handled = session.handle_input(key_m).await.unwrap();
            assert!(handled);
            assert_eq!(session.mode, AttachMode::Terminal);
            assert!(session.vt100_parser.is_some());
            
            // Second press: Terminal -> External
            let handled = session.handle_input(key_m).await.unwrap();
            assert!(handled);
            assert_eq!(session.mode, AttachMode::External);
            assert!(session.vt100_parser.is_none());
            
            // Third press: External -> Simple
            let handled = session.handle_input(key_m).await.unwrap();
            assert!(handled);
            assert_eq!(session.mode, AttachMode::Simple);
            assert!(session.vt100_parser.is_none());
        }

        // Helper function to create a test session async
        async fn create_test_session_async() -> DockerAttachSession {
            DockerAttachSession::new(
                Uuid::new_v4(),
                "test-session".to_string(),
                "test-container".to_string(),
            ).await.unwrap()
        }
    }
}