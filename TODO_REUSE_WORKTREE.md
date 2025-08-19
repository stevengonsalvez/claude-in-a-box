# TODO: Session Restart/Recreate Feature Implementation

## Feature Request (CORRECTED UNDERSTANDING)
When boss mode sessions are finished (stopped), provide an explicit option to RESTART/RECREATE that specific session. This should:
- Reuse the SAME worktree that session was using
- Create a NEW container for that session
- Preserve all git state and file changes in the worktree
- Be an EXPLICIT trigger (not automatic)
- Default behavior unchanged (new sessions still create new worktrees)

## Implementation Plan

### Phase 1: Test-First Development
- [ ] Write tests for restarting/recreating a stopped session
- [ ] Write tests for preserving worktree state during restart
- [ ] Write tests for both Interactive and Boss mode session restarts
- [ ] Write tests for error cases (worktree missing, session not stopped, etc.)

### Phase 2: Core Implementation
- [ ] Add `recreate_session()` method to SessionLifecycleManager
- [ ] Method should create new container using session's existing worktree
- [ ] Update session with new container ID and set status to Running
- [ ] Handle cleanup of old container if it still exists

### Phase 3: UI Integration
- [ ] Add "Restart" or "Recreate" action for stopped sessions in UI
- [ ] Show this option only for sessions in Stopped status
- [ ] Add keyboard shortcut for restart action (e.g., 'r' key)
- [ ] Update session list to show restart option for stopped sessions

### Phase 4: Testing & Documentation
- [ ] Test complete workflow: create -> stop -> restart
- [ ] Ensure git state preservation works correctly
- [ ] Test with both Interactive and Boss modes
- [ ] Update documentation with restart functionality

## Technical Details

### Current Session Lifecycle
1. `create_session()` - creates new session with new worktree + container
2. `start_session()` - starts existing container for a session
3. `stop_session()` - stops container for a session (worktree remains)
4. `remove_session()` - removes container + worktree + session

### New Method Needed
- `recreate_session(session_id)` - creates new container for existing session with existing worktree

### Key Workflow
1. User creates session -> new worktree + container (UNCHANGED)
2. User stops session -> container stops, worktree remains (UNCHANGED)
3. User restarts session -> NEW container with SAME worktree (NEW FEATURE)
4. User removes session -> cleanup everything (UNCHANGED)

### UI Changes Needed
- Show "Restart" option for stopped sessions
- Add keyboard shortcut for restart action
- Update session status indicators

## Previous Implementation (INCORRECT)
- ❌ Implemented finding worktrees across different sessions
- ❌ Implemented creating new sessions with any existing worktree
- ❌ This was not what the user requested

## Correct Implementation (TO DO)
- ✅ Focus on restarting specific stopped sessions
- ✅ Use the session's own worktree (not finding others)
- ✅ Explicit user action required
- ✅ Default behavior unchanged
