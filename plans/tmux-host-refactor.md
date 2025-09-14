# Host-Based Tmux Interactive Mode Implementation Plan

## Overview
Complete architectural shift from Docker containers to direct host tmux sessions, following the claude-squad pattern exactly. Sessions run directly on the host machine with tmux managing isolation and persistence.

## Current State Analysis
The application currently uses Docker containers for isolation and session management, with blocking `docker exec -it` calls that corrupt the TUI. Previous attempts to fix this with PTY + docker exec have failed. The solution is to remove containers entirely and use tmux directly on the host.

## Desired End State
Users can create, attach to, and detach from tmux sessions running directly on the host machine. Each session runs in an isolated git worktree with its own environment. Sessions persist across application restarts, and users can seamlessly switch between multiple concurrent sessions using Ctrl+Q to detach.

### Key Architectural Changes:
- Remove all Docker/container dependencies
- Sessions run as tmux sessions on the host
- Git worktrees provide file isolation
- Environment variables provide configuration isolation
- tmux provides terminal multiplexing and persistence

## What We're NOT Doing
- Not using Docker containers at all
- Not implementing container-based isolation
- Not managing container lifecycles
- Not dealing with container networking or volumes
- Not implementing our own terminal multiplexer (using tmux)

## Implementation Approach
Follow claude-squad exactly: create detached tmux sessions on the host, maintain PTY connections for communication, use capture-pane for preview, and attach/detach with proper input forwarding.

## Phase 1: Remove Container Dependencies

### Overview
Strip out all Docker/container-related code and dependencies.

### Changes Required:

#### 1. Update Cargo.toml
**File**: `Cargo.toml`
**Changes**: Remove Docker dependencies, add PTY support

```toml
[dependencies]
# Remove these:
# bollard = "0.16.1"  # Docker API client - REMOVE

# Add these:
nix = { version = "0.27", features = ["pty", "term", "signal", "process"] }
portable-pty = "0.8"
signal-hook = "0.3"
signal-hook-tokio = { version = "0.3", features = ["futures-v0_3"] }
```

#### 2. Remove Docker Modules
**Files to delete**:
- `src/docker/` - entire directory
- `src/config/container.rs` - container configuration
- `docker/` - entire directory with Dockerfiles and scripts

#### 3. Update Session Model
**File**: `src/models/session.rs`
**Changes**: Remove container fields, add tmux fields

```rust
// ABOUTME: Session model for host-based tmux sessions
// Manages tmux sessions running directly on the host machine

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub name: String,
    pub workspace_path: String,
    pub worktree_path: String,  // Git worktree location
    pub branch_name: String,

    // Tmux session info (replaces container_id)
    pub tmux_session_name: String,
    pub tmux_pid: Option<u32>,

    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub git_changes: GitChanges,
    pub recent_logs: Option<String>,

    // Optional environment variables for the session
    pub environment_vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    Created,
    Running,
    Attached,
    Detached,
    Stopped,
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Cargo build succeeds without Docker dependencies: `cargo build`
- [ ] No remaining Docker imports: `rg "use.*docker" src/`
- [ ] No remaining bollard imports: `rg "bollard" src/`

#### Manual Verification:
- [ ] Application starts without Docker daemon
- [ ] No Docker-related errors on startup

---

## Phase 2: Implement Host Tmux Session Management

### Overview
Create the core tmux session management that runs directly on the host.

### Changes Required:

#### 1. Create Tmux Module
**File**: `src/tmux/mod.rs` (new file)
**Changes**: Core tmux module structure

```rust
// ABOUTME: Host-based tmux session management
// Manages tmux sessions running directly on the host machine

pub mod session;
pub mod pty;
pub mod error;

pub use session::TmuxSession;
pub use error::TmuxError;

#[derive(Debug, thiserror::Error)]
pub enum TmuxError {
    #[error("PTY creation failed: {0}")]
    PtyCreationFailed(String),

    #[error("Tmux not installed on host")]
    TmuxNotInstalled,

    #[error("Session already exists: {0}")]
    SessionExists(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

#### 2. Implement TmuxSession for Host
**File**: `src/tmux/session.rs` (new file)
**Changes**: Direct host tmux session management

```rust
use std::process::{Command, Stdio};
use std::os::unix::io::{AsRawFd, RawFd, FromRawFd};
use nix::pty::{openpty, Winsize};
use tokio::sync::oneshot;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct TmuxSession {
    pub name: String,
    pub worktree_path: String,
    pub program: String,  // e.g., "claude", "bash"

    // PTY for communication
    ptmx: Option<RawFd>,  // Master side
    pts: Option<RawFd>,   // Slave side

    // Attach state
    attached: bool,
    attach_ctx: Option<tokio::sync::oneshot::Sender<()>>,
    input_task: Option<tokio::task::JoinHandle<()>>,
    output_task: Option<tokio::task::JoinHandle<()>>,
}

impl TmuxSession {
    /// Check if tmux is installed on the host
    pub fn check_tmux_installed() -> Result<(), TmuxError> {
        let output = Command::new("which")
            .arg("tmux")
            .output()
            .map_err(|_| TmuxError::TmuxNotInstalled)?;

        if !output.status.success() {
            return Err(TmuxError::TmuxNotInstalled);
        }
        Ok(())
    }

    /// Create a new tmux session on the host
    pub async fn create(
        name: &str,
        worktree_path: &str,
        program: &str,
        env_vars: &HashMap<String, String>,
    ) -> Result<Self, TmuxError> {
        Self::check_tmux_installed()?;

        // Sanitize session name
        let session_name = format!("ciab_{}", name.replace(' ', "_").replace('.', "_"));

        // Check if session already exists
        let check = Command::new("tmux")
            .args(&["has-session", &format!("-t={}", session_name)])
            .output()?;

        if check.status.success() {
            return Err(TmuxError::SessionExists(session_name));
        }

        // Create PTY for session
        let winsize = Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let pty = openpty(Some(&winsize), None)
            .map_err(|e| TmuxError::PtyCreationFailed(e.to_string()))?;

        // Build tmux command with environment
        let mut cmd = Command::new("tmux");
        cmd.args(&[
            "new-session",
            "-d",  // Detached
            "-s", &session_name,
            "-c", worktree_path,  // Working directory
        ]);

        // Environment variables are optional - host config is used by default
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        // Add the program to run
        cmd.arg(program);

        // Execute with PTY
        cmd.stdin(unsafe { Stdio::from_raw_fd(pty.slave) })
           .stdout(unsafe { Stdio::from_raw_fd(pty.slave) })
           .stderr(unsafe { Stdio::from_raw_fd(pty.slave) });

        let output = cmd.output()?;

        if !output.status.success() {
            return Err(TmuxError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create tmux session: {}",
                    String::from_utf8_lossy(&output.stderr))
            )));
        }

        // Configure tmux session
        Command::new("tmux")
            .args(&["set-option", "-t", &session_name, "history-limit", "10000"])
            .output()?;

        Command::new("tmux")
            .args(&["set-option", "-t", &session_name, "mouse", "on"])
            .output()?;

        Ok(Self {
            name: session_name,
            worktree_path: worktree_path.to_string(),
            program: program.to_string(),
            ptmx: Some(pty.master),
            pts: Some(pty.slave),
            attached: false,
            attach_ctx: None,
            input_task: None,
            output_task: None,
        })
    }

    /// Capture current pane content for preview
    pub async fn capture_pane(&self) -> Result<String, TmuxError> {
        let output = tokio::process::Command::new("tmux")
            .args(&[
                "capture-pane",
                "-p",  // Print to stdout
                "-e",  // Include escape sequences
                "-J",  // Join wrapped lines
                "-t", &self.name,
            ])
            .output()
            .await?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Attach to the session for interactive use
    pub async fn attach(&mut self) -> Result<oneshot::Receiver<()>, TmuxError> {
        if self.attached {
            return Err(TmuxError::IoError(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Already attached to session"
            )));
        }

        // Create new PTY for attach
        let winsize = Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let pty = openpty(Some(&winsize), None)
            .map_err(|e| TmuxError::PtyCreationFailed(e.to_string()))?;

        self.ptmx = Some(pty.master);
        self.pts = Some(pty.slave);

        // Start tmux attach-session process
        let mut child = tokio::process::Command::new("tmux")
            .args(&["attach-session", "-t", &self.name])
            .stdin(unsafe { Stdio::from_raw_fd(pty.slave) })
            .stdout(unsafe { Stdio::from_raw_fd(pty.slave) })
            .stderr(unsafe { Stdio::from_raw_fd(pty.slave) })
            .spawn()?;

        let (detach_tx, detach_rx) = oneshot::channel();
        self.attach_ctx = Some(detach_tx);

        // Input forwarding task
        let master_fd = pty.master;
        let mut detach_rx_input = detach_rx;
        let input_task = tokio::spawn(async move {
            let mut stdin = tokio::io::stdin();
            let mut buf = [0u8; 1024];

            // Skip initial control sequences (first 50ms)
            let start = std::time::Instant::now();

            loop {
                tokio::select! {
                    result = stdin.read(&mut buf) => {
                        if let Ok(n) = result {
                            if n > 0 {
                                // Skip initial noise
                                if start.elapsed() < std::time::Duration::from_millis(50) {
                                    continue;
                                }

                                // Check for Ctrl+Q (ASCII 17)
                                if n == 1 && buf[0] == 17 {
                                    // Detach from tmux
                                    let detach_seq = b"\x02d";  // Ctrl+B, d
                                    let _ = nix::unistd::write(master_fd, detach_seq);
                                    break;
                                }

                                // Forward input to tmux
                                let _ = nix::unistd::write(master_fd, &buf[..n]);
                            }
                        }
                    }
                    _ = &mut detach_rx_input => {
                        break;
                    }
                }
            }
        });

        // Output forwarding task
        let output_task = tokio::spawn(async move {
            let mut stdout = tokio::io::stdout();
            let mut buf = [0u8; 4096];

            loop {
                match nix::unistd::read(master_fd, &mut buf) {
                    Ok(n) if n > 0 => {
                        let _ = stdout.write_all(&buf[..n]).await;
                        let _ = stdout.flush().await;
                    }
                    _ => break,
                }
            }
        });

        self.attached = true;
        self.input_task = Some(input_task);
        self.output_task = Some(output_task);

        // Return receiver for detach signal
        Ok(detach_rx)
    }

    /// Detach from the session
    pub async fn detach(&mut self) -> Result<(), TmuxError> {
        if !self.attached {
            return Ok(());
        }

        // Signal tasks to stop
        if let Some(tx) = self.attach_ctx.take() {
            let _ = tx.send(());
        }

        // Wait for tasks
        if let Some(task) = self.input_task.take() {
            let _ = task.await;
        }
        if let Some(task) = self.output_task.take() {
            let _ = task.await;
        }

        // Close PTY
        if let Some(fd) = self.ptmx.take() {
            let _ = nix::unistd::close(fd);
        }
        if let Some(fd) = self.pts.take() {
            let _ = nix::unistd::close(fd);
        }

        self.attached = false;
        Ok(())
    }

    /// Kill the tmux session
    pub async fn kill(&mut self) -> Result<(), TmuxError> {
        if self.attached {
            self.detach().await?;
        }

        Command::new("tmux")
            .args(&["kill-session", "-t", &self.name])
            .output()?;

        Ok(())
    }

    /// List all CIAB tmux sessions on the host
    pub async fn list_sessions() -> Result<Vec<String>, TmuxError> {
        let output = tokio::process::Command::new("tmux")
            .args(&["list-sessions", "-F", "#{session_name}"])
            .output()
            .await?;

        if output.status.success() {
            let sessions: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|s| s.starts_with("ciab_"))
                .map(String::from)
                .collect();
            Ok(sessions)
        } else {
            Ok(Vec::new())
        }
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Tmux module compiles: `cargo build`
- [ ] Can detect tmux installation: `which tmux`
- [ ] Type checking passes: `cargo check`

#### Manual Verification:
- [ ] Can create tmux session on host
- [ ] Session appears in `tmux list-sessions`
- [ ] Can capture pane content

---

## Phase 3: Replace Session Lifecycle Management

### Overview
Replace Docker container lifecycle with host tmux session lifecycle.

### Changes Required:

#### 1. Create Session Manager
**File**: `src/session/manager.rs` (new file)
**Changes**: Replace container manager with session manager

```rust
// ABOUTME: Session lifecycle management for host tmux sessions
// Manages creation, attachment, and cleanup of tmux sessions

use crate::tmux::TmuxSession;
use crate::git::WorktreeManager;
use crate::models::{Session, SessionStatus};
use std::collections::HashMap;
use uuid::Uuid;

pub struct SessionManager {
    sessions: HashMap<Uuid, Session>,
    tmux_sessions: HashMap<Uuid, TmuxSession>,
    worktree_manager: WorktreeManager,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            tmux_sessions: HashMap::new(),
            worktree_manager: WorktreeManager::new(),
        }
    }

    pub async fn create_session(
        &mut self,
        workspace_path: &str,
        branch_name: &str,
        session_name: &str,
    ) -> Result<Uuid, Box<dyn std::error::Error>> {
        let session_id = Uuid::new_v4();

        // Create git worktree
        let worktree_path = self.worktree_manager
            .create_worktree(workspace_path, branch_name, &session_name)?;

        // Optional environment variables - Claude CLI uses host config
        let mut env_vars = HashMap::new();
        env_vars.insert("CIAB_SESSION".to_string(), session_name.to_string());
        env_vars.insert("CIAB_WORKTREE".to_string(), worktree_path.clone());

        // Determine program to run (claude if available, else bash)
        let program = if Command::new("which")
            .arg("claude")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "claude"
        } else {
            "/bin/bash -l"
        };

        // Create tmux session
        let tmux_session = TmuxSession::create(
            session_name,
            &worktree_path,
            program,
            &env_vars,
        ).await?;

        // Create session model
        let session = Session {
            id: session_id,
            name: session_name.to_string(),
            workspace_path: workspace_path.to_string(),
            worktree_path: worktree_path.clone(),
            branch_name: branch_name.to_string(),
            tmux_session_name: tmux_session.name.clone(),
            tmux_pid: None,
            status: SessionStatus::Running,
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            git_changes: Default::default(),
            recent_logs: None,
            environment_vars: env_vars,
            claude_api_key: None,  // Uses host configuration
        };

        self.sessions.insert(session_id, session);
        self.tmux_sessions.insert(session_id, tmux_session);

        Ok(session_id)
    }

    pub async fn attach_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        let tmux_session = self.tmux_sessions.get_mut(&session_id)
            .ok_or("Session not found")?;

        tmux_session.attach().await?;

        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.status = SessionStatus::Attached;
            session.last_accessed = chrono::Utc::now();
        }

        Ok(())
    }

    pub async fn detach_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        let tmux_session = self.tmux_sessions.get_mut(&session_id)
            .ok_or("Session not found")?;

        tmux_session.detach().await?;

        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.status = SessionStatus::Detached;
        }

        Ok(())
    }

    pub async fn restore_sessions(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Find existing CIAB tmux sessions
        let existing_sessions = TmuxSession::list_sessions().await?;

        for session_name in existing_sessions {
            // Try to restore session metadata from disk or recreate
            println!("Found existing tmux session: {}", session_name);
            // TODO: Implement session restoration logic
        }

        Ok(())
    }

    pub async fn cleanup_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        // Kill tmux session
        if let Some(mut tmux_session) = self.tmux_sessions.remove(&session_id) {
            tmux_session.kill().await?;
        }

        // Clean up worktree
        if let Some(session) = self.sessions.get(&session_id) {
            self.worktree_manager.remove_worktree(&session.worktree_path)?;
        }

        // Remove from sessions map
        self.sessions.remove(&session_id);

        Ok(())
    }
}
```

#### 2. Update AppState
**File**: `src/app/state.rs`
**Changes**: Replace container manager with session manager

```rust
use crate::session::SessionManager;

pub struct AppState {
    // Remove: container_manager: ContainerManager,
    // Add:
    pub session_manager: SessionManager,

    // ... rest of fields remain the same
}

impl AppState {
    pub fn new() -> Self {
        Self {
            session_manager: SessionManager::new(),
            // ... other fields
        }
    }

    pub async fn create_new_session(&mut self) -> Result<(), Box<dyn Error>> {
        // Get selected workspace
        let workspace = self.get_selected_workspace()?;

        // Generate session name
        let session_name = format!("{}_{}",
            workspace.name,
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );

        // Create session
        let session_id = self.session_manager
            .create_session(
                &workspace.path,
                &workspace.current_branch,
                &session_name,
            )
            .await?;

        self.ui_needs_refresh = true;
        Ok(())
    }

    pub async fn attach_to_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn Error>> {
        self.session_manager.attach_session(session_id).await?;
        self.attached_session_id = Some(session_id);
        self.current_view = View::AttachedTerminal;
        self.ui_needs_refresh = true;
        Ok(())
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Session manager compiles: `cargo build`
- [ ] No container references remain: `rg -i container src/`
- [ ] Tests pass: `cargo test`

#### Manual Verification:
- [ ] Can create session with git worktree
- [ ] Environment variables set correctly in tmux session
- [ ] Claude CLI runs if available, else bash

---

## Phase 4: Update TUI Components

### Overview
Update TUI components to work with host tmux sessions instead of containers.

### Changes Required:

#### 1. Update Session List Component
**File**: `src/components/session_list.rs`
**Changes**: Display host tmux sessions

```rust
impl SessionListComponent {
    pub fn render(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        let sessions: Vec<ListItem> = state.session_manager
            .get_sessions()
            .iter()
            .map(|session| {
                let status_icon = match session.status {
                    SessionStatus::Running => "🟢",
                    SessionStatus::Attached => "🔵",
                    SessionStatus::Detached => "⚪",
                    SessionStatus::Stopped => "🔴",
                    _ => "⚫",
                };

                let line = format!(
                    "{} {} - {} ({})",
                    status_icon,
                    session.name,
                    session.branch_name,
                    session.tmux_session_name,
                );

                ListItem::new(line)
            })
            .collect();

        let list = List::new(sessions)
            .block(Block::default()
                .title(" Sessions (tmux) ")
                .borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.state);
    }
}
```

#### 2. Update AttachedTerminal Component
**File**: `src/components/attached_terminal.rs`
**Changes**: Stream from host tmux session

```rust
pub struct AttachedTerminalComponent {
    content_buffer: Vec<String>,
    scroll_offset: usize,
    last_update: Instant,
}

impl AttachedTerminalComponent {
    pub async fn update(&mut self, session_manager: &mut SessionManager, session_id: Uuid) {
        if self.last_update.elapsed() > Duration::from_millis(100) {
            if let Some(tmux_session) = session_manager.get_tmux_session_mut(session_id) {
                if let Ok(content) = tmux_session.capture_pane().await {
                    self.content_buffer = content.lines().map(String::from).collect();
                }
            }
            self.last_update = Instant::now();
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Terminal (Ctrl+Q to detach) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Render terminal content with ANSI support
        let visible_lines: Vec<Line> = self.content_buffer
            .iter()
            .skip(self.scroll_offset)
            .take(inner.height as usize)
            .map(|line| Line::from(line.as_str()))
            .collect();

        let paragraph = Paragraph::new(visible_lines)
            .style(Style::default())
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, inner);
    }
}
```

#### 3. Update Event Handling
**File**: `src/app/events.rs`
**Changes**: Handle tmux-specific events

```rust
impl EventHandler {
    pub fn handle_key_event(&mut self, key: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        match state.current_view {
            View::SessionList => self.handle_session_list_keys(key, state),
            View::AttachedTerminal => self.handle_attached_terminal_keys(key, state),
            _ => None,
        }
    }

    fn handle_session_list_keys(&mut self, key: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        match key.code {
            KeyCode::Char('n') => Some(AppEvent::CreateSession),
            KeyCode::Char('a') | KeyCode::Enter => Some(AppEvent::AttachSession),
            KeyCode::Char('d') => Some(AppEvent::DeleteSession),
            KeyCode::Char('r') => Some(AppEvent::RefreshSessions),
            KeyCode::Char('q') => Some(AppEvent::Quit),
            _ => None,
        }
    }

    fn handle_attached_terminal_keys(&mut self, key: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        // Ctrl+Q to detach
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
            return Some(AppEvent::DetachSession);
        }

        // All other input goes to tmux
        let bytes = self.key_to_bytes(key);
        Some(AppEvent::SendToTmux(bytes))
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] UI components compile: `cargo build`
- [ ] No container UI references: `rg -i "container\|docker" src/components/`

#### Manual Verification:
- [ ] Session list shows tmux sessions
- [ ] Can navigate session list
- [ ] Terminal content displays correctly
- [ ] Status indicators work

---

## Phase 5: Integration and Polish

### Overview
Complete integration, add session persistence, and polish the experience.

### Changes Required:

#### 1. Add Session Persistence
**File**: `src/session/persistence.rs` (new file)
**Changes**: Save and restore session metadata

```rust
use serde::{Serialize, Deserialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: Uuid,
    pub name: String,
    pub workspace_path: String,
    pub worktree_path: String,
    pub branch_name: String,
    pub tmux_session_name: String,
    pub created_at: DateTime<Utc>,
}

impl SessionManager {
    pub fn save_sessions(&self) -> Result<(), Box<dyn Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("ciab");

        std::fs::create_dir_all(&config_dir)?;

        let sessions_file = config_dir.join("sessions.json");
        let metadata: Vec<SessionMetadata> = self.sessions
            .values()
            .map(|s| SessionMetadata {
                id: s.id,
                name: s.name.clone(),
                workspace_path: s.workspace_path.clone(),
                worktree_path: s.worktree_path.clone(),
                branch_name: s.branch_name.clone(),
                tmux_session_name: s.tmux_session_name.clone(),
                created_at: s.created_at,
            })
            .collect();

        let json = serde_json::to_string_pretty(&metadata)?;
        std::fs::write(sessions_file, json)?;

        Ok(())
    }

    pub fn load_sessions(&mut self) -> Result<(), Box<dyn Error>> {
        let sessions_file = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("ciab")
            .join("sessions.json");

        if !sessions_file.exists() {
            return Ok(());
        }

        let json = std::fs::read_to_string(sessions_file)?;
        let metadata: Vec<SessionMetadata> = serde_json::from_str(&json)?;

        // Match with existing tmux sessions
        let existing_tmux = TmuxSession::list_sessions().await?;

        for meta in metadata {
            if existing_tmux.contains(&meta.tmux_session_name) {
                // Restore session
                let session = Session {
                    id: meta.id,
                    name: meta.name,
                    workspace_path: meta.workspace_path,
                    worktree_path: meta.worktree_path,
                    branch_name: meta.branch_name,
                    tmux_session_name: meta.tmux_session_name,
                    tmux_pid: None,
                    status: SessionStatus::Detached,
                    created_at: meta.created_at,
                    last_accessed: chrono::Utc::now(),
                    git_changes: Default::default(),
                    recent_logs: None,
                    environment_vars: HashMap::new(),
                    claude_api_key: None,
                };

                self.sessions.insert(meta.id, session);
            }
        }

        Ok(())
    }
}
```

#### 2. Add Window Resize Support
**File**: `src/tmux/resize.rs` (new file)
**Changes**: Handle terminal resize

```rust
use signal_hook::consts::SIGWINCH;
use signal_hook_tokio::Signals;

pub async fn monitor_window_resize(session_manager: Arc<Mutex<SessionManager>>) {
    let mut signals = Signals::new(&[SIGWINCH]).unwrap();

    while let Some(signal) = signals.next().await {
        if signal == SIGWINCH {
            let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));

            let winsize = Winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };

            let manager = session_manager.lock().await;
            for tmux_session in manager.get_all_tmux_sessions() {
                if let Some(ptmx) = tmux_session.get_master_fd() {
                    let _ = nix::ioctl_write_ptr!(
                        tiocswinsz,
                        b'T',
                        104,
                        nix::pty::Winsize
                    )(ptmx, &winsize);
                }
            }
        }
    }
}
```

#### 3. Update Main Loop
**File**: `src/main.rs`
**Changes**: Initialize with host tmux

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Check tmux is installed
    TmuxSession::check_tmux_installed()
        .expect("tmux must be installed on the host system");

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app_state = AppState::new();

    // Restore existing sessions
    app_state.session_manager.load_sessions().await?;

    // Start window resize monitor
    let session_manager = Arc::new(Mutex::new(app_state.session_manager));
    tokio::spawn(monitor_window_resize(session_manager.clone()));

    // Main event loop
    loop {
        terminal.draw(|f| ui::draw(f, &app_state))?;

        // Handle events
        if let Event::Key(key) = event::read()? {
            if let Some(app_event) = event_handler.handle_key_event(key, &mut app_state) {
                match app_event {
                    AppEvent::Quit => break,
                    AppEvent::CreateSession => {
                        app_state.create_new_session().await?;
                    }
                    AppEvent::AttachSession => {
                        if let Some(session_id) = app_state.get_selected_session_id() {
                            app_state.attach_to_session(session_id).await?;
                        }
                    }
                    AppEvent::DetachSession => {
                        if let Some(session_id) = app_state.attached_session_id {
                            app_state.session_manager.detach_session(session_id).await?;
                            app_state.attached_session_id = None;
                            app_state.current_view = View::SessionList;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Update attached terminal if needed
        if let Some(session_id) = app_state.attached_session_id {
            if let Some(component) = &mut app_state.attached_terminal_component {
                component.update(&mut app_state.session_manager, session_id).await;
            }
        }
    }

    // Cleanup
    app_state.session_manager.save_sessions()?;
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Full build passes: `cargo build --release`
- [ ] All tests pass: `cargo test --all`
- [ ] Clippy clean: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] Sessions persist across app restarts
- [ ] Window resize updates tmux pane size
- [ ] Clean shutdown saves session state
- [ ] Can restore and reattach to existing sessions

---

## Testing Strategy

### Unit Tests:
- TmuxSession creation/destruction
- Session name sanitization
- PTY operations
- Session persistence serialization

### Integration Tests:
- Full session lifecycle (create → attach → detach → restore)
- Multiple concurrent sessions
- Git worktree operations
- Environment variable propagation

### Manual Testing Steps:
1. Check tmux is installed: `which tmux`
2. Start CIAB: `cargo run`
3. Create new session with 'n'
4. Verify tmux session created: `tmux list-sessions`
5. Attach with 'a' or Enter
6. Verify terminal works, run commands
7. Detach with Ctrl+Q
8. Verify return to session list
9. Quit app with 'q'
10. Restart app and verify sessions restored
11. Reattach to existing session
12. Test window resize
13. Clean up with 'd' to delete session

## Performance Considerations
- No Docker overhead - native host performance
- Tmux handles all terminal multiplexing efficiently
- PTY operations are lightweight
- No container build times or image management
- Git worktrees provide instant file isolation

## Migration Notes
- Users must have tmux installed on their host system
- Existing container-based sessions cannot be migrated
- Environment setup (Claude CLI, API keys) must be configured on host
- Git worktrees will be created in `.worktrees/` subdirectory
- Session metadata stored in `~/.config/ciab/sessions.json`

## Benefits of Host-Based Approach
1. **Simplicity**: No Docker complexity, just tmux
2. **Performance**: Native execution, no virtualization overhead
3. **Persistence**: Tmux sessions survive app restarts naturally
4. **Compatibility**: Works on any Unix system with tmux
5. **Resource Usage**: Minimal - just tmux processes
6. **Debugging**: Can attach to sessions with regular tmux client

## References
- Claude-squad implementation: `claude-squad/session/tmux/`
- Original research: `research/2025-09-14_tmux-interactive-refactor.md`
- Tmux documentation: https://github.com/tmux/tmux/wiki
- PTY handling: https://docs.rs/nix/latest/nix/pty/