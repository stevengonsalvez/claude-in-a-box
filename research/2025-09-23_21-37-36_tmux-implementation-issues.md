# Research: Tmux Implementation Issues in claude-in-a-box

**Date**: 2025-09-23 21:37:36
**Repository**: claude-in-a-box
**Branch**: terragon/refactor-tmux-host
**Commit**: 624cdf1c
**Research Type**: Codebase Analysis

## Research Question
Why are tmux sessions not working properly? Sessions show as "Detached" with "can't find session" errors, and delete functionality fails trying to delete containers instead of tmux sessions.

## Executive Summary
The tmux implementation is architecturally broken due to incomplete refactoring from Docker to tmux. The core issue is that SessionManager (which manages tmux sessions) is not integrated with the UI display system (which reads from AppState.workspaces). Additionally, SessionLoader has a TODO comment and doesn't actually load tmux sessions, and the delete functionality still tries to use Docker operations.

## Key Findings
1. **SessionLoader doesn't load tmux sessions** - It only loads worktrees and has a TODO comment (`src/app/session_loader.rs:33`)
2. **Dual session storage with no sync** - SessionManager stores sessions separately from AppState.workspaces
3. **Delete function broken** - Returns error immediately trying to use Docker (`src/app/state.rs:2675`)
4. **Sessions created but invisible** - SessionManager creates them but UI reads from different source

## Detailed Findings

### Codebase Analysis

#### Critical Issue #1: SessionLoader Not Implemented
**File**: `src/app/session_loader.rs:30-34`
```rust
pub async fn load_active_sessions(&self) -> Result<Vec<Workspace>> {
    info!("Loading active sessions from tmux");

    // TODO: Implement loading from tmux sessions
    // For now, return empty workspaces
```

**Impact**: This is why sessions don't appear! The SessionLoader is responsible for populating the UI's workspace list, but it never queries tmux sessions or the SessionManager.

**What it does instead**:
- Lines 40-71: Only loads orphaned worktrees from disk
- Sets all sessions to `SessionStatus::Stopped` regardless of actual state

#### Critical Issue #2: Dual Session Storage Systems
**SessionManager Storage**: `src/session/manager.rs:13`
```rust
pub struct SessionManager {
    sessions: HashMap<Uuid, Session>,
    tmux_sessions: HashMap<Uuid, TmuxSession>,
    worktree_manager: WorktreeManager,
}
```

**AppState Storage**: `src/app/state.rs:581`
```rust
pub struct AppState {
    pub workspaces: Vec<Workspace>,  // UI reads from here
    pub session_manager: SessionManager,  // Sessions created here
```

**Problem**: When a session is created:
1. SessionManager creates and stores it in its HashMap
2. UI refreshes by calling `load_real_workspaces()`
3. This calls SessionLoader which doesn't read from SessionManager
4. Result: Created sessions exist but UI can't see them

#### Critical Issue #3: Delete Function Using Docker Code
**File**: `src/app/state.rs:2673-2675`
```rust
pub async fn delete_session(&mut self, session_id: Uuid) -> anyhow::Result<()> {
    info!("Deleting session: {}", session_id);
    return Err(anyhow::anyhow!("Docker operations not supported - using tmux instead"));
```

**Impact**: Delete immediately fails with error. The correct implementation exists at `src/session/manager.rs:141-156` but is never called.

#### Critical Issue #4: Attachment Fails Due to Missing Sessions
**File**: `src/app/tmux_handler.rs:14-18`
```rust
let tmux_session_name = self.workspaces
    .iter()
    .flat_map(|w| &w.sessions)
    .find(|s| s.id == session_id)
    .and_then(|s| Some(s.tmux_session_name.clone()));
```

**Problem**: Since sessions aren't in workspaces (due to Issue #1), attachment can't find the tmux_session_name and fails with "Session not found".

### Architecture Insights

#### Current Flow (Broken)
1. User presses 'n' → Creates session in SessionManager
2. UI refreshes → Calls SessionLoader
3. SessionLoader → Only loads worktrees (TODO for tmux)
4. UI shows → Empty or stopped sessions only
5. User presses 'a' → Can't find session in workspaces
6. User presses 'd' → Fails with Docker error

#### Intended Flow
1. User presses 'n' → Creates session in SessionManager
2. UI refreshes → Should query SessionManager or tmux directly
3. UI shows → Active tmux sessions with correct status
4. User presses 'a' → Finds session and attaches
5. User presses 'd' → Kills tmux and removes worktree

## Code References
- `src/app/session_loader.rs:33` - TODO: Implement loading from tmux sessions
- `src/app/state.rs:2675` - Delete returns error instead of using SessionManager
- `src/app/state.rs:626` - SessionManager field exists but not integrated
- `src/session/manager.rs:141-156` - Working cleanup_session() method unused
- `src/app/tmux_handler.rs:14-18` - Attachment looks in wrong place for sessions
- `src/components/session_list.rs:59` - UI reads from workspaces not SessionManager

## Recommendations

### Fix #1: Implement SessionLoader tmux Integration
**File**: `src/app/session_loader.rs`
```rust
pub async fn load_active_sessions(&self) -> Result<Vec<Workspace>> {
    // Get tmux sessions
    let tmux_sessions = TmuxSession::list_sessions().await?;

    // Load from SessionManager or query tmux directly
    // Match tmux sessions with worktrees
    // Set correct status (Running/Attached/Detached)
}
```

### Fix #2: Replace delete_session Implementation
**File**: `src/app/state.rs:2673`
```rust
pub async fn delete_session(&mut self, session_id: Uuid) -> anyhow::Result<()> {
    info!("Deleting session: {}", session_id);

    // Use the working SessionManager implementation
    self.session_manager.cleanup_session(session_id).await?;

    // Reload workspaces to update UI
    self.load_real_workspaces().await;
    self.ui_needs_refresh = true;

    Ok(())
}
```

### Fix #3: Integrate SessionManager with Workspaces
Either:
- **Option A**: Make SessionLoader query SessionManager for sessions
- **Option B**: Sync SessionManager changes to workspaces after operations
- **Option C**: Eliminate dual storage - use only SessionManager

### Fix #4: Complete tmux Handler Integration
Ensure tmux_handler looks in SessionManager for sessions instead of only checking workspaces.

## Open Questions
1. Why was SessionManager created but not integrated with the UI?
2. Should sessions be stored in one place or synchronized between two?
3. Was the Docker-to-tmux refactor left incomplete intentionally?

## References
- Internal docs: `/Users/stevengonsalvez/d/git/claude-in-a-box/plans/tmux-host-refactor.md`
- Related code: All files in `src/tmux/`, `src/session/`, `src/app/`
- The claude-squad reference implementation wasn't accessible but the current code shows clear architectural issues

## Immediate Actions Required

1. **Quick Fix for Sessions Not Showing**:
   - Implement `SessionLoader::load_active_sessions()` to query tmux
   - Or make it read from SessionManager instead of just worktrees

2. **Quick Fix for Delete**:
   - Replace the delete_session method to use SessionManager::cleanup_session()

3. **Quick Fix for Attachment**:
   - Either populate workspaces properly or make attachment check SessionManager

These issues stem from an incomplete refactor where the Docker infrastructure was partially replaced but key integration points were missed.