// ABOUTME: TmuxSession implementation for managing host tmux sessions
// Provides direct tmux session creation, attachment, and management on the host

use std::process::{Command, Stdio};
use std::os::unix::io::{AsRawFd, RawFd, FromRawFd};
use nix::pty::{openpty, Winsize};
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
        let mut _child = tokio::process::Command::new("tmux")
            .args(&["attach-session", "-t", &self.name])
            .stdin(unsafe { Stdio::from_raw_fd(pty.slave) })
            .stdout(unsafe { Stdio::from_raw_fd(pty.slave) })
            .stderr(unsafe { Stdio::from_raw_fd(pty.slave) })
            .spawn()?;

        let (detach_tx, detach_rx) = oneshot::channel();
        self.attach_ctx = Some(detach_tx);

        // Input forwarding task
        let master_fd = pty.master;
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
                                    let detach_seq = b"\x02d";  // Ctrl+B, d
                                    let _ = nix::unistd::write(master_fd, detach_seq);
                                    break;
                                }

                                // Forward input to tmux
                                let _ = nix::unistd::write(master_fd, &buf[..n]);
                            }
                        }
                    }
                }
            }
        });

        // Output forwarding task
        let output_master_fd = pty.master;
        let output_task = tokio::spawn(async move {
            let mut stdout = tokio::io::stdout();
            let mut buf = [0u8; 4096];

            loop {
                match nix::unistd::read(output_master_fd, &mut buf) {
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

    /// Get the master PTY file descriptor
    pub fn get_master_fd(&self) -> Option<RawFd> {
        self.ptmx
    }

    /// Resize the PTY window
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), TmuxError> {
        if let Some(ptmx) = self.ptmx {
            let winsize = Winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            
            // Use the TIOCSWINSZ ioctl to set window size
            unsafe {
                let ret = libc::ioctl(ptmx, libc::TIOCSWINSZ, &winsize);
                if ret < 0 {
                    return Err(TmuxError::IoError(std::io::Error::last_os_error()));
                }
            }
        }
        Ok(())
    }
}

impl Drop for TmuxSession {
    fn drop(&mut self) {
        // Clean up PTY file descriptors
        if let Some(fd) = self.ptmx.take() {
            let _ = nix::unistd::close(fd);
        }
        if let Some(fd) = self.pts.take() {
            let _ = nix::unistd::close(fd);
        }
    }
}