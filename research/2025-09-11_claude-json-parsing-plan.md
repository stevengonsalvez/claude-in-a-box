# Claude JSON Parsing Architecture Research & Improvement Plan

**Date**: 2025-09-11
**Repository**: claude-in-a-box
**Branch**: feat/alt-headless
**Research Type**: Comprehensive Codebase Analysis

## Research Question
How to implement good parsing logic for Claude's JSON returns, inspired by StreamMessage.tsx widget pattern

## Executive Summary
The current implementation has a solid foundation with robust JSON streaming and structured payload detection. However, it lacks the extensible widget pattern that would enable rich, tool-specific formatting. This plan proposes a widget-based architecture that maintains backward compatibility while enabling sophisticated rendering for each tool type.

## Key Findings

1. **Current Architecture is Solid**: Robust JSON streaming with brace-balanced parsing, configurable buffers, and good error recovery
2. **Limited Tool Coverage**: Only TodoWrite and Glob have special formatting; other tools use generic display
3. **No Widget Pattern**: Current system uses monolithic conversion function rather than composable widgets
4. **Missing Streaming Events**: Some Claude streaming events aren't fully handled

## Current Architecture Analysis

### Data Flow Pipeline
```
Docker Logs → JSON Extraction → stream_json_objects() → AgentParser → AgentEvent → LogEntry → TUI Display
```

### Core Components

#### 1. JSON Streaming (`src/docker/log_streaming.rs:375-437`)
- **Brace-balanced parsing** handles incomplete JSON
- **String-aware** processing with escape handling
- **Buffer protection** with 256KB default limit
- **Unicode-safe** operations

#### 2. Agent Events (`src/agent_parsers/types.rs:32-99`)
```rust
pub enum AgentEvent {
    SessionInfo,
    Thinking,
    Message,
    StreamingText,
    ToolCall,
    ToolResult,
    Error,
    Usage,
    Custom,
    Structured(StructuredPayload),
}
```

#### 3. Structured Payloads (`src/agent_parsers/types.rs:8-23`)
```rust
pub enum StructuredPayload {
    TodoList { items, pending, in_progress, done },
    GlobResults { paths, total },
    PrettyJson(String),
}
```

### Current Tool Formatting

Only two tools have rich formatting:

1. **TodoWrite**: Multi-line with status icons (☑, ⏳, ◻︎)
2. **Glob**: File list with truncation ("… +N more")

Other tools show generic format:
```
🔧 ToolName: Description
💻 Command: [command text]
```

## Identified Gaps

### 1. Limited Tool Coverage
Missing rich formatting for: Edit (diffs), Write (previews), Read (excerpts), Grep (highlights), Task (sub-agents)

### 2. No Composable Widgets
Single `agent_event_to_log_entry()` function handles all conversions - not extensible

### 3. Missing Event Types
- `ContentBlockStart`/`ContentBlockStop`
- `MessageDelta`
- Partial streaming results

### 4. Rigid Display Format
Everything converts to string-based `LogEntry` - no interactive components

## Proposed Widget Architecture

### Phase 1: Widget Pattern Foundation

```rust
// src/widgets/mod.rs
trait MessageWidget {
    fn can_handle(&self, event: &AgentEvent) -> bool;
    fn render(&self, event: AgentEvent) -> WidgetOutput;
}

enum WidgetOutput {
    Simple(LogEntry),
    MultiLine(Vec<LogEntry>),
    Interactive(InteractiveComponent),
}

struct WidgetRegistry {
    widgets: Vec<Box<dyn MessageWidget>>,
}
```

### Phase 2: Specialized Widgets

Create individual widgets for each tool type:

- **BashWidget**: Command with syntax highlighting, exit codes
- **EditWidget**: Diff view with additions/deletions
- **WriteWidget**: File preview with syntax highlighting
- **ReadWidget**: Content excerpt with line numbers
- **GrepWidget**: Search results with match highlights
- **TaskWidget**: Sub-agent spawning visualization
- **ThinkingWidget**: Collapsible thinking sections

### Phase 3: Extended Structured Payloads

```rust
enum StructuredPayload {
    // Existing...
    TodoList,
    GlobResults,
    PrettyJson,

    // New additions:
    DiffView { file_path, additions, deletions, context },
    FilePreview { path, content, line_numbers, syntax_lang },
    GrepResults { matches, total_matches, files_searched },
    TestResults { passed, failed, skipped, failures },
    SubAgentSpawn { agent_type, task, status },
}
```

### Phase 4: Smart Content Detection

Enhanced pattern matching in `parse_structured_from_value()`:

```rust
fn parse_structured_from_value(v: &Value) -> Option<AgentEvent> {
    // Detect diffs
    if v.get("old_string").is_some() && v.get("new_string").is_some() {
        return Some(StructuredPayload::DiffView { ... });
    }

    // Detect test results
    if v.get("passed").is_some() && v.get("failed").is_some() {
        return Some(StructuredPayload::TestResults { ... });
    }

    // Detect grep results
    if let Some(matches) = v.get("matches").and_then(|m| m.as_array()) {
        return Some(StructuredPayload::GrepResults { ... });
    }
}
```

### Phase 5: Streaming Event Support

```rust
enum StreamingState {
    Idle,
    InMessage { id: String, accumulated: String },
    InToolCall { id: String, name: String, partial_input: Value },
    InThinking { accumulated: String },
}
```

### Phase 6: Enhanced Error Recovery

```rust
enum ParseError {
    IncompleteJson { buffer: String, expected: String },
    MalformedTool { tool_name: String, reason: String },
    UnknownEventType { type_str: String, raw: Value },
}

impl ClaudeJsonParser {
    fn recover_from_error(&mut self, error: ParseError) -> Option<AgentEvent> {
        // Graceful degradation strategies
    }
}
```

## Implementation Plan

### Week 1: Foundation
- [x] Create widget trait and registry system
- [x] Implement core widgets (Bash, Edit, Todo)
- [x] Add widget tests

### Week 2: Expansion
- [ ] Extend StructuredPayload types
- [ ] Enhance content detection
- [ ] Add more widgets (Write, Read, Grep)

### Week 3: Streaming
- [ ] Add streaming event support
- [ ] Implement error recovery
- [ ] Performance optimization

### Week 4: Polish
- [ ] Interactive components (stretch)
- [ ] Migration from old system
- [ ] Documentation

## Testing Strategy

```rust
#[test]
fn test_widget_rendering() {
    let event = create_bash_tool_call("ls -la");
    let output = BashWidget.render(event);
    assert!(matches!(output, WidgetOutput::MultiLine(_)));
}

#[test]
fn test_widget_fallback() {
    let unknown = create_unknown_event();
    let output = registry.render(unknown);
    assert!(matches!(output, WidgetOutput::Simple(_)));
}
```

## Migration Path

1. Keep existing `agent_event_to_log_entry` as fallback
2. Gradually migrate tool types to widgets
3. Feature flag for new system
4. A/B test with users
5. Deprecate old system once stable

## Code References

### Core Files
- `src/agent_parsers/types.rs:32-99` - AgentEvent enum
- `src/agent_parsers/claude_json.rs:307-385` - Structured detection
- `src/docker/log_streaming.rs:375-437` - JSON streaming
- `src/docker/log_streaming.rs:481-743` - Event conversion
- `src/components/live_logs_stream.rs:192-250` - Display rendering

### Test Files
- `src/agent_parsers/claude_json.rs:457-702` - Parser tests
- `src/docker/log_streaming.rs:892-992` - Integration tests

## Next Steps

1. Review this plan with the team
2. Create GitHub issues for each phase
3. Set up feature branch for widget system
4. Begin with BashWidget as proof of concept
5. Gather feedback on widget output format

## Benefits of This Approach

1. **Extensibility**: Easy to add new tool widgets
2. **Maintainability**: Each widget is self-contained
3. **Testability**: Individual widget testing
4. **Performance**: Lazy rendering, caching possible
5. **User Experience**: Rich, tool-specific formatting

---
*Generated with Claude on 2025-09-11*
