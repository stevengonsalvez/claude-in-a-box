# Tmux Integration Fixes Implementation Plan

## Overview
Complete the incomplete Docker-to-tmux refactor by properly integrating SessionManager with the UI display system. The core issue is that SessionManager creates and manages tmux sessions correctly, but they never appear in the UI because SessionLoader has a TODO comment and doesn't query tmux sessions.

## Current State Analysis
Based on research conducted on 2025-09-23, the tmux implementation has four critical issues:
1. **SessionLoader doesn't load tmux sessions** - TODO at `src/app/session_loader.rs:33`
2. **Dual session storage with no sync** - SessionManager and AppState.workspaces are disconnected
3. **Delete function uses Docker code** - Returns error at `src/app/state.rs:2675`
4. **Sessions created but invisible** - UI reads from wrong source

### Key Discoveries:
- SessionManager implementation works correctly (`src/session/manager.rs`)
- TmuxSession can create/attach/kill sessions (`src/tmux/session.rs`)
- SessionPersistence exists but is never used (`src/session/persistence.rs`)
- UI expects data in `state.workspaces` but SessionManager stores in separate HashMap

## Desired End State
Users can create tmux sessions that immediately appear in the UI, attach to them interactively, delete them cleanly, and have sessions persist across app restarts.

### Verification:
- Press 'n' → Session appears in list immediately
- Press 'a' → Attaches to tmux session successfully
- Press 'd' → Deletes tmux session and worktree cleanly
- Restart app → Previous sessions restored with correct status

## What We're NOT Doing
- Not refactoring the entire architecture (keeping dual storage for now)
- Not implementing advanced features (session templates, auto-recovery)
- Not changing the UI components (they work correctly)
- Not modifying TmuxSession core functionality (it works)

## Implementation Approach
Fix the integration points between working components rather than rewriting them. SessionManager and TmuxSession work correctly - they just need to be connected to the UI layer.

---

## Phase 1: Fix Session Visibility in UI

### Overview
Make tmux sessions appear in the UI by implementing the TODO in SessionLoader to query actual tmux sessions.

### Changes Required:

#### 1. Implement SessionLoader tmux Integration
**File**: `src/app/session_loader.rs`
**Changes**: Replace TODO at line 30-86 with actual implementation

```rust
pub async fn load_active_sessions(&self) -> Result<Vec<Workspace>> {
    info!("Loading active sessions from tmux");

    // Get list of running tmux sessions
    let tmux_sessions = TmuxSession::list_sessions().await.unwrap_or_default();
    info!("Found {} tmux sessions", tmux_sessions.len());

    // Load worktrees
    let worktrees = self.worktree_manager.list_all_worktrees().await?;

    // Group sessions by source repository
    let mut workspace_map: HashMap<String, Vec<Session>> = HashMap::new();

    // Process tmux sessions
    for tmux_name in &tmux_sessions {
        // Extract session info from tmux name (format: ciab_workspace_timestamp)
        if let Some(session) = self.create_session_from_tmux(&tmux_name, &worktrees).await {
            let workspace_key = session.workspace_path.clone();
            workspace_map.entry(workspace_key)
                .or_insert_with(Vec::new)
                .push(session);
        }
    }

    // Also add orphaned worktrees as stopped sessions
    for worktree in worktrees {
        if !tmux_sessions.iter().any(|t| worktree.path.to_string_lossy().contains(&t.replace("ciab_", ""))) {
            if let Some(session) = self.create_session_from_worktree(worktree).await {
                let workspace_key = session.workspace_path.clone();
                workspace_map.entry(workspace_key)
                    .or_insert_with(Vec::new)
                    .push(session);
            }
        }
    }

    // Convert to workspace format
    let workspaces: Vec<Workspace> = workspace_map
        .into_iter()
        .map(|(path, sessions)| {
            let name = Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            Workspace {
                id: Uuid::new_v4(),
                name,
                path: path.clone(),
                current_branch: "main".to_string(), // Will be updated by git operations
                sessions,
                last_accessed: Utc::now(),
                source_type: WorkspaceSourceType::Repository,
            }
        })
        .collect();

    Ok(workspaces)
}

async fn create_session_from_tmux(&self, tmux_name: &str, worktrees: &[Worktree]) -> Option<Session> {
    // Parse tmux session name to extract details
    let name_without_prefix = tmux_name.strip_prefix("ciab_").unwrap_or(tmux_name);

    // Find matching worktree
    let matching_worktree = worktrees.iter()
        .find(|w| name_without_prefix.contains(&w.name));

    if let Some(worktree) = matching_worktree {
        let mut session = Session::new(worktree.name.clone(), worktree.source_path.to_string_lossy().to_string());
        session.tmux_session_name = tmux_name.to_string();
        session.worktree_path = worktree.path.to_string_lossy().to_string();
        session.branch_name = worktree.branch.clone();
        session.status = SessionStatus::Running; // Tmux session exists, so it's running

        // Check if attached
        if let Ok(output) = Command::new("tmux")
            .args(&["list-clients", "-t", tmux_name])
            .output() {
            if output.status.success() && !output.stdout.is_empty() {
                session.status = SessionStatus::Attached;
            }
        }

        Some(session)
    } else {
        None
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] Code compiles: `cargo build`
- [x] Tmux sessions listed: `tmux list-sessions | grep ciab_`
- [x] Sessions appear in HashMap: Add debug logging

#### Manual Verification:
- [ ] Create session with 'n' - appears in UI immediately
- [ ] Session shows correct status (Running/Attached/Detached)
- [ ] Multiple sessions display correctly
- [ ] Orphaned worktrees show as Stopped

---

## Phase 2: Fix Delete Functionality

### Overview
Replace Docker-based deletion code with SessionManager cleanup that kills tmux and removes worktrees.

### Changes Required:

#### 1. Fix delete_session in AppState
**File**: `src/app/state.rs`
**Changes**: Replace method at line 2673

```rust
pub async fn delete_session(&mut self, session_id: Uuid) -> anyhow::Result<()> {
    info!("Deleting session: {}", session_id);

    // Find the session in workspaces to get tmux name
    let tmux_session_name = self.workspaces
        .iter()
        .flat_map(|w| &w.sessions)
        .find(|s| s.id == session_id)
        .map(|s| s.tmux_session_name.clone());

    // Kill tmux session if it exists
    if let Some(tmux_name) = tmux_session_name {
        if let Err(e) = self.kill_tmux_session(&tmux_name).await {
            warn!("Failed to kill tmux session: {}", e);
        }
    }

    // Remove worktree
    let worktree_path = self.workspaces
        .iter()
        .flat_map(|w| &w.sessions)
        .find(|s| s.id == session_id)
        .map(|s| s.worktree_path.clone());

    if let Some(path) = worktree_path {
        let worktree_manager = WorktreeManager::new();
        if let Err(e) = worktree_manager.remove_worktree(session_id).await {
            warn!("Failed to remove worktree: {}", e);
        }
    }

    // Remove from UI state
    for workspace in &mut self.workspaces {
        workspace.sessions.retain(|s| s.id != session_id);
    }

    // Reload workspaces to sync state
    self.load_real_workspaces().await;
    self.ui_needs_refresh = true;

    info!("Session {} deleted successfully", session_id);
    Ok(())
}

async fn kill_tmux_session(&self, tmux_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    Command::new("tmux")
        .args(&["kill-session", "-t", tmux_name])
        .status()?;
    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [x] Delete compiles: `cargo build`
- [x] No Docker error messages in logs
- [x] Tmux session killed: `! tmux has-session -t {name}`

#### Manual Verification:
- [ ] Press 'd' on session - confirmation dialog appears
- [ ] Confirm delete - session disappears from UI
- [ ] Tmux session no longer exists
- [ ] Worktree removed from filesystem

---

## Phase 3: Add Session Persistence

### Overview
Integrate SessionPersistence to save sessions on creation and restore on app startup.

### Changes Required:

#### 1. Add Persistence to SessionManager
**File**: `src/session/manager.rs`
**Changes**: Add persistence field and integrate save/restore

```rust
pub struct SessionManager {
    sessions: HashMap<Uuid, Session>,
    tmux_sessions: HashMap<Uuid, TmuxSession>,
    worktree_manager: WorktreeManager,
    persistence: SessionPersistence, // Add this field
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            tmux_sessions: HashMap::new(),
            worktree_manager: WorktreeManager::new(),
            persistence: SessionPersistence::new().unwrap_or_else(|_| {
                warn!("Failed to create SessionPersistence");
                SessionPersistence::default()
            }),
        }
    }

    pub async fn create_session_with_id(...) -> Result<Uuid, Box<dyn std::error::Error>> {
        // ... existing creation code ...

        // Save to persistence after successful creation
        if let Err(e) = self.persistence.save_session(&session) {
            warn!("Failed to persist session: {}", e);
        }

        Ok(session_id)
    }

    pub async fn restore_sessions(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Restoring sessions from persistence");

        let persisted_sessions = self.persistence.restore_sessions()?;
        let tmux_sessions = TmuxSession::list_sessions().await?;

        for persisted in persisted_sessions {
            // Check if tmux session still exists
            if tmux_sessions.contains(&persisted.tmux_session_name) {
                // Convert to Session and restore
                let mut session = Session::from(persisted);
                session.status = SessionStatus::Detached; // Default to detached

                // Check if attached
                if let Ok(output) = Command::new("tmux")
                    .args(&["list-clients", "-t", &session.tmux_session_name])
                    .output() {
                    if output.status.success() && !output.stdout.is_empty() {
                        session.status = SessionStatus::Attached;
                    }
                }

                self.sessions.insert(session.id, session);
            }
        }

        Ok(())
    }
}
```

#### 2. Restore Sessions on App Init
**File**: `src/app/state.rs`
**Changes**: Add restoration in App::init (around line 685)

```rust
impl App {
    pub async fn init(&mut self) {
        // ... existing Claude integration ...

        // Restore persisted sessions
        if let Err(e) = self.state.session_manager.restore_sessions().await {
            warn!("Failed to restore sessions: {}", e);
        }

        // Load workspaces (this now includes restored sessions)
        self.state.load_real_workspaces().await;

        // ... rest of init ...
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Sessions saved to `~/.claude-box/sessions/`
- [ ] JSON files created with session data
- [ ] Restoration loads saved sessions

#### Manual Verification:
- [ ] Create session, quit app, restart - session appears
- [ ] Session status preserved across restarts
- [ ] Killed tmux sessions don't restore

---

## Phase 4: Fix Attachment Issues

### Overview
Ensure sessions can be found and attached to by fixing the lookup mechanism.

### Changes Required:

#### 1. Update Attachment Handler
**File**: `src/app/tmux_handler.rs`
**Changes**: Improve session lookup at line 14-18

```rust
pub fn attach_to_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
    // Try to find session in workspaces first
    let session_info = self.workspaces
        .iter()
        .flat_map(|w| &w.sessions)
        .find(|s| s.id == session_id)
        .map(|s| (s.tmux_session_name.clone(), s.name.clone()));

    // If not found in workspaces, check SessionManager
    let session_info = session_info.or_else(|| {
        self.session_manager
            .get_session(session_id)
            .map(|s| (s.tmux_session_name.clone(), s.name.clone()))
    });

    if let Some((tmux_name, display_name)) = session_info {
        info!("Attaching to session: {} ({})", display_name, tmux_name);

        // ... rest of attachment logic ...
    } else {
        Err("Session not found in workspaces or SessionManager".into())
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Attachment finds sessions in either location
- [ ] Tmux attach command executes successfully

#### Manual Verification:
- [ ] Press 'a' on session - attaches immediately
- [ ] Interactive tmux session works
- [ ] Ctrl+Q detaches properly
- [ ] Can reattach after detaching

---

## Testing Strategy

### Unit Tests:
- SessionLoader correctly identifies tmux sessions
- Delete removes tmux session and worktree
- Persistence saves/loads correctly

### Integration Tests:
- Full session lifecycle (create → attach → detach → delete)
- Session restoration across app restarts
- Multiple concurrent sessions

### Manual Testing Steps:
1. Create new session with 'n'
2. Verify session appears in list with Running status
3. Attach with 'a' - verify tmux attaches
4. Detach with Ctrl+Q - verify returns to UI
5. Delete with 'd' - verify cleanup
6. Quit and restart - verify sessions restore

## Performance Considerations
- SessionLoader queries tmux on every refresh (consider caching)
- Persistence I/O on every session creation (consider batching)
- Tmux list-sessions called frequently (minimal overhead)

## Migration Notes
- Existing tmux sessions will be discovered on first run
- No data migration needed - works with existing sessions
- Persistence directory created automatically

## References
- Research document: `/research/2025-09-23_21-37-36_tmux-implementation-issues.md`
- Original refactor plan: `/plans/tmux-host-refactor.md`
- SessionManager: `src/session/manager.rs`
- SessionLoader: `src/app/session_loader.rs:30-86`
- Delete function: `src/app/state.rs:2673`