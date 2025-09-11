# Research: Improved Log Parsing from Container to TUI

**Date**: 2025-09-10 19:33:18
**Repository**: claude-in-a-box  
**Branch**: feat/alt-headless
**Commit**: 461a56d (feat(logs): add robust JSON streaming parser and safe truncation)
**Research Type**: Comprehensive (Codebase + Architecture)

## Research Question
Analyze the latest changes involving improved parsing of logs from the container to the TUI, with a decoupled mechanism to parse different agent logs and specifics of Claude in a Claude-specific file.

## Executive Summary
The codebase has evolved from basic Docker log streaming to a sophisticated, decoupled agent parsing system. Recent changes introduce a robust JSON streaming parser for Claude's `--output-format stream-json`, with safety mechanisms preventing memory issues, and support for rich structured content like todo lists and file paths.

## Key Findings
- **Decoupled Architecture**: Agent-specific parsers implement a common `AgentOutputParser` trait, enabling support for multiple AI agents
- **Claude JSON Streaming**: Robust brace-balanced parser handles partial JSON frames across Docker log boundaries
- **Safety Features**: Configurable buffer limits (256KB default) prevent unbounded memory growth
- **Rich Content Support**: Structured payloads for todos and file paths (not yet rendered in TUI)

## Detailed Findings

### Codebase Analysis

#### Agent Parser Architecture
**Location**: `src/agent_parsers/`

The system uses a trait-based architecture for extensibility:

1. **AgentOutputParser Trait** (`types.rs:140-152`)
   - Common interface for all agent parsers
   - Methods: `parse_line()`, `flush()`, `agent_type()`, `reset()`
   - Enables hot-swapping parsers based on detected format

2. **Current Implementations**:
   - **ClaudeJsonParser** (`claude_json.rs:1-391`): Handles Claude's stream-json format
   - **PlainTextParser** (`plain_text.rs:1-45`): Fallback for non-JSON output

3. **ParserFactory** (`types.rs:154-178`)
   - Auto-detects format from first line
   - Supports explicit parser selection by agent type
   - JSON detection handles Docker timestamp prefixes

#### Claude-Specific Parsing Implementation
**File**: `src/agent_parsers/claude_json.rs`

**Key Features**:
- **Event Type Handling** (lines 33-55):
  - `system`: Session initialization with model, tools, MCP servers
  - `assistant`: Messages, tool calls, usage statistics  
  - `user`: Tool results with error handling

- **Streaming Support** (lines 124-138):
  - Buffers incomplete messages
  - Sends delta updates for real-time display
  - Maintains message IDs for correlation

- **Structured Payload Parsing** (lines 285-323):
  - **TodoList**: Extracts items with status counts
  - **GlobResults**: File path lists with totals
  - Falls back to pretty-printed JSON

- **State Management** (`types.rs:117-129`):
  - Tracks active tool calls
  - Buffers incomplete JSON lines
  - Manages streaming message state

#### Container to TUI Log Streaming Flow
**File**: `src/docker/log_streaming.rs`

**Data Flow**:
1. **Docker Log Collection** (lines 147-164):
   - Uses Bollard Docker client
   - Streams stdout/stderr with timestamps
   - Starts with last 100 lines for context

2. **JSON Stream Processing** (lines 204-298):
   - Detects JSON by finding `{` character
   - Buffers partial objects across frames
   - Uses `stream_json_objects()` for brace-balanced parsing
   - Enforces 256KB buffer limit with configurable override

3. **Agent Event Conversion** (lines 454-573):
   - Converts AgentEvents to LogEntry for display
   - Rich formatting with emojis and structure:
     - ToolCall: `🔧 {name}: {description}` + `💻 Command: {cmd}`
     - Message: `💬 {content}`
     - ToolResult: `✅/❌ Result: {content}` (500 char limit)
     - Usage: `📈 Usage: {tokens}`

4. **Safety Mechanisms**:
   - **Buffer Limits**: Prevents unbounded memory growth
   - **Unicode-Safe Truncation**: Respects character boundaries
   - **Debug Mode**: `CLAUDE_BOX_PARSER_DEBUG` env var for tracing

#### Session Modes
**File**: `src/models/session.rs:8-11`

```rust
pub enum SessionMode {
    Interactive,  // Traditional shell access
    Boss,        // Non-interactive with JSON streaming
}
```

**Boss Mode Specifics**:
- Uses `claude --print --output-format stream-json`
- Automatically enhances prompts with project guidelines
- Enables rich TUI display with structured output
- JSON events parsed for visual hierarchy

### Architecture Insights

**Pattern**: **Strategy Pattern** for parser selection
- ParserFactory creates appropriate parser based on format
- All parsers implement common AgentOutputParser trait
- Runtime selection without code changes

**Convention**: **Event-Driven Architecture**
- Docker logs → Events → Log Entries → TUI Display
- Loosely coupled components communicate via events
- Enables parallel processing and buffering

**Design Decision**: **Streaming-First Approach**
- Handles partial data gracefully
- Buffers incomplete JSON objects
- Flushes on completion or buffer limits
- Prioritizes responsiveness over completeness

### Test Coverage Analysis

**Current Tests**:
- Parser factory detection (`types.rs:185-190`)
- Basic log entry parsing (`log_streaming.rs:725-771`)
- Tool call rendering (`log_streaming.rs:773-794`)

**Missing Coverage**:
- ClaudeJsonParser parsing logic
- JSON streaming brace-balance algorithm
- All AgentEvent type conversions
- Structured payload handling
- Error scenarios and edge cases

## Recommendations

Based on this research:

1. **Complete Structured Rendering**: Add `AgentEvent::Structured` handling in `agent_event_to_log_entry()` to display todo lists and file paths in the TUI

2. **Add Comprehensive Tests**: Create unit tests for:
   - ClaudeJsonParser event parsing
   - stream_json_objects() algorithm
   - Structured payload extraction
   - Buffer limit enforcement

3. **Extend Parser Support**: Architecture is ready for additional agent parsers:
   - GPT (OpenAI format)
   - Gemini (Google format)
   - Local LLMs (Ollama, etc.)

4. **Optimize Memory Usage**: Consider streaming JSON parser library (like `serde_json::StreamDeserializer`) for better memory efficiency with large payloads

5. **Enhanced Error Recovery**: Add recovery mechanisms for:
   - Corrupted JSON mid-stream
   - Parser state reset on errors
   - Fallback to plain text on repeated failures

## Open Questions

- Why is `AgentEvent::Structured` not handled in the conversion function?
- Should buffer limits be per-session or global?
- How to handle multi-agent sessions with different output formats?
- Should structured payloads have custom UI components?

## Code References

### Core Parser Files
- `src/agent_parsers/claude_json.rs:1-391` - Claude JSON stream parser
- `src/agent_parsers/types.rs:1-191` - Common types and traits
- `src/agent_parsers/plain_text.rs:1-45` - Fallback text parser
- `src/agent_parsers/mod.rs:1-10` - Module exports

### Log Streaming Pipeline
- `src/docker/log_streaming.rs:1-796` - Docker log collection and processing
- `src/docker/log_streaming.rs:365-411` - JSON streaming parser
- `src/docker/log_streaming.rs:454-573` - Event to LogEntry conversion

### TUI Display Components
- `src/components/live_logs_stream.rs:1-578` - Live log display widget
- `src/components/log_parser.rs:1-403` - Log categorization and cleaning
- `src/components/log_formatter_simple.rs:1-120` - Visual formatting

### Session Management
- `src/models/session.rs:8-11` - SessionMode enum
- `src/docker/session_lifecycle.rs:741-747` - Mode environment setup
- `docker/claude-dev/scripts/startup.sh:165-166` - Boss mode execution

## References

- Internal docs: Project guidelines in docker/claude-dev/.claude-box
- Related commits: 461a56d (JSON parser), 6723bea (log formatter)
- Architecture patterns: Strategy, Event-Driven, Streaming-First