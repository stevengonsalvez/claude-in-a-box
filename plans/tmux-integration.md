# Tmux Integration Implementation Plan

**Created**: 2025-10-19
**Status**: Planning
**Estimated Duration**: 2 weeks
**Complexity**: High

## Executive Summary

Convert claude-in-a-box's interactive session model from Docker containers to direct tmux sessions, mirroring the claude-squad architecture. This enables a split-pane TUI with live preview and seamless attach/detach functionality without container overhead.

## Problem Statement

**Current State**:
- Sessions run in Docker containers
- "Attach" just shows container logs (not truly interactive)
- No live preview of what's happening inside containers
- Heavy overhead for simple interactive sessions

**Desired State**:
- Sessions run in native tmux sessions (no containers)
- Live preview pane showing tmux output in real-time
- True attach/detach with Ctrl+Q
- Scroll mode for reviewing history
- Lightweight, fast, responsive

## Research Findings

### claude-squad Architecture Analysis

**Core Components**:
1. **TmuxSession** (`session/tmux/tmux.go`): 527 lines
   - PTY management using `creack/pty`
   - Session lifecycle: create → start → attach → detach → cleanup
   - Content capture via `tmux capture-pane -p -e -J`
   - Detach key: Ctrl+Q (ASCII 17)

2. **Preview Mechanism** (`ui/preview.go`): 270 lines
   - 100ms update interval
   - Two modes: normal (truncated) and scroll (full history)
   - Viewport-based scrolling in scroll mode
   - Auto-scroll in normal mode

3. **Attach Flow** (`session/tmux/tmux.go:269-344`):
   - Create channel for detach signaling
   - Launch goroutine: copy PTY → stdout
   - Launch goroutine: stdin → PTY, listen for Ctrl+Q
   - On Ctrl+Q: close PTY, create new PTY for preview, signal completion

4. **Key Features**:
   - Session naming: `claudesquad_{sanitized_name}`
   - History limit: 10,000 lines
   - Mouse scrolling enabled
   - Detach warning on abnormal exit
   - Preview captures last N lines or full history with `-S -`

### claude-in-a-box Current State

**Session Model** (`src/models/session.rs`):
- Tracks container ID, status, workspace path
- Docker-centric lifecycle
- No tmux integration

**Attach Mechanism** (`src/app/state.rs:1390`):
- Just shows attached session ID
- Displays container logs
- No true terminal interaction

**Components**:
- `attached_terminal.rs`: Static info display
- `log_streaming.rs`: Container log fetching
- No preview pane with live content

## Detailed Implementation Plan

### Phase 1: Core Tmux Module (Foundation)

**Objective**: Create Rust tmux session management module

**Files to Create**:

#### 1.1 `src/tmux/mod.rs`
```rust
pub mod session;
pub mod pty_wrapper;
pub mod capture;

pub use session::{TmuxSession, SessionStatus};
pub use pty_wrapper::PtyWrapper;
pub use capture::CaptureOptions;
```

**Success Criteria**:
- [ ] Module compiles and exports public API
- [ ] No external dependencies yet visible to rest of codebase

#### 1.2 `src/tmux/session.rs`
**Core struct**:
```rust
pub struct TmuxSession {
    sanitized_name: String,
    program: String,              // "claude", "aider", etc.
    pty: Option<PtyWrapper>,      // Current PTY connection
    attach_state: AttachState,
}

enum AttachState {
    Detached,
    Attached { cancel_tx: mpsc::Sender<()> },
}
```

**Key Methods**:
- `new(name: String, program: String) -> Self`
- `async fn start(&mut self, work_dir: &Path) -> Result<()>`
- `async fn attach(&mut self) -> Result<mpsc::Receiver<()>>`
- `async fn detach(&mut self) -> Result<()>`
- `async fn capture_pane_content(&self) -> Result<String>`
- `async fn capture_full_history(&self) -> Result<String>`
- `fn does_session_exist(&self) -> bool`
- `async fn cleanup(&mut self) -> Result<()>`

**Implementation Details**:
- Session name sanitization: `tmux_` prefix, replace spaces/dots
- Use `tokio::process::Command` for tmux commands
- PTY management via `portable-pty` crate
- Detach key detection: scan stdin for byte 0x11 (Ctrl+Q)

**Success Criteria**:
- [ ] Can create tmux session via `tmux new-session -d`
- [ ] Can capture pane content via `tmux capture-pane -p`
- [ ] Can detect Ctrl+Q during attach
- [ ] Cleanup kills tmux session properly

#### 1.3 `src/tmux/pty_wrapper.rs`
**Purpose**: Abstract PTY operations

```rust
pub struct PtyWrapper {
    master: Box<dyn portable_pty::MasterPty>,
    reader: tokio::sync::Mutex<Box<dyn Read + Send>>,
    writer: tokio::sync::Mutex<Box<dyn Write + Send>>,
}

impl PtyWrapper {
    pub fn start(cmd: Command) -> Result<Self>
    pub async fn read(&self, buf: &mut [u8]) -> Result<usize>
    pub async fn write(&self, buf: &[u8]) -> Result<usize>
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()>
}
```

**Success Criteria**:
- [ ] Can spawn tmux attach command
- [ ] Can read/write to PTY
- [ ] Can resize PTY

#### 1.4 `src/tmux/capture.rs`
**Purpose**: Content capture utilities

```rust
pub struct CaptureOptions {
    pub start_line: Option<String>,  // "-" for start of history
    pub end_line: Option<String>,    // "-" for end of history
    pub include_escape_sequences: bool,
    pub join_wrapped_lines: bool,
}

pub async fn capture_pane(
    session_name: &str,
    options: CaptureOptions,
) -> Result<String>
```

**Success Criteria**:
- [ ] Can capture visible pane content
- [ ] Can capture full scrollback history
- [ ] Preserves ANSI escape codes

#### 1.5 Update `Cargo.toml`
```toml
[dependencies]
portable-pty = "0.8"
tokio = { version = "1", features = ["process", "io-util", "sync"] }
```

**Success Criteria**:
- [ ] Dependencies resolve
- [ ] Project compiles

---

### Phase 2: Session Model Updates

**Objective**: Extend Session model for tmux integration

**File**: `src/models/session.rs`

**Changes**:

#### 2.1 Add Fields
```rust
pub struct Session {
    // ... existing fields ...

    // Tmux integration
    pub tmux_session_name: Option<String>,
    pub preview_content: Option<String>,
    pub is_attached: bool,
}
```

#### 2.2 Add Methods
```rust
impl Session {
    pub fn get_tmux_name(&self) -> String {
        format!("tmux_{}", self.name.replace(' ', "_"))
    }

    pub fn set_preview(&mut self, content: String) {
        self.preview_content = Some(content);
    }

    pub fn mark_attached(&mut self) {
        self.is_attached = true;
    }

    pub fn mark_detached(&mut self) {
        self.is_attached = false;
    }
}
```

**Success Criteria**:
- [ ] Session model extends without breaking existing code
- [ ] Serialization/deserialization works with new fields

---

### Phase 3: Preview Pane Component

**Objective**: Create TUI component for tmux preview

**File**: `src/components/tmux_preview.rs`

**Component Structure**:
```rust
pub struct TmuxPreviewPane {
    width: u16,
    height: u16,
    preview_mode: PreviewMode,
    viewport: Viewport,  // For scroll mode
}

pub enum PreviewMode {
    Normal,              // Last N lines, auto-scrolling
    Scroll { offset: usize },  // Full history, manual scroll
}
```

**Features**:
- Display last N lines in normal mode
- Switch to scroll mode on Shift+Up/Down
- Mouse wheel support
- Footer: "ESC to exit scroll mode | ↑↓ to scroll"
- Update interval: 100ms

**Rendering**:
```rust
impl TmuxPreviewPane {
    pub fn render(&self, frame: &mut Frame, area: Rect, session: &Session) {
        match &session.preview_content {
            Some(content) => self.render_content(frame, area, content),
            None => self.render_placeholder(frame, area),
        }
    }

    fn render_content(&self, frame: &mut Frame, area: Rect, content: &str) {
        match self.preview_mode {
            PreviewMode::Normal => {
                // Show last N lines
                let lines = content.lines().collect::<Vec<_>>();
                let visible = lines.iter().rev().take(area.height as usize);
                // ...
            }
            PreviewMode::Scroll { offset } => {
                // Use viewport
                self.viewport.render(frame, area, content, offset);
            }
        }
    }
}
```

**Success Criteria**:
- [ ] Displays tmux content in TUI
- [ ] Scroll mode works with keyboard/mouse
- [ ] Gracefully handles empty content
- [ ] Updates smoothly (no flicker)

---

### Phase 4: Attach/Detach Handler

**Objective**: Implement seamless TUI suspend/resume for tmux attachment

**File**: `src/app/attach_handler.rs`

**Core Logic**:
```rust
pub struct AttachHandler {
    terminal: Arc<Mutex<Terminal<CrosstermBackend<io::Stdout>>>>,
}

impl AttachHandler {
    pub async fn attach_session(
        &self,
        session: &mut TmuxSession,
    ) -> Result<()> {
        // 1. Suspend Ratatui
        self.suspend_tui().await?;

        // 2. Attach to tmux PTY
        let detach_rx = session.attach().await?;

        // 3. Wait for detach signal (Ctrl+Q)
        detach_rx.await;

        // 4. Resume Ratatui
        self.resume_tui().await?;

        Ok(())
    }

    async fn suspend_tui(&self) -> Result<()> {
        let mut terminal = self.terminal.lock().await;
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen
        )?;
        terminal.show_cursor()?;
        Ok(())
    }

    async fn resume_tui(&self) -> Result<()> {
        let mut terminal = self.terminal.lock().await;
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::EnterAlternateScreen
        )?;
        terminal.hide_cursor()?;
        terminal.clear()?;
        Ok(())
    }
}
```

**Success Criteria**:
- [ ] TUI suspends cleanly (no artifacts)
- [ ] Tmux session takes full terminal control
- [ ] Ctrl+Q returns to TUI without corruption
- [ ] Terminal state fully restored

---

### Phase 5: AppState Integration

**Objective**: Wire tmux sessions into application state management

**File**: `src/app/state.rs`

**Changes**:

#### 5.1 Add Tmux Session Manager
```rust
pub struct AppState {
    // ... existing fields ...

    tmux_sessions: HashMap<Uuid, TmuxSession>,
    preview_update_task: Option<JoinHandle<()>>,
}
```

#### 5.2 Replace Container Methods
```rust
impl AppState {
    // Replace attach_to_container with:
    pub async fn attach_to_tmux_session(&mut self, session_id: Uuid) -> Result<()> {
        if let Some(tmux_session) = self.tmux_sessions.get_mut(&session_id) {
            let attach_handler = AttachHandler::new(self.terminal.clone());
            attach_handler.attach_session(tmux_session).await?;

            // Update session state
            if let Some(session) = self.find_session_mut(session_id) {
                session.mark_detached();
            }
        }
        Ok(())
    }

    // New method for preview updates
    async fn start_preview_updates(&mut self) {
        let sessions = self.tmux_sessions.clone();
        let update_interval = Duration::from_millis(100);

        self.preview_update_task = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(update_interval);
            loop {
                interval.tick().await;
                for (id, session) in sessions.iter() {
                    if let Ok(content) = session.capture_pane_content().await {
                        // Send update to main thread
                        // ...
                    }
                }
            }
        }));
    }
}
```

#### 5.3 Update Session Creation
```rust
pub async fn create_new_session(&mut self, name: String, workspace_path: String) -> Result<Uuid> {
    let session = Session::new(name.clone(), workspace_path.clone());
    let session_id = session.id;

    // Create tmux session
    let mut tmux_session = TmuxSession::new(
        session.get_tmux_name(),
        "claude".to_string(),
    );

    // Setup git worktree
    let worktree_path = self.setup_git_worktree(&session)?;

    // Start tmux session in worktree
    tmux_session.start(&worktree_path).await?;

    // Store session
    self.add_session(workspace_path, session);
    self.tmux_sessions.insert(session_id, tmux_session);

    Ok(session_id)
}
```

**Success Criteria**:
- [ ] Creating session spawns tmux session
- [ ] Preview updates run automatically
- [ ] Attach/detach works from UI
- [ ] Killing session cleans up tmux

---

### Phase 6: UI Updates

**Objective**: Update UI components for tmux workflow

**Files**:
- `src/components/session_list.rs`
- `src/components/layout.rs`
- `src/app/events.rs`

**Changes**:

#### 6.1 Session List (`session_list.rs`)
```rust
// Update session display to show tmux status
pub fn render_session_item(&self, session: &Session) -> Line {
    let status_indicator = if session.is_attached {
        "🔗"  // Attached
    } else if session.tmux_session_name.is_some() {
        "●"   // Running
    } else {
        "○"   // Stopped
    };

    Line::from(vec![
        Span::raw(status_indicator),
        Span::raw(" "),
        Span::styled(session.name.clone(), Style::default().bold()),
        // ...
    ])
}
```

#### 6.2 Layout (`layout.rs`)
```rust
// Replace log viewer with tmux preview
pub fn render_main_view(&self, frame: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),  // Session list
            Constraint::Percentage(60),  // Tmux preview
        ])
        .split(frame.size());

    self.session_list.render(frame, chunks[0], state);
    self.tmux_preview.render(frame, chunks[1], state);  // NEW
}
```

#### 6.3 Events (`events.rs`)
```rust
// Update event handling
pub enum AppEvent {
    // ... existing ...

    AttachTmuxSession,
    DetachTmuxSession,
    EnterScrollMode,
    ExitScrollMode,
    ScrollPreviewUp,
    ScrollPreviewDown,
}

pub fn handle_key_event(key: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('a'), KeyModifiers::NONE) => Some(AppEvent::AttachTmuxSession),
        (KeyCode::Char('q'), KeyModifiers::CONTROL) => Some(AppEvent::DetachTmuxSession),
        (KeyCode::Up, KeyModifiers::SHIFT) => Some(AppEvent::ScrollPreviewUp),
        (KeyCode::Down, KeyModifiers::SHIFT) => Some(AppEvent::ScrollPreviewDown),
        (KeyCode::Esc, _) => Some(AppEvent::ExitScrollMode),
        // ...
    }
}
```

**Success Criteria**:
- [ ] Session list shows tmux status
- [ ] Preview pane displays in main layout
- [ ] Keyboard shortcuts work as expected
- [ ] UI feels responsive

---

### Phase 7: Git Worktree Integration

**Objective**: Ensure git worktrees work with tmux sessions

**File**: `src/git/worktree_manager.rs`

**Changes**:

```rust
pub async fn create_worktree_for_tmux(&self, session_name: &str) -> Result<PathBuf> {
    let branch_name = format!("claude/{}", session_name);

    // Create worktree
    let worktree_path = self.worktrees_dir.join(session_name);

    Command::new("git")
        .args(["worktree", "add", "-b", &branch_name])
        .arg(&worktree_path)
        .output()
        .await?;

    Ok(worktree_path)
}

pub async fn cleanup_worktree(&self, session_name: &str) -> Result<()> {
    let worktree_path = self.worktrees_dir.join(session_name);

    // Remove worktree
    Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(&worktree_path)
        .output()
        .await?;

    // Prune
    Command::new("git")
        .args(["worktree", "prune"])
        .output()
        .await?;

    Ok(())
}
```

**Success Criteria**:
- [ ] Worktree created before tmux starts
- [ ] Tmux session starts in worktree directory
- [ ] Cleanup removes worktree and prunes

---

### Phase 8: Configuration & Documentation

**Objective**: Make tmux behavior configurable and documented

**Files**:
- `src/config/mod.rs`
- `README.md`
- `docs/tmux-integration.md`

**Config Schema** (`config/mod.rs`):
```rust
#[derive(Deserialize, Serialize)]
pub struct TmuxConfig {
    #[serde(default = "default_detach_key")]
    pub detach_key: String,

    #[serde(default = "default_update_interval")]
    pub preview_update_interval_ms: u64,

    #[serde(default = "default_history_limit")]
    pub history_limit: u32,

    #[serde(default = "default_mouse_scroll")]
    pub enable_mouse_scroll: bool,
}

fn default_detach_key() -> String { "ctrl-q".to_string() }
fn default_update_interval() -> u64 { 100 }
fn default_history_limit() -> u32 { 10000 }
fn default_mouse_scroll() -> bool { true }
```

**Documentation**:

`docs/tmux-integration.md`:
```markdown
# Tmux Integration

## Overview
Claude-in-a-box uses tmux sessions for interactive code sessions, providing
a split-pane interface with live preview.

## Architecture
- **Left Pane**: Session list
- **Right Pane**: Live tmux preview

## Workflow
1. Press `n` to create new session
2. Session starts in isolated git worktree
3. Preview pane shows live tmux output
4. Press `a` to attach (full terminal takeover)
5. Press `Ctrl+Q` to detach (return to TUI)

## Scroll Mode
- Press `Shift+Up/Down` to enter scroll mode
- View full session history
- Press `ESC` to exit scroll mode

## Configuration
See `.claude-in-a-box/config.toml`:

```toml
[tmux]
detach_key = "ctrl-q"
preview_update_interval_ms = 100
history_limit = 10000
enable_mouse_scroll = true
```

## Troubleshooting
- **Tmux not found**: Install via `brew install tmux` (macOS) or `apt install tmux` (Linux)
- **Orphaned sessions**: Run `tmux ls` and `tmux kill-session -t <name>`
- **Garbled output**: Check terminal TERM variable
```

**Success Criteria**:
- [ ] Configuration loads and applies
- [ ] Documentation is clear and complete
- [ ] Help screen updated

---

## Testing Strategy

### Unit Tests

#### Test: Session Creation
```rust
#[tokio::test]
async fn test_create_tmux_session() {
    let mut session = TmuxSession::new("test_session", "bash");
    session.start(Path::new("/tmp")).await.unwrap();
    assert!(session.does_session_exist());
    session.cleanup().await.unwrap();
}
```

#### Test: Content Capture
```rust
#[tokio::test]
async fn test_capture_pane_content() {
    let mut session = TmuxSession::new("test_capture", "bash");
    session.start(Path::new("/tmp")).await.unwrap();

    let content = session.capture_pane_content().await.unwrap();
    assert!(!content.is_empty());

    session.cleanup().await.unwrap();
}
```

#### Test: Detach Key Detection
```rust
#[tokio::test]
async fn test_detach_key_detection() {
    // Simulate Ctrl+Q in stdin
    let input = vec![0x11u8];  // Ctrl+Q
    let detected = detect_detach_key(&input);
    assert!(detected);
}
```

### Integration Tests

#### Test: Complete Lifecycle
```rust
#[tokio::test]
async fn test_session_lifecycle() {
    let mut state = AppState::new().await.unwrap();

    // Create session
    let session_id = state.create_new_session(
        "test".to_string(),
        "/tmp".to_string()
    ).await.unwrap();

    // Verify tmux session exists
    assert!(state.tmux_sessions.contains_key(&session_id));

    // Preview should update
    tokio::time::sleep(Duration::from_millis(150)).await;
    let session = state.find_session(session_id).unwrap();
    assert!(session.preview_content.is_some());

    // Cleanup
    state.kill_session(session_id).await.unwrap();
    assert!(!state.tmux_sessions.contains_key(&session_id));
}
```

### Manual Testing Checklist

- [ ] **Basic Flow**
  - [ ] Create new session
  - [ ] Preview shows prompt
  - [ ] Type in preview (should update live)
  - [ ] Attach with `a`
  - [ ] Detach with `Ctrl+Q`
  - [ ] Back in TUI

- [ ] **Scroll Mode**
  - [ ] Press `Shift+Up` to enter
  - [ ] Scroll up/down with arrows
  - [ ] Scroll with mouse wheel
  - [ ] Press `ESC` to exit
  - [ ] Preview returns to normal

- [ ] **Multi-Session**
  - [ ] Create 3 sessions
  - [ ] Switch between them
  - [ ] Previews update independently
  - [ ] Attach to each one
  - [ ] All detach correctly

- [ ] **Git Integration**
  - [ ] Session starts in worktree
  - [ ] `git branch` shows session branch
  - [ ] Changes isolated to worktree
  - [ ] Kill cleans up worktree

- [ ] **Error Handling**
  - [ ] Ctrl+D in tmux shows warning
  - [ ] Invalid session name handled
  - [ ] Tmux not installed → clear error
  - [ ] Duplicate session name rejected

- [ ] **Performance**
  - [ ] Preview updates < 10ms lag
  - [ ] Attach/detach instant (<500ms)
  - [ ] Multiple sessions don't slow down

---

## Migration Plan

### For New Sessions
- Default to tmux backend
- No changes needed from user

### For Existing Docker Sessions
Not applicable - starting fresh with tmux

### Rollback Plan
If critical issues arise:
1. Add feature flag: `--use-docker`
2. Keep Docker code path
3. Document known issues
4. Fix in follow-up release

---

## Risk Assessment

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| PTY incompatibility | High | Low | Use battle-tested `portable-pty`, test on Mac/Linux/Windows |
| Tmux not installed | High | Medium | Check at startup, show install instructions |
| Session orphaning | Medium | Medium | Cleanup on exit, periodic orphan detection |
| Performance issues | Medium | Low | Async architecture, configurable update interval |
| Loss of isolation | Low | High | Document security model, recommend VM for untrusted code |

---

## Success Criteria

### Must Have
- ✅ Create tmux session from TUI
- ✅ Live preview with <100ms latency
- ✅ Ctrl+Q detach works reliably
- ✅ Scroll mode functional
- ✅ Git worktrees integrate
- ✅ No session leaks on exit

### Should Have
- ✅ Mouse scroll support
- ✅ Configurable keybindings
- ✅ Status indicators
- ✅ Documentation complete

### Nice to Have
- ⏳ Session templates
- ⏳ Custom prompts
- ⏳ Session history replay

---

## Implementation Checklist

### Phase 1: Core Tmux Module ✅ = Done, 🔄 = In Progress, ⏸ = Pending
- ✅ Create `src/tmux/mod.rs`
- ✅ Implement `src/tmux/session.rs`
- ✅ Implement `src/tmux/pty_wrapper.rs`
- ✅ Implement `src/tmux/capture.rs`
- ✅ Update `Cargo.toml` with dependencies
- ✅ Unit tests for TmuxSession
- ✅ Unit tests for content capture

### Phase 2: Session Model
- ✅ Add tmux fields to Session
- ✅ Add helper methods
- ✅ Update serialization
- ✅ Test with mock data

### Phase 3: Preview Pane
- ✅ Create `src/components/tmux_preview.rs`
- ✅ Implement normal mode rendering
- ✅ Implement scroll mode
- ✅ Add mouse support
- ✅ Add keyboard shortcuts
- ✅ Test rendering edge cases

### Phase 4: Attach/Detach
- ✅ Create `src/app/attach_handler.rs`
- ✅ Implement TUI suspend
- ✅ Implement TUI resume
- ✅ Implement Ctrl+Q detection
- ✅ Test state transitions
- ✅ Test error recovery

### Phase 5: AppState Integration
- ⏸ Add tmux session map
- ⏸ Implement preview update loop
- ⏸ Update session creation
- ⏸ Update session deletion
- ⏸ Integration tests

### Phase 6: UI Updates
- ⏸ Update session list display
- ⏸ Integrate preview pane in layout
- ⏸ Update event handlers
- ⏸ Update help text
- ⏸ Manual UI testing

### Phase 7: Git Worktree
- ⏸ Update worktree creation
- ⏸ Update cleanup logic
- ⏸ Test isolation
- ⏸ Test branch management

### Phase 8: Configuration
- ⏸ Add tmux config section
- ⏸ Implement config loading
- ⏸ Write documentation
- ⏸ Update README
- ⏸ Final testing

---

## Timeline

| Week | Phases | Deliverables |
|------|--------|--------------|
| Week 1 | Phase 1-2 | Core tmux module, updated session model |
| Week 1 | Phase 3-4 | Preview component, attach/detach working |
| Week 2 | Phase 5-6 | Full integration, updated UI |
| Week 2 | Phase 7-8 | Git integration, docs, polish |

**Total Duration**: ~2 weeks
**Review Points**: End of each phase

---

## Next Steps

1. **Immediate**: Begin Phase 1 - Create tmux module structure
2. **Day 1-3**: Complete Phases 1-2 (foundation)
3. **Day 4-6**: Complete Phases 3-4 (UX)
4. **Day 7-9**: Complete Phases 5-6 (integration)
5. **Day 10-12**: Complete Phases 7-8 (polish)
6. **Day 13-14**: Testing and refinement

**Ready to begin implementation? Stevie, approve and I'll start with Phase 1!** 🚀
