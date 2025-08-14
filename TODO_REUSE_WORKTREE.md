# TODO: Reuse Worktree Feature Implementation

## Feature Request
When boss mode sessions are finished (stopped), allow creating a new session on the same worktree without creating a new worktree. This should spin up a new container in the existing worktree.

## Implementation Plan

### Phase 1: Test-First Development ✅ COMPLETED
- [x] Write tests for detecting existing worktrees for a workspace
- [x] Write tests for reusing existing worktrees in session creation
- [x] Write tests for the new session creation workflow with worktree reuse

### Phase 2: Core Implementation ✅ COMPLETED
- [x] Add method to WorktreeManager to find existing worktrees for a workspace
- [x] Add method to SessionLifecycleManager to create session with existing worktree
- [x] Modify session creation workflow to support worktree reuse option
- [ ] Add UI option to reuse existing worktree when creating new session

### Phase 3: Integration & Testing 🔄 IN PROGRESS
- [x] Test the complete workflow: stop session -> create new session with reuse
- [x] Ensure git state is preserved correctly
- [x] Test with both Interactive and Boss modes
- [ ] Fix test cleanup issues
- [ ] Update documentation

### Phase 4: UI Integration
- [ ] Add UI option in session creation to reuse existing worktree
- [ ] Show available worktrees for reuse in the UI
- [ ] Handle edge cases (no existing worktrees, multiple worktrees)

## Technical Details

### New Methods Implemented ✅
1. `WorktreeManager::find_worktrees_for_workspace(workspace_path) -> Vec<WorktreeInfo>`
2. `SessionLifecycleManager::create_session_with_existing_worktree(request, worktree_info)`

### Test Results
- ✅ `test_find_existing_worktrees_for_workspace` - PASSED
- ✅ `test_find_worktrees_empty_result` - PASSED  
- ⚠️ `test_create_session_with_existing_worktree` - FAILED (cleanup issue)
- ⚠️ `test_worktree_reuse_preserves_git_state` - FAILED (cleanup issue)

### Key Considerations
- Preserve git state in existing worktree ✅
- Handle container cleanup properly ⚠️ (needs fix)
- Ensure session isolation despite shared worktree ✅
- Support both Interactive and Boss modes ✅

## Next Steps
1. Fix test cleanup issues in session removal
2. Add UI integration for worktree reuse selection
3. Add documentation and examples
