# Research: Refactoring CIAB Interactive Mode to tmux-based Architecture

**Date**: 2025-09-14 22:51:00
**Repository**: ciab
**Branch**: feat/tmux-interactive
**Commit**: e36db95
**Research Type**: Comprehensive Architecture Analysis

## Research Question

How to refactor the current Rust TUI implementation from blocking Docker exec calls to a tmux-based session management system similar to claude-squad?

## Executive Summary

The current CIAB implementation uses blocking `docker exec -it` calls that break the TUI state when entering interactive mode. Claude-squad solves this elegantly using tmux sessions with PTY (pseudoterminal) management, enabling seamless attach/detach without disrupting the TUI. This research provides a complete refactoring strategy to adopt the tmux approach in Rust.

## Key Findings

1. **Critical Failure Point**: The blocking `exec_interactive_blocking` function in `/Users/stevengonsalvez/d/git/ciab/src/docker/container_manager.rs:841-926` destroys TUI state
2. **Solution Architecture**: Claude-squad maintains dual PTY connections - one for monitoring, one for interaction
3. **Implementation Path**: Replace Docker exec with tmux session management inside containers, using PTY for non-blocking I/O

## Architecture Comparison

### Current CIAB Architecture (Broken)

```
User Input → TUI (Ratatui) → Docker Exec (BLOCKING) → Container
                    ↓
            TUI State Corrupted
                    ↓
            Cannot Properly Detach
```

**Problems**:
- `disable_raw_mode()` breaks TUI state
- Blocking call freezes entire application
- No way to cleanly detach without killing session
- Terminal state often cannot be restored

### Claude-Squad Architecture (Working)

```
User Input → TUI (Bubble Tea) → PTY → tmux attach → Container
                    ↓                       ↓
            Preview (polling)        Interactive (streaming)
                    ↓                       ↓
            capture-pane             Input/Output forwarding
```

**Benefits**:
- Non-blocking async I/O via PTY
- Clean attach/detach with Ctrl+Q
- TUI state preserved throughout
- Session persistence across detach/reattach

## Detailed Refactoring Plan

### Phase 1: Add tmux Session Management Module

Create new module: `/Users/stevengonsalvez/d/git/ciab/src/tmux/mod.rs`

```rust
// ABOUTME: tmux session management for interactive container sessions
// Provides non-blocking PTY-based communication with tmux sessions inside containers

use nix::pty::{openpty, Winsize};
use std::os::unix::io::{AsRawFd, RawFd};
use tokio::process::Command;

pub struct TmuxSession {
    session_name: String,
    container_id: String,

    // PTY handles
    master_fd: RawFd,
    slave_fd: RawFd,

    // Attach state
    attached: bool,
    attach_handle: Option<tokio::task::JoinHandle<()>>,
    detach_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl TmuxSession {
    /// Create a new tmux session inside a container
    pub async fn create(
        container_id: &str,
        session_name: &str,
        program: &str,
        workdir: &str,
    ) -> Result<Self, TmuxError> {
        // Step 1: Create PTY pair
        let winsize = Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let pty = openpty(Some(&winsize), None)?;

        // Step 2: Execute tmux new-session inside container via docker exec
        let cmd = vec![
            "docker", "exec", container_id,
            "tmux", "new-session", "-d", "-s", session_name,
            "-c", workdir, program,
        ];

        // Step 3: Establish monitoring connection
        let monitor_cmd = vec![
            "docker", "exec", container_id,
            "tmux", "attach-session", "-t", session_name,
        ];

        // Return configured session
        Ok(Self {
            session_name: session_name.to_string(),
            container_id: container_id.to_string(),
            master_fd: pty.master,
            slave_fd: pty.slave,
            attached: false,
            attach_handle: None,
            detach_tx: None,
        })
    }

    /// Capture current pane content for preview
    pub async fn capture_pane(&self) -> Result<String, TmuxError> {
        let output = Command::new("docker")
            .args(&[
                "exec", &self.container_id,
                "tmux", "capture-pane", "-p", "-e", "-J",
                "-t", &self.session_name,
            ])
            .output()
            .await?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Attach to the session for interactive use
    pub async fn attach(&mut self) -> Result<(), TmuxError> {
        if self.attached {
            return Err(TmuxError::AlreadyAttached);
        }

        let (detach_tx, detach_rx) = tokio::sync::oneshot::channel();
        self.detach_tx = Some(detach_tx);

        // Spawn input forwarding task
        let master_fd = self.master_fd;
        let input_task = tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let mut buf = [0u8; 1024];

            loop {
                tokio::select! {
                    // Read from stdin
                    result = stdin.read(&mut buf) => {
                        match result {
                            Ok(n) if n > 0 => {
                                // Check for Ctrl+Q (ASCII 17)
                                if n == 1 && buf[0] == 17 {
                                    // Trigger detach
                                    break;
                                }
                                // Forward to PTY
                                nix::unistd::write(master_fd, &buf[..n])?;
                            }
                            _ => break,
                        }
                    }
                    // Check for detach signal
                    _ = &mut detach_rx => {
                        break;
                    }
                }
            }
        });

        // Spawn output forwarding task
        let output_task = tokio::spawn(async move {
            let mut stdout = tokio::io::stdout();
            let mut buf = [0u8; 4096];

            loop {
                match nix::unistd::read(master_fd, &mut buf) {
                    Ok(n) if n > 0 => {
                        stdout.write_all(&buf[..n]).await?;
                        stdout.flush().await?;
                    }
                    _ => break,
                }
            }
        });

        self.attached = true;
        self.attach_handle = Some(input_task);

        Ok(())
    }

    /// Detach from the session
    pub async fn detach(&mut self) -> Result<(), TmuxError> {
        if !self.attached {
            return Ok(());
        }

        // Signal detach
        if let Some(tx) = self.detach_tx.take() {
            let _ = tx.send(());
        }

        // Wait for tasks to complete
        if let Some(handle) = self.attach_handle.take() {
            handle.await?;
        }

        self.attached = false;
        Ok(())
    }
}
```

### Phase 2: Replace Blocking Docker Exec

Remove the problematic function in `/Users/stevengonsalvez/d/git/ciab/src/docker/container_manager.rs:841-926`

Replace with:

```rust
impl ContainerManager {
    /// Create a tmux session for interactive use
    pub async fn create_tmux_session(
        &self,
        container_id: &str,
        session_name: &str,
    ) -> Result<TmuxSession, ContainerError> {
        // Use the new TmuxSession API
        TmuxSession::create(
            container_id,
            session_name,
            "claude",  // or appropriate program
            "/workspace",
        ).await.map_err(|e| ContainerError::TmuxError(e))
    }
}
```

### Phase 3: Update AppState Session Management

Modify `/Users/stevengonsalvez/d/git/ciab/src/app/state.rs:1390-1460`:

```rust
impl AppState {
    pub async fn handle_enter_pressed(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(session) = self.get_selected_session_mut() {
            // Create or retrieve tmux session
            let tmux_session = self.container_manager
                .create_tmux_session(&session.container_id, &session.name)
                .await?;

            // Store in session state
            session.tmux_session = Some(tmux_session);

            // Attach for interactive use
            session.tmux_session.as_mut().unwrap().attach().await?;

            // No need to disable/enable raw mode!
            // PTY handles everything
        }
        Ok(())
    }

    pub async fn handle_detach(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(session) = self.get_selected_session_mut() {
            if let Some(tmux) = &mut session.tmux_session {
                tmux.detach().await?;
            }
        }
        Ok(())
    }
}
```

### Phase 4: Refactor AttachedTerminal Component

Update `/Users/stevengonsalvez/d/git/ciab/src/components/attached_terminal.rs`:

```rust
pub struct AttachedTerminal {
    tmux_session: Option<TmuxSession>,
    content_cache: String,
    last_update: Instant,
}

impl AttachedTerminal {
    pub async fn update(&mut self) {
        // Poll for content updates every 100ms
        if self.last_update.elapsed() > Duration::from_millis(100) {
            if let Some(tmux) = &self.tmux_session {
                match tmux.capture_pane().await {
                    Ok(content) => self.content_cache = content,
                    Err(_) => {} // Handle error gracefully
                }
            }
            self.last_update = Instant::now();
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        // Render the cached content with ANSI color support
        let content = Paragraph::new(self.content_cache.clone())
            .style(Style::default())
            .block(Block::default().borders(Borders::ALL));

        content.render(area, buf);
    }
}
```

### Phase 5: Add Window Resize Handling

```rust
use signal_hook::consts::SIGWINCH;
use signal_hook_tokio::Signals;

pub async fn monitor_terminal_size(tmux_session: Arc<Mutex<TmuxSession>>) {
    let mut signals = Signals::new(&[SIGWINCH]).unwrap();

    while let Some(signal) = signals.next().await {
        if signal == SIGWINCH {
            // Get new terminal size
            let (cols, rows) = crossterm::terminal::size().unwrap();

            // Update PTY size
            let winsize = Winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };

            let session = tmux_session.lock().await;
            nix::pty::ioctl::TIOCSWINSZ(session.master_fd, &winsize).unwrap();
        }
    }
}
```

## Implementation Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
nix = { version = "0.27", features = ["pty", "term"] }
signal-hook = "0.3"
signal-hook-tokio = { version = "0.3", features = ["futures-v0_3"] }
```

## Migration Strategy

1. **Week 1**: Implement TmuxSession module with basic create/attach/detach
2. **Week 2**: Replace blocking docker exec calls, test with single session
3. **Week 3**: Update UI components for preview/attach modes
4. **Week 4**: Add window resize, scrollback, and error recovery
5. **Week 5**: Comprehensive testing and edge case handling

## Critical Success Factors

1. **Non-blocking I/O**: All tmux operations must be async
2. **Clean Detach**: Ctrl+Q must work reliably without breaking TUI
3. **Session Persistence**: Sessions must survive detach/reattach cycles
4. **Error Recovery**: Graceful handling of tmux/container failures
5. **Performance**: Preview polling should not impact responsiveness

## Testing Strategy

### Unit Tests
- TmuxSession creation/destruction
- PTY read/write operations
- Attach/detach state transitions

### Integration Tests
- Full session lifecycle with real containers
- Multiple concurrent sessions
- Network/container failure scenarios

### End-to-End Tests
- User workflow: create → attach → interact → detach → reattach
- Terminal resize during active session
- Scrollback and history navigation

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| PTY library compatibility | High | Test nix crate PTY support thoroughly |
| tmux not installed in containers | Critical | Ensure tmux in all container images |
| Terminal state corruption | High | Implement robust error recovery |
| Performance with many sessions | Medium | Implement smart polling with backoff |

## Open Questions

1. Should we support multiple tmux windows/panes per session?
2. How to handle container restart with active sessions?
3. Should we implement session recording/replay?
4. What's the best way to handle copy/paste in tmux sessions?

## References

- Current broken implementation: `/Users/stevengonsalvez/d/git/ciab/src/docker/container_manager.rs:841-926`
- Claude-squad tmux implementation: `/Users/stevengonsalvez/d/git/ciab/claude-squad/session/tmux/tmux.go`
- Container tmux script: `/Users/stevengonsalvez/d/git/ciab/docker/claude-dev/scripts/tmux-claude.sh`
- Rust PTY documentation: https://docs.rs/nix/latest/nix/pty/

## Conclusion

The refactoring from blocking Docker exec to tmux-based session management will solve the critical TUI corruption issue while providing a superior user experience with persistent sessions, clean attach/detach, and proper terminal handling. The claude-squad implementation provides a proven architectural pattern that can be successfully adapted to Rust using the nix crate for PTY management and tokio for async I/O.