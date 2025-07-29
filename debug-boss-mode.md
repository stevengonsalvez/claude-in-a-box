# Debug Boss Mode Issue - Testing Instructions

## Summary
I've added comprehensive debug logging to trace the boss mode prompt input issue. The enhanced logging will help us identify exactly where the flow is breaking.

## Debug Logging Added

### Event Handling (`src/app/events.rs`)
- **InputPrompt step**: Debug logs for all key events and prompt validation
- **ConfigurePermissions step**: Debug logs for Enter key handling  
- **Event processing**: Logs when `NewSessionProceedToPermissions` and `NewSessionCreate` events are processed

### State Management (`src/app/state.rs`)
- **new_session_proceed_to_permissions()**: Logs step transitions
- **new_session_create()**: Logs session creation flow and step validation
- **create_session_with_logs()**: Logs Docker container creation process

## Test Instructions

### Step 1: Run with Debug Logging
```bash
cd /Users/stevengonsalvez/d/git/claude-in-a-box
RUST_LOG=debug ./target/release/claude-box 2>&1 | tee debug-output.log
```

### Step 2: Reproduce the Issue
1. Press `n` to create a new session
2. Select any Git repository (use arrow keys, press Enter)
3. Enter any branch name (press Enter)
4. Select **Boss Mode** (use arrow keys, press Enter)
5. **CRITICAL STEP**: Type a prompt like "Analyze this codebase"
6. Press **Enter** (this is where it should fail)

### Step 3: Check Debug Output

Look for these debug messages in the terminal or `debug-output.log`:

#### Expected Flow (if working):
```
DEBUG InputPrompt: Received key event: Enter with modifiers: ...
DEBUG InputPrompt: Enter detected, checking prompt validity
DEBUG InputPrompt: Current prompt content: 'Analyze this codebase' (length: 21)
INFO  InputPrompt: Prompt is valid (21), proceeding to permissions
INFO  Processing NewSessionProceedToPermissions event
INFO  new_session_proceed_to_permissions called
DEBUG Current session state step: InputPrompt
INFO  Advancing from InputPrompt to ConfigurePermissions
```

#### If It Fails at Step Transition:
```
DEBUG InputPrompt: Received key event: Enter with modifiers: ...
WARN  InputPrompt: Prompt is empty, not proceeding
```
OR
```
ERROR InputPrompt: No session state found, cannot proceed
```

#### If It Fails at Permissions Step:
```
INFO  Processing NewSessionProceedToPermissions event
ERROR Cannot proceed to permissions - no session state found
```
OR
```
WARN  Cannot proceed to permissions - not in InputPrompt step (current: ...)
```

#### If It Fails at Container Creation:
```
DEBUG ConfigurePermissions: Enter pressed, creating new session
INFO  Processing NewSessionCreate event - queueing async action
INFO  new_session_create called
WARN  Cannot create session - not in ConfigurePermissions step (current: ...)
```

## Key Debug Points

### 1. Prompt Input Validation
- Check if prompt content is being captured correctly
- Verify prompt is not empty when Enter is pressed

### 2. Event Flow
- Verify `NewSessionProceedToPermissions` event is generated
- Check if `new_session_proceed_to_permissions()` is called
- Confirm step transitions from `InputPrompt` → `ConfigurePermissions`

### 3. Session Creation
- Verify `NewSessionCreate` event is generated in permissions step
- Check if `new_session_create()` is called
- Confirm Docker container creation is attempted

## Expected Results

### If Debug Logging Shows:
- **Events are generated but not processed**: Issue in event processing loop
- **Step doesn't transition**: Issue in state management
- **Container creation fails**: Issue in Docker integration
- **No events generated**: Issue in key event handling

## Next Steps

Based on the debug output, we can:
1. **Identify the exact failure point**
2. **Fix the specific issue** (event handling, state management, or Docker)
3. **Verify the fix** with another test run

Run this test and share the debug output - we'll quickly identify and fix the root cause! 🔍

---
**Generated**: 2025-01-27 by Claude Code
**Status**: Ready for testing with enhanced debug logging