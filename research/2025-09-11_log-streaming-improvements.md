# Research: Log Streaming and Rich Format Display Improvements

**Date**: 2025-09-11 12:45:00
**Repository**: claude-in-a-box
**Branch**: feat/alt-headless
**Commit**: b5b3d5c (latest)
**Research Type**: Codebase

## Research Question
Understand the recent improvements to log streaming and rich format display implemented over the last couple of days, specifically focusing on fixing log stream parsing to display rich format on the screen.

## Executive Summary
Over the past few days, the team implemented a sophisticated JSON streaming parser with brace-balanced scanning, safe memory management, and rich TUI formatting. This transforms raw Docker container logs and Claude's JSON output into a visually appealing, interactive terminal interface with REPL-style tool displays, structured data visualization, and intelligent truncation.

## Key Findings
- **Robust JSON Parser**: Implemented brace-balanced scanner that handles partial JSON objects across Docker log frames
- **Rich Visual Display**: Added REPL-style formatting for tool usage, structured data rendering for todos/file lists, and category-based color coding
- **Memory Safety**: Introduced configurable buffer limits (256KB default) with Unicode-safe truncation to prevent memory issues

## Detailed Findings

### Codebase Analysis

#### JSON Streaming Parser (`src/docker/log_streaming.rs`)
- **Brace-Balanced Scanner** (lines 375-440): Core algorithm that tracks nested braces/brackets while respecting string boundaries
- **Safe Buffering** (lines 258-296): Prevents unbounded memory growth with configurable limits via `CLAUDE_BOX_JSON_BUF_MAX`
- **Partial Object Handling** (lines 213-304): Buffers incomplete JSON objects until properly closed, with automatic fallback to plain text on overflow
- **Unicode-Safe Truncation** (lines 585-597): Ensures character boundaries are preserved when truncating large outputs

#### Agent Parser Architecture (`src/agent_parsers/`)
- **Trait-Based System** (`types.rs:132-144`): Common interface enabling multiple AI agent support
- **Claude JSON Parser** (`claude_json.rs:25-57`): Converts Claude's `--output-format stream-json` into unified `AgentEvent` types
- **Structured Payload Detection** (`claude_json.rs:308-385`): Intelligently extracts TodoLists, file paths, Cargo.toml, and generic JSON
- **Event Types Supported**: System info, assistant messages, tool calls, streaming text, usage statistics

#### Log Processing Pipeline (`src/components/`)
- **Log Parser** (`log_parser.rs:59-103`): ANSI stripping, timestamp extraction, category detection with regex patterns
- **Simple Formatter** (`log_formatter_simple.rs:42-68`): Stateless formatting with timestamps, badges, and styled messages
- **Live Logs Stream** (`live_logs_stream.rs:192-208`): Real-time display with filtering, auto-scroll, and focus indicators

### Rich Format Display Improvements

#### REPL-Style Tool Formatting
```
🔧 Bash: Run tests to check current status
💻 Command: cargo test --quiet 2>&1 | tail -10
```
- Tool calls show descriptive headlines with emoji icons
- Command parameters displayed on separate lines
- Query parameters use 🔎 prefix for search operations

#### Structured Data Visualization
**Todo Lists**:
```
📝 Task Management
  ☑ Completed task
  ⏳ Task in progress  
  ◻︎ Pending task
  Σ 3 • 1 pending • 1 ⏳ • 1 ☑
```

**File Lists**:
```
📂 Found 25 files
  • src/main.rs
  • src/lib.rs
  … +23 more
```

#### Visual Hierarchy
- **Category Icons**: System ⚙️, Auth 🔐, Container 📦, Claude 🤖, Command ⌨️, Git 🔀
- **Log Levels**: Trace 🔍, Debug 🐛, Info ℹ️, Success ✅, Warning ⚠️, Error ❌, Fatal 💀
- **Smart Timestamps**: "now", "5s ago", "2m ago", or HH:MM format

### Documentation Insights
From `plans/structured-tui-rendering.md`:
- **Vision**: Transform TUI from raw JSON blobs to rich visual lists
- **Three-Phase Strategy**: Pipeline completion → Visual rendering → Interactivity
- **Design Principles**: Decoupled architecture, graceful degradation, extensibility

From `research/2025-09-10_19-33-18_improved-log-parsing.md`:
- Research identified need for better structured data handling
- Influenced implementation of type-specific rendering
- Established patterns for future agent support

## Code References
- `src/docker/log_streaming.rs:375-440` - Brace-balanced JSON scanner
- `src/docker/log_streaming.rs:213-304` - JSON streaming with safe buffering
- `src/agent_parsers/claude_json.rs:308-385` - Structured payload extraction
- `src/components/log_formatter_simple.rs:42-68` - Beautiful TUI formatting
- `src/components/log_parser.rs:59-103` - Pattern detection engine
- `src/docker/log_streaming.rs:538-572` - REPL-style tool formatting
- `src/docker/log_streaming.rs:642-712` - Structured data rendering

## Architecture Insights
- **Pattern**: Event-driven streaming pipeline (Docker → AgentEvents → LogEntries → TUI)
- **Convention**: Trait-based parser system for multi-agent support
- **Design Decision**: Stateless formatters to avoid borrow checker issues
- **Memory Strategy**: Configurable buffer limits with graceful fallback

## Recommendations
Based on this research:
1. **Complete Phase 1**: Wire up `AgentEvent::Structured` handling if not already done
2. **Add Comprehensive Tests**: Test coverage for edge cases in JSON parsing and structured rendering
3. **Monitor Memory Usage**: Watch buffer usage patterns in production to tune limits
4. **Document Debug Options**: Create user guide for `CLAUDE_BOX_PARSER_DEBUG` and other env vars

## Open Questions
- Is the `AgentEvent::Structured` pipeline fully connected to the display layer?
- Are there performance metrics for the new streaming parser under load?
- How does the system handle extremely large structured payloads (e.g., 1000+ todos)?

## References
- Internal docs: `plans/structured-tui-rendering.md`, `research/2025-09-10_19-33-18_improved-log-parsing.md`
- Key commits: `461a56d` (JSON streaming parser), `b5b3d5c` (logging updates)
- Environment variables: `CLAUDE_BOX_PARSER_DEBUG`, `CLAUDE_BOX_JSON_BUF_MAX`