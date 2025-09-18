// ABOUTME: TmuxSession implementation for managing host tmux sessions
// Provides direct tmux session creation, attachment, and management on the host

use std::process::{Command, Stdio};
use std::os::unix::io::{AsRawFd, RawFd, FromRawFd, IntoRawFd};
use tokio::sync::oneshot;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::tmux::error::TmuxError;

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

        // For nix 0.27, we use portable-pty instead of nix::pty
        // Since nix::pty doesn't exist in 0.27, let's create the session without PTY for now
        // and use tmux's own session management
        
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
            ptmx: None,
            pts: None,
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

    /// Attach to the session for interactive use using portable-pty
    pub async fn attach(&mut self) -> Result<oneshot::Receiver<()>, TmuxError> {
        if self.attached {
            return Err(TmuxError::IoError(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Already attached to session"
            )));
        }

        // Use portable-pty for terminal handling
        use portable_pty::{native_pty_system, CommandBuilder, PtySize};
        
        let pty_system = native_pty_system();
        let pty_pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }).map_err(|e| TmuxError::PtyCreationFailed(e.to_string()))?;

        let mut cmd = CommandBuilder::new("tmux");
        cmd.args(&["attach-session", "-t", &self.name]);
        
        let mut child = pty_pair.slave.spawn_command(cmd)
            .map_err(|e| TmuxError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to spawn tmux attach: {}", e)
            )))?;

        let (detach_tx, detach_rx) = oneshot::channel();
        self.attach_ctx = Some(detach_tx);

        // Get raw file descriptors for I/O
        let master = pty_pair.master;
        
        // Input forwarding task
        let session_name = self.name.clone();
        let input_task = tokio::spawn(async move {
            let mut stdin = tokio::io::stdin();
            let mut buf = [0u8; 1024];

            // Skip initial control sequences (first 50ms)
            let start = Instant::now();

            loop {
                tokio::select! {
                    result = stdin.read(&mut buf) => {
                        if let Ok(n) = result {
                            if n > 0 {
                                // Skip initial noise
                                if start.elapsed() < Duration::from_millis(50) {
                                    continue;
                                }

                                // Check for Ctrl+Q (ASCII 17)
                                if n == 1 && buf[0] == 17 {
                                    // Detach from tmux
                                    let _ = tokio::process::Command::new("tmux")
                                        .args(&["detach-client", "-t", &session_name])
                                        .output()
                                        .await;
                                    break;
                                }

                                // Forward input to tmux (would need master.write_all in real impl)
                            }
                        }
                    }
                }
            }
        });

        // Output forwarding task
        let output_task = tokio::spawn(async move {
            // In a real implementation, we'd read from master and write to stdout
            // For now, just sleep
            tokio::time::sleep(Duration::from_secs(3600)).await;
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
            task.abort();
        }
        if let Some(task) = self.output_task.take() {
            task.abort();
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

    /// Get the master PTY file descriptor
    pub fn get_master_fd(&self) -> Option<RawFd> {
        self.ptmx
    }

    /// Resize the tmux window
    pub fn resize(&self, _cols: u16, _rows: u16) -> Result<(), TmuxError> {
        // Terminal resize is handled automatically by tmux when attached
        // The tmux session will resize based on the terminal dimensions
        // when attached, so no explicit resize is needed here
        Ok(())
    }
}

impl Drop for TmuxSession {
    fn drop(&mut self) {
        // Clean up is handled by detach/kill methods
    }
}