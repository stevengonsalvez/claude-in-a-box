# Fix Tmux Detach (Ctrl+Q) and Split Screen Implementation Plan

## Overview
Fix two critical tmux integration issues: (1) Ctrl+Q doesn't properly detach and return to TUI, and (2) Split screen captures from the wrong session. The root causes are a blocking attach implementation and incorrect session targeting in split screen.

## Current State Analysis
- **Blocking Attach**: The current implementation uses `Command::status()` which blocks the entire TUI until tmux exits
- **PTY Implementation**: Exists but I/O forwarding is not implemented (TODOs in code)
- **Split Screen**: Captures from selected session instead of attached session
- **Session Management**: Properly tracks sessions but attach mechanism blocks UI

## Desired End State
- Ctrl+Q detaches from tmux and returns control to TUI immediately
- Split screen shows live content from the attached session
- TUI remains responsive while attached to tmux sessions
- Users can monitor other sessions while attached

### Key Discoveries:
- Two competing attach implementations exist: blocking (tmux_handler.rs) and PTY-based (tmux/session.rs)
- PTY implementation has proper architecture but missing I/O forwarding (src/tmux/session.rs:203-204)
- Split screen captures from `get_selected_session()` not `attached_session_id` (src/components/split_screen.rs:91)

## What We're NOT Doing
- Not removing the existing blocking attach (keep as fallback initially)
- Not implementing PTY resize handling (can be added later)
- Not changing the overall tmux session management architecture
- Not modifying boss mode or Docker-related code

## Implementation Approach
We'll implement a non-blocking attach using the existing PTY infrastructure, then fix split screen to capture from the correct session. This maintains backward compatibility while fixing the critical issues.

## Phase 1: Implement PTY I/O Forwarding

### Overview
Complete the PTY-based attach implementation by adding the missing I/O forwarding code.

### Changes Required:

#### 1. Fix Input Forwarding
**File**: `src/tmux/session.rs`
**Changes**: Replace TODO at lines 203-204 with actual PTY write

```rust
// Line 203-204: Replace TODO with actual forwarding
if let Some(master) = &self.master {
    if let Ok(mut writer) = master.take_writer() {
        let _ = writer.write_all(&buf[..n]);
    }
}
```

#### 2. Fix Output Forwarding
**File**: `src/tmux/session.rs`
**Changes**: Replace sleep loop at lines 221-225 with PTY read

```rust
// Line 221-225: Replace sleep with actual PTY reading
if let Some(master) = &master_clone {
    if let Ok(mut reader) = master.take_reader() {
        let mut output = vec![0u8; 4096];
        loop {
            tokio::select! {
                result = reader.read(&mut output) => {
                    match result {
                        Ok(n) if n > 0 => {
                            // Send to stdout
                            let _ = tokio::io::stdout().write_all(&output[..n]).await;
                            let _ = tokio::io::stdout().flush().await;
                        }
                        _ => break,
                    }
                }
                _ = detach_rx.recv() => {
                    break;
                }
            }
        }
    }
}
```

#### 3. Store PTY Master Reference
**File**: `src/tmux/session.rs`
**Changes**: Add field to store PTY master at line ~20

```rust
pub struct TmuxSession {
    // ... existing fields ...
    master: Option<Box<dyn portable_pty::MasterPty + Send>>,
}
```

### Success Criteria:

#### Automated Verification:
- [x] Code compiles: `cargo build`
- [x] Tests pass: `cargo test tmux`
- [ ] No clippy warnings: `cargo clippy`

#### Manual Verification:
- [ ] Can type in attached tmux session
- [ ] Can see output from tmux session
- [ ] Terminal responds to commands

---

## Phase 2: Switch to Non-Blocking Attach

### Overview
Replace the blocking attach in tmux_handler with the PTY-based implementation.

### Changes Required:

#### 1. Modify Attach Method
**File**: `src/app/tmux_handler.rs`
**Changes**: Replace blocking command at lines 47-52

```rust
pub async fn attach_to_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
    // ... existing session lookup and state update ...

    // Get tmux session from session manager
    let detach_rx = self.session_manager
        .get_tmux_session_mut(session_id)
        .ok_or("Tmux session not found")?
        .attach()
        .await?;

    // Store that we're attached
    self.attached_session_id = Some(session_id);

    // Don't block - return immediately
    // The detach will be handled by the main event loop
    Ok(())
}
```

#### 2. Handle Detach in Main Loop
**File**: `src/main.rs`
**Changes**: Add detach monitoring after line 300

```rust
// Check for detach from attached session
if let Some(attached_id) = app.state.attached_session_id {
    if let Some(tmux_session) = app.state.session_manager.get_tmux_session(attached_id) {
        if !tmux_session.is_attached() {
            // Session was detached
            app.state.attached_session_id = None;
            app.state.current_view = View::SessionList;

            // Update session status
            if let Some(session) = app.state.get_session_by_id_mut(attached_id) {
                session.status = SessionStatus::Detached;
            }
        }
    }
}
```

#### 3. Add Session Manager Access Methods
**File**: `src/session/manager.rs`
**Changes**: Add getter methods

```rust
pub fn get_tmux_session(&self, session_id: Uuid) -> Option<&TmuxSession> {
    self.tmux_sessions.get(&session_id)
}

pub fn get_tmux_session_mut(&mut self, session_id: Uuid) -> Option<&mut TmuxSession> {
    self.tmux_sessions.get_mut(&session_id)
}
```

### Success Criteria:

#### Automated Verification:
- [x] Build succeeds: `cargo build`
- [x] Integration tests pass: `cargo test test_tmux_attach`

#### Manual Verification:
- [ ] Attach to session doesn't block TUI
- [ ] Can still interact with tmux session
- [ ] Ctrl+Q detaches and returns to session list
- [ ] Session status updates correctly

---

## Phase 3: Fix Split Screen Session Targeting

### Overview
Make split screen capture from the attached session instead of the selected session.

### Changes Required:

#### 1. Update Capture Method
**File**: `src/components/split_screen.rs`
**Changes**: Modify capture_tmux_content at lines 90-107

```rust
pub async fn capture_tmux_content(&mut self, state: &AppState) {
    // Prioritize attached session over selected session
    let target_session = if let Some(attached_id) = state.attached_session_id {
        state.workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == attached_id)
    } else {
        state.get_selected_session()
    };

    if let Some(session) = target_session {
        match state.session_manager.capture_session_pane(session.id).await {
            Ok(content) => {
                self.captured_content = content;
                self.last_update = Some(std::time::Instant::now());
            }
            Err(_) => {
                self.captured_content = "Failed to capture tmux content".to_string();
                self.last_update = Some(std::time::Instant::now());
            }
        }
    } else {
        self.captured_content = "No session attached or selected".to_string();
        self.last_update = Some(std::time::Instant::now());
    }
}
```

#### 2. Update Display Title
**File**: `src/components/split_screen.rs`
**Changes**: Update render_tmux_content to show attached vs selected

```rust
fn render_tmux_content(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
    let is_attached = state.attached_session_id.is_some() &&
        state.attached_session_id == state.get_selected_session().map(|s| s.id);

    let title = if is_attached {
        "Live Session View (Attached)"
    } else {
        "Live Session View (Selected)"
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    // ... rest of rendering unchanged ...
}
```

### Success Criteria:

#### Automated Verification:
- [x] Compilation successful: `cargo build`
- [x] Split screen tests pass: `cargo test split_screen`

#### Manual Verification:
- [ ] Split screen shows content from attached session
- [ ] When no session attached, shows selected session
- [ ] Title indicates whether viewing attached or selected
- [ ] Content updates live as you work in tmux

---

## Phase 4: Add Terminal Mode Management

### Overview
Properly handle terminal raw mode when attaching/detaching to prevent terminal corruption.

### Changes Required:

#### 1. Terminal Mode in PTY Attach
**File**: `src/tmux/session.rs`
**Changes**: Add terminal mode handling in attach method

```rust
pub async fn attach(&mut self) -> Result<oneshot::Receiver<()>, TmuxError> {
    // ... existing PTY setup ...

    // Set terminal to raw mode for PTY interaction
    crossterm::terminal::enable_raw_mode()
        .map_err(|e| TmuxError::AttachFailed(e.to_string()))?;

    // ... rest of attach implementation ...
}

pub async fn detach(&mut self) -> Result<(), TmuxError> {
    // ... existing detach logic ...

    // Restore terminal mode
    let _ = crossterm::terminal::disable_raw_mode();

    Ok(())
}
```

### Success Criteria:

#### Manual Verification:
- [ ] Terminal doesn't get corrupted after detach
- [ ] Can type normally after returning to TUI
- [ ] Terminal properly switches modes during attach/detach

---

## Testing Strategy

### Unit Tests:
- Test PTY I/O forwarding works correctly
- Test detach signal propagation
- Test session state transitions

### Integration Tests:
1. Create tmux session
2. Attach using new non-blocking method
3. Send input and verify output
4. Press Ctrl+Q and verify detach
5. Verify session still exists after detach

### Manual Testing Steps:
1. Start the application
2. Create a new tmux session
3. Attach to the session (press 'a' or Enter)
4. Verify you can interact with tmux
5. Press Ctrl+Q to detach
6. Verify immediate return to session list
7. Toggle split screen (press 'v')
8. Verify split screen shows attached session content
9. Run commands in tmux and verify live updates in split screen

## Performance Considerations
- PTY I/O is async and non-blocking
- Split screen capture runs every 250ms (configurable)
- No blocking operations in main UI thread

## Migration Notes
- Existing sessions continue to work
- Blocking attach remains available as fallback if needed
- No changes to session persistence or storage

## References
- Original research: `research/2025-09-28_tmux-detach-splitscreen-issues.md`
- PTY implementation: `src/tmux/session.rs:134-234`
- Blocking attach: `src/app/tmux_handler.rs:12-79`
- Split screen: `src/components/split_screen.rs:90-107`