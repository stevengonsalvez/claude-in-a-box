# Interactive Terminal Integration Guide

## Overview

The new interactive terminal system provides direct PTY streaming from containers to the TUI through WebSockets, eliminating the multi-layer interaction overhead and providing a true terminal experience.

## Architecture

```
TUI (Rust) ←→ WebSocket ←→ Container PTY Service (TypeScript) ←→ Claude CLI
```

## Key Components

### 1. **WebSocket Protocol** (`src/terminal/protocol.rs`)
- Rust implementation of the WebSocket message protocol
- Matches the TypeScript protocol in the container
- Handles all message types for bidirectional communication

### 2. **WebSocket Client** (`src/terminal/websocket_client.rs`)
- Manages WebSocket connection lifecycle
- Auto-reconnection with exponential backoff
- Heartbeat mechanism for connection health
- Message queuing and routing

### 3. **Terminal Emulator** (`src/terminal/terminal_emulator.rs`)
- VT100 terminal emulation using the `vt100` crate
- ANSI escape code processing
- Scrollback buffer management
- Selection and copy/paste support (framework in place)
- Proper cursor rendering

### 4. **Interactive Terminal Component** (`src/terminal/interactive_terminal.rs`)
- Combines WebSocket client and terminal emulator
- Three view modes: Normal, Expanded, Minimized
- Keyboard input handling and PTY forwarding
- Permission prompt handling
- Connection status management

## Integration Steps

### 1. Replace Live Logs Component

In `src/app/state.rs`, update the imports:
```rust
use crate::terminal::InteractiveTerminalComponent;
```

Replace the logs viewer with the interactive terminal:
```rust
// Instead of LiveLogsStreamComponent
let mut terminal = InteractiveTerminalComponent::new(
    session_id,
    session_name,
    container_id,
    8080, // WebSocket port
).await?;

// Connect to the PTY service
terminal.connect().await?;
```

### 2. Update Event Handling

In `src/app/events.rs`, add handling for terminal-specific keys:
```rust
// Handle 'x' key for expand/collapse
KeyCode::Char('x') => {
    if let Some(terminal) = &mut self.terminal {
        terminal.toggle_view_mode();
    }
}

// Forward other keys to terminal when in expanded mode
_ => {
    if let Some(terminal) = &mut self.terminal {
        if terminal.view_mode == ViewMode::Expanded {
            terminal.handle_input(key).await?;
        }
    }
}
```

### 3. Update Rendering

In the main render loop, use the new terminal component:
```rust
// Render based on view mode
match terminal.view_mode {
    ViewMode::Normal => {
        // Render in right panel
        terminal.render(frame, right_panel).await;
    }
    ViewMode::Expanded => {
        // Render fullscreen
        terminal.render(frame, full_area).await;
    }
    ViewMode::Minimized => {
        // Render status bar only
        terminal.render(frame, status_area).await;
    }
}
```

### 4. Container Configuration

Ensure containers expose the WebSocket port (8080):
```rust
// When creating container
let exposed_ports = HashMap::from([
    ("8080/tcp", HashMap::new()), // PTY WebSocket
    // ... other ports
]);
```

### 5. Session Lifecycle

Connect when session becomes active:
```rust
// On session activation
if let Some(terminal) = &mut self.terminal {
    terminal.connect().await?;
}

// On session deactivation
if let Some(terminal) = &mut self.terminal {
    terminal.disconnect().await?;
}
```

## Usage

### User Interactions

1. **View Terminal**: Select a session to see its terminal output
2. **Expand Terminal**: Press `x` to toggle fullscreen mode
3. **Direct Interaction**: In expanded mode, all keyboard input goes to the container
4. **Scroll**: PageUp/PageDown to scroll through history
5. **Clear**: Ctrl+L to clear the terminal
6. **Permission Prompts**: Number keys select options when prompted

### Terminal Features

- **Full ANSI Support**: Colors, styles, cursor movement
- **Scrollback Buffer**: 10,000 lines of history
- **Real-time Updates**: Direct WebSocket streaming
- **Bidirectional I/O**: Full interactive terminal experience
- **Auto-reconnection**: Handles connection drops gracefully

## Configuration

### WebSocket Settings

In `websocket_client.rs`:
```rust
reconnect_interval: Duration::from_secs(2),
max_reconnect_attempts: 10,
heartbeat_interval: Duration::from_secs(30),
```

### Terminal Settings

In `terminal_emulator.rs`:
```rust
max_scrollback: 10000,
default_cols: 120,
default_rows: 40,
```

## Debugging

### Enable Debug Logging

```bash
RUST_LOG=debug cargo run
```

### Check WebSocket Connection

The terminal shows connection status in the UI. Check logs for details:
```
[INFO] Connecting to WebSocket at ws://container_id:8080/pty
[INFO] WebSocket connected successfully
[INFO] Session initialized: agent-123456789
```

### Verify Container PTY Service

Ensure the container is running the PTY service:
```bash
docker logs <container_id> | grep "PTY Service started"
```

## Performance Considerations

1. **Buffer Management**: Terminal emulator limits scrollback to prevent memory issues
2. **Message Batching**: WebSocket client batches messages when possible
3. **Render Optimization**: Only re-renders when terminal content changes
4. **Connection Pooling**: Reuses WebSocket connections across view mode changes

## Future Enhancements

1. **Terminal Multiplexing**: Multiple PTY sessions per container
2. **Session Recording**: Record and replay terminal sessions
3. **Search in Terminal**: Find text in scrollback buffer
4. **Custom Themes**: User-configurable color schemes
5. **Mouse Support**: Click to position cursor, select text
6. **File Transfer**: Drag-and-drop files to/from container

## Troubleshooting

### Terminal Not Connecting

1. Check container is running: `docker ps`
2. Verify PTY service is started: `docker logs <container>`
3. Check WebSocket port is exposed: `docker port <container>`
4. Review TUI logs: `RUST_LOG=debug`

### Garbled Output

1. Ensure terminal size is synchronized
2. Check TERM environment variable in container
3. Verify ANSI processing is working

### Input Not Working

1. Ensure terminal is in expanded mode for direct input
2. Check WebSocket connection status
3. Verify PTY service is receiving messages

## Testing

Run the integration tests:
```bash
cargo test terminal::tests
```

Manual testing checklist:
- [ ] Terminal connects on session activation
- [ ] Output streams correctly
- [ ] Keyboard input works in expanded mode
- [ ] View modes transition smoothly
- [ ] Scrolling works correctly
- [ ] ANSI colors render properly
- [ ] Permission prompts display and respond
- [ ] Reconnection works after disconnect