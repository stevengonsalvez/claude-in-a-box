# Widget System Refactoring Handover

**Date**: 2025-09-14
**Project**: claude-in-a-box
**Branch**: feat/logging-1
**Focus**: Improving JSON message routing like Opcode

## Summary

Began refactoring the widget system to achieve cleaner message routing similar to Opcode's StreamMessage.tsx. Created several new widgets and a message router, but encountered architectural differences that need to be addressed.

## What Was Completed

### New Widgets Created
1. **message_router.rs** - Central routing system (needs adaptation)
2. **multiedit_widget.rs** - Handles multiple file edits
3. **mcp_widget.rs** - Model Context Protocol tools
4. **ls_result_widget.rs** - Directory listing results
5. **system_reminder_widget.rs** - System notifications

### Key Findings

The main difference between Opcode and claude-in-a-box:
- **Opcode**: Uses a flat JSON structure where `message.type` and `message.content` determine routing
- **claude-in-a-box**: Uses an `AgentEvent` enum with specific variants for each event type

## Current Issues

### Compilation Errors
The new widgets assume `AgentEvent` has fields like `event_type` and `payload`, but it's actually an enum with variants like:
```rust
pub enum AgentEvent {
    SessionInfo { ... },
    Thinking { content: String },
    Message { content: String, id: Option<String> },
    ToolCall { id: String, name: String, input: Value, ... },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
    // etc.
}
```

## Recommended Next Steps

### Option 1: Adapt Widgets to Enum Pattern
Update all new widgets to use pattern matching:
```rust
match event {
    AgentEvent::ToolCall { name, input, .. } if name.starts_with("mcp__") => {
        // Handle MCP tool
    }
    AgentEvent::ToolResult { content, .. } if Self::is_ls_result(&content) => {
        // Handle LS result
    }
    _ => // fallback
}
```

### Option 2: Create Intermediate Layer
Add a translation layer that converts `AgentEvent` to a structure similar to Opcode's:
```rust
struct MessageData {
    event_type: String,
    content: Option<Value>,
    metadata: HashMap<String, Value>,
}

impl From<AgentEvent> for MessageData {
    // Convert enum to flat structure
}
```

### Option 3: Refactor AgentEvent (Breaking Change)
Change `AgentEvent` from an enum to a struct with a type field:
```rust
pub struct AgentEvent {
    pub event_type: String,
    pub payload: Option<StructuredPayload>,
    pub metadata: HashMap<String, Value>,
}
```

## Files Modified
```
A src/widgets/message_router.rs
A src/widgets/multiedit_widget.rs
A src/widgets/mcp_widget.rs
A src/widgets/ls_result_widget.rs
A src/widgets/system_reminder_widget.rs
M src/widgets/mod.rs
M src/docker/log_streaming.rs
```

## How to Continue

### Fix Compilation
```bash
# Option 1: Quick fix - revert to WidgetRegistry
git checkout src/docker/log_streaming.rs

# Option 2: Fix the widgets
# Update each widget's can_handle() and render() to use pattern matching
```

### Complete Integration
1. Fix pattern matching in all new widgets
2. Update MessageRouter to work with enum variants
3. Add tool result caching mechanism
4. Test with real Claude output

## Testing Commands
```bash
# Check compilation
cargo check

# Run tests
cargo test widget

# Test with real Claude
cargo run
# Then in another terminal:
docker logs -f <container_name>
```

## Architecture Recommendation

The cleanest approach would be Option 2 - create an intermediate translation layer. This preserves the existing enum-based `AgentEvent` while allowing widgets to work with a flatter structure similar to Opcode.

Example implementation:
```rust
// In message_router.rs
impl MessageRouter {
    fn route_event(&self, event: AgentEvent, ...) -> WidgetOutput {
        let message_data = MessageData::from(event);

        match message_data.event_type.as_str() {
            "tool_call" => self.route_tool_call(message_data, ...),
            "tool_result" => self.route_tool_result(message_data, ...),
            // etc.
        }
    }
}
```

This approach:
- Maintains backward compatibility
- Provides cleaner widget interfaces
- Allows for Opcode-style routing logic
- Easier to test and maintain

## Context for Next Session

The goal is to achieve the clean message rendering seen in Opcode where each JSON event type gets its own specialized widget. The main challenge is adapting Opcode's React/TypeScript patterns to Rust's type system while maintaining the existing `AgentEvent` enum structure.

Consider whether a full refactor is worth it, or if improving the existing widget system incrementally would be more practical.