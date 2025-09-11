# Structured Data TUI Rendering Implementation Plan

## Overview
Enhance the TUI to render structured JSON data (todos, paths, file contents) as rich visual lists with proper formatting, icons, and summary statistics instead of plain text blobs.

## Current State Analysis
Based on research document `research/2025-09-10_19-33-18_improved-log-parsing.md`:
- **Parser**: ClaudeJsonParser already extracts structured data (todos, paths) into StructuredPayload
- **Types**: AgentEvent::Structured variant exists with TodoList, GlobResults, PrettyJson
- **Missing**: agent_event_to_log_entry() doesn't handle Structured events - they're dropped
- **Rendering**: No visual formatting for structured data in the TUI components

## Desired End State
The TUI should display:
- **Todos**: Check/progress/done icons with counts (◻︎/⏳/☑ + summary)
- **Paths**: Bulleted file lists with truncation ("• /path" + "… +N more")
- **Read summaries**: Key-value pairs, special handling for Cargo.toml
- **Unknown JSON**: Pretty-printed fallback (never raw)
- **Interaction**: Expand/collapse for long lists (Enter key)

### Example Output:
```
📝 Todos
  ⏳ Analyze codebase structure
  ◻︎ Check project organization
  ◻︎ Verify build tooling
  ☑ Add unit tests for parser
Σ 4 • 2 pending • 1 ⏳ • 1 ☑
```

## What We're NOT Doing
- Complex interactive widgets (checkboxes, editing)
- Persistent expand/collapse state across sessions
- Syntax highlighting for code blocks
- Real-time updates of todo status
- Custom themes or color schemes

## Implementation Approach
Three phases: complete the data pipeline, enhance rendering, add interactivity

---

## Phase 1: Complete AgentEvent Pipeline

### Overview
Wire up the Structured event handling so data flows from parser to TUI

### Changes Required:

#### 1. Agent Event Conversion
**File**: `src/docker/log_streaming.rs`
**Location**: Lines 454-573 (agent_event_to_log_entry function)
**Changes**: Add Structured variant handling after line 571

```rust
AgentEvent::Structured(payload) => {
    use crate::agent_parsers::StructuredPayload;
    
    let (level, icon, message) = match payload {
        StructuredPayload::TodoList { title, items, pending, in_progress, done } => {
            let mut msg = String::new();
            
            // Title line
            if let Some(t) = title {
                msg.push_str(&format!("📝 {}\n", t));
            } else {
                msg.push_str("📝 Todos\n");
            }
            
            // Todo items (max 10 shown inline)
            for (i, item) in items.iter().take(10).enumerate() {
                let icon = match item.status.as_str() {
                    "done" | "completed" => "☑",
                    "in_progress" | "active" => "⏳",
                    _ => "◻︎",
                };
                msg.push_str(&format!("  {} {}\n", icon, item.text));
            }
            
            if items.len() > 10 {
                msg.push_str(&format!("  … +{} more\n", items.len() - 10));
            }
            
            // Summary line
            msg.push_str(&format!("Σ {} • {} pending • {} ⏳ • {} ☑", 
                items.len(), pending, in_progress, done));
            
            (LogEntryLevel::Info, "📝", msg)
        }
        
        StructuredPayload::GlobResults { paths, total } => {
            let mut msg = format!("📂 Found {} files\n", total);
            
            // Show first 15 paths
            for path in paths.iter().take(15) {
                msg.push_str(&format!("  • {}\n", path));
            }
            
            if paths.len() > 15 {
                msg.push_str(&format!("  … +{} more", paths.len() - 15));
            }
            
            (LogEntryLevel::Info, "📂", msg)
        }
        
        StructuredPayload::PrettyJson(json_str) => {
            (LogEntryLevel::Info, "📋", format!("📋 Data:\n{}", json_str))
        }
    };
    
    LogEntry::new(level, container_name.to_string(), message)
        .with_session(session_id)
        .with_metadata("event_type", "structured")
        .with_metadata("icon", icon)
}
```

#### 2. Enhance Structured Parsing
**File**: `src/agent_parsers/claude_json.rs`
**Location**: Lines 285-323 (parse_structured_from_value)
**Changes**: Add ReadSummary detection for Cargo.toml patterns

```rust
// After existing TodoList and GlobResults checks, before None return:

// Detect Cargo.toml structure
if let Some(package) = v.get("package").and_then(|p| p.as_object()) {
    let name = package.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
    let version = package.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0");
    let edition = package.get("edition").and_then(|e| e.as_str()).unwrap_or("2021");
    
    let json_str = format!("📦 Cargo: {} v{} (edition {})", name, version, edition);
    return Some(AgentEvent::Structured(StructuredPayload::PrettyJson(json_str)));
}

// Generic key-value detection for Read tool results
if v.is_object() && !v.as_object().unwrap().is_empty() {
    // Pretty print with 2-space indentation
    if let Ok(pretty) = serde_json::to_string_pretty(&v) {
        return Some(AgentEvent::Structured(StructuredPayload::PrettyJson(pretty)));
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] Cargo build succeeds: `cargo build`
- [x] Existing tests pass: `cargo test`
- [x] New structured event flows through to LogEntry

#### Manual Verification:
- [ ] TodoWrite tool results show as formatted lists
- [ ] Glob tool results display file paths
- [ ] JSON blobs are pretty-printed

---

## Phase 2: Enhanced Visual Rendering

### Overview
Create dedicated rendering logic for structured payloads with proper formatting

### Changes Required:

#### 1. Add Structured Payload to LogEntry
**File**: `src/components/live_logs_stream.rs`
**Location**: Lines 374-382 (LogEntry struct)
**Changes**: Add structured_payload field

```rust
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: LogEntryLevel,
    pub source: String,
    pub message: String,
    pub session_id: Option<uuid::Uuid>,
    pub parsed_data: Option<super::log_parser::ParsedLog>,
    pub metadata: std::collections::HashMap<String, String>,
    pub structured_payload: Option<crate::agent_parsers::StructuredPayload>, // NEW
}
```

#### 2. Update LogEntry Construction
**File**: `src/docker/log_streaming.rs`
**Location**: Modify agent_event_to_log_entry to store payload

```rust
// In the Structured match arm, instead of just formatting to string:
AgentEvent::Structured(payload) => {
    let (level, summary) = match &payload {
        StructuredPayload::TodoList { items, .. } => {
            (LogEntryLevel::Info, format!("📝 {} todos", items.len()))
        }
        StructuredPayload::GlobResults { total, .. } => {
            (LogEntryLevel::Info, format!("📂 {} files", total))
        }
        StructuredPayload::PrettyJson(_) => {
            (LogEntryLevel::Info, "📋 JSON data".to_string())
        }
    };
    
    let mut entry = LogEntry::new(level, container_name.to_string(), summary)
        .with_session(session_id)
        .with_metadata("event_type", "structured");
    
    entry.structured_payload = Some(payload);
    entry
}
```

#### 3. Create Structured Formatter
**File**: `src/components/log_formatter_structured.rs` (NEW)
**Changes**: Create new formatter for structured payloads

```rust
// ABOUTME: Formatter for structured payloads with rich visual rendering
// Handles todos, file lists, and JSON data with appropriate icons and layout

use crate::agent_parsers::StructuredPayload;
use ratatui::text::{Line, Span};
use ratatui::style::{Color, Style, Modifier};

pub struct StructuredFormatter {
    pub max_items: usize,
    pub expanded: bool,
}

impl StructuredFormatter {
    pub fn new() -> Self {
        Self {
            max_items: 10,
            expanded: false,
        }
    }
    
    pub fn format_payload(&self, payload: &StructuredPayload) -> Vec<Line> {
        match payload {
            StructuredPayload::TodoList { title, items, pending, in_progress, done } => {
                self.format_todos(title.as_deref(), items, *pending, *in_progress, *done)
            }
            StructuredPayload::GlobResults { paths, total } => {
                self.format_paths(paths, *total)
            }
            StructuredPayload::PrettyJson(json) => {
                self.format_json(json)
            }
        }
    }
    
    fn format_todos(
        &self,
        title: Option<&str>,
        items: &[crate::agent_parsers::TodoItem],
        pending: u32,
        in_progress: u32,
        done: u32,
    ) -> Vec<Line> {
        let mut lines = Vec::new();
        
        // Title
        lines.push(Line::from(vec![
            Span::styled("📝 ", Style::default()),
            Span::styled(
                title.unwrap_or("Todos"),
                Style::default().add_modifier(Modifier::BOLD)
            ),
        ]));
        
        // Items
        let show_count = if self.expanded { items.len() } else { self.max_items.min(items.len()) };
        
        for item in items.iter().take(show_count) {
            let (icon, color) = match item.status.as_str() {
                "done" | "completed" => ("☑", Color::Green),
                "in_progress" | "active" => ("⏳", Color::Yellow),
                _ => ("◻︎", Color::Gray),
            };
            
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(icon, Style::default().fg(color)),
                Span::raw(" "),
                Span::raw(&item.text),
            ]));
        }
        
        if items.len() > show_count {
            lines.push(Line::from(Span::styled(
                format!("  … +{} more", items.len() - show_count),
                Style::default().fg(Color::DarkGray)
            )));
        }
        
        // Summary
        lines.push(Line::from(vec![
            Span::styled("Σ ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{} • ", items.len())),
            Span::raw(format!("{} pending • ", pending)),
            Span::styled(format!("{} ⏳ • ", in_progress), Style::default().fg(Color::Yellow)),
            Span::styled(format!("{} ☑", done), Style::default().fg(Color::Green)),
        ]));
        
        lines
    }
    
    fn format_paths(&self, paths: &[String], total: usize) -> Vec<Line> {
        let mut lines = Vec::new();
        
        lines.push(Line::from(vec![
            Span::styled("📂 ", Style::default()),
            Span::styled(
                format!("Found {} files", total),
                Style::default().add_modifier(Modifier::BOLD)
            ),
        ]));
        
        let show_count = if self.expanded { paths.len() } else { self.max_items.min(paths.len()) };
        
        for path in paths.iter().take(show_count) {
            lines.push(Line::from(vec![
                Span::raw("  • "),
                Span::raw(path),
            ]));
        }
        
        if paths.len() > show_count {
            lines.push(Line::from(Span::styled(
                format!("  … +{} more", paths.len() - show_count),
                Style::default().fg(Color::DarkGray)
            )));
        }
        
        lines
    }
    
    fn format_json(&self, json: &str) -> Vec<Line> {
        let mut lines = Vec::new();
        
        lines.push(Line::from(vec![
            Span::styled("📋 ", Style::default()),
            Span::styled("Data", Style::default().add_modifier(Modifier::BOLD)),
        ]));
        
        for line in json.lines().take(if self.expanded { 100 } else { 10 }) {
            lines.push(Line::from(Span::raw(format!("  {}", line))));
        }
        
        if json.lines().count() > 10 && !self.expanded {
            lines.push(Line::from(Span::styled(
                "  … [Enter to expand]",
                Style::default().fg(Color::DarkGray)
            )));
        }
        
        lines
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Code compiles: `cargo build`
- [ ] Type checking passes: `cargo check`

#### Manual Verification:
- [ ] Structured data displays with proper icons
- [ ] Todo counts are accurate
- [ ] Path lists truncate appropriately
- [ ] JSON is pretty-printed

---

## Phase 3: Add Interactivity & Tests

### Overview
Add expand/collapse functionality and comprehensive test coverage

### Changes Required:

#### 1. Add Expand/Collapse State
**File**: `src/components/live_logs_stream.rs`
**Location**: Add to LiveLogsStreamComponent struct around line 40

```rust
pub struct LiveLogsStreamComponent {
    // ... existing fields ...
    expanded_entries: std::collections::HashSet<usize>, // NEW - indices of expanded entries
}

// In handle_input method, add Enter key handling:
KeyCode::Enter => {
    if let Some(selected) = self.selected_index {
        if self.expanded_entries.contains(&selected) {
            self.expanded_entries.remove(&selected);
        } else {
            self.expanded_entries.insert(selected);
        }
    }
}
```

#### 2. Unit Tests for Parser
**File**: `src/agent_parsers/claude_json.rs`
**Location**: Add tests module at end of file

```rust
#[cfg(test)]
mod structured_parsing_tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_parse_todo_list() {
        let json = json!({
            "todos": [
                {"text": "Task 1", "status": "pending"},
                {"text": "Task 2", "status": "in_progress"},
                {"text": "Task 3", "status": "done"}
            ]
        });
        
        let event = ClaudeJsonParser::parse_structured_from_value(&json);
        assert!(matches!(
            event,
            Some(AgentEvent::Structured(StructuredPayload::TodoList { .. }))
        ));
        
        if let Some(AgentEvent::Structured(StructuredPayload::TodoList { items, pending, in_progress, done, .. })) = event {
            assert_eq!(items.len(), 3);
            assert_eq!(pending, 1);
            assert_eq!(in_progress, 1);
            assert_eq!(done, 1);
        }
    }
    
    #[test]
    fn test_parse_glob_results() {
        let json = json!({
            "paths": ["src/main.rs", "src/lib.rs", "Cargo.toml"],
            "total": 3
        });
        
        let event = ClaudeJsonParser::parse_structured_from_value(&json);
        assert!(matches!(
            event,
            Some(AgentEvent::Structured(StructuredPayload::GlobResults { .. }))
        ));
    }
    
    #[test]
    fn test_parse_cargo_toml() {
        let json = json!({
            "package": {
                "name": "my-app",
                "version": "1.0.0",
                "edition": "2021"
            }
        });
        
        let event = ClaudeJsonParser::parse_structured_from_value(&json);
        assert!(matches!(
            event,
            Some(AgentEvent::Structured(StructuredPayload::PrettyJson(_)))
        ));
    }
}
```

#### 3. Integration Test
**File**: `tests/structured_rendering_test.rs` (NEW)

```rust
use claude_in_a_box::agent_parsers::{AgentOutputParser, ClaudeJsonParser};

#[test]
fn test_streaming_json_with_structured_payloads() {
    let mut parser = ClaudeJsonParser::new();
    
    // Simulate streaming JSON
    let lines = vec![
        r#"{"type":"user","message":{"content":[{"type":"tool_result","#,
        r#""tool_use_id":"123","content":"{\"todos\":[{\"text\":\"Test\",\"status\":\"done\"}]}"}]}}"#,
    ];
    
    let mut all_events = Vec::new();
    for line in lines {
        match parser.parse_line(line) {
            Ok(events) => all_events.extend(events),
            Err(e) => panic!("Parse error: {}", e),
        }
    }
    
    // Should have parsed the todo list
    assert!(all_events.iter().any(|e| matches!(e, 
        crate::agent_parsers::AgentEvent::Structured(_)
    )));
}
```

### Success Criteria:

#### Automated Verification:
- [x] All tests pass: `cargo test` (7 new structured parsing tests added and passing)
- [ ] Clippy passes: `cargo clippy`
- [ ] Format check: `cargo fmt -- --check`

#### Manual Verification:
- [ ] Enter key toggles expand/collapse
- [ ] Expanded state shows more items
- [ ] State persists while viewing same session
- [ ] Performance acceptable with large lists

---

## Testing Strategy

### Unit Tests:
- Parse todos with various status combinations
- Parse file paths with different counts
- Parse Cargo.toml detection
- Parse generic JSON fallback
- Test truncation at boundaries

### Integration Tests:
- Streaming JSON splits handled correctly
- Multiple structured events in sequence
- Mixed structured and regular events
- Buffer overflow scenarios

### Manual Testing Steps:
1. Start Boss mode session
2. Trigger TodoWrite tool - verify formatted display
3. Trigger Glob tool - verify path list
4. Trigger Read on Cargo.toml - verify special formatting
5. Test expand/collapse with Enter key
6. Verify performance with 100+ todos

## Performance Considerations
- Limit inline display to 10-15 items (configurable)
- Use iterators to avoid cloning large lists
- Cache formatted output if unchanged
- Consider virtual scrolling for very long lists

## Migration Notes
- Existing logs remain compatible
- No database changes required
- Graceful fallback for unknown structures

## References
- Original requirements: User-provided specification
- Related research: `research/2025-09-10_19-33-18_improved-log-parsing.md`
- Parser implementation: `src/agent_parsers/claude_json.rs:285-323`
- Event conversion: `src/docker/log_streaming.rs:454-573`