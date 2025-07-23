# Session Exit Fix Documentation

## Problem
Users couldn't easily exit Claude CLI sessions - Ctrl+C would exit the entire claude-in-a-box interface instead of just the Claude session.

## Solution Implemented

### 1. Changed Default Behavior
- **Before**: Attached directly to Claude CLI (`claude --dangerously-skip-permissions`)
- **After**: Attaches to an interactive bash shell (`/bin/bash`)

### 2. Enhanced User Experience
- Custom welcome message shows available commands
- Clear instructions on how to:
  - Start Claude CLI: Type `claude` 
  - Exit session: Type `exit`
  - Use other shell commands

### 3. Visual Indicators
- Custom prompt: `[claude-box] ~/workspace $`
- Welcome banner with session information
- Updated UI text to reflect shell attachment

## Implementation Details

### Files Modified:
1. **src/app/state.rs** - Changed exec command from claude to /bin/bash
2. **docker/claude-dev/scripts/session-bashrc.sh** - Created custom bashrc with welcome message
3. **docker/claude-dev/Dockerfile** - Added custom bashrc to container
4. **src/components/attached_terminal.rs** - Updated UI text

### User Workflow:
1. Press 'a' to attach to container
2. See welcome message with instructions
3. Use shell normally or run `claude` when needed
4. Type `exit` to return to claude-in-a-box interface

## Benefits:
- More flexible - users can run other commands
- Clear exit mechanism
- Better onboarding experience
- No confusion about how to detach from session