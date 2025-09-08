// ABOUTME: Agent output parser module - provides modular parsing for different AI agent outputs
// Supports Claude JSON streaming, plain text, and extensible for future agents

pub mod types;
pub mod claude_json;
pub mod plain_text;

pub use types::{AgentEvent, AgentOutputParser, ParserFactory, ParserState, McpServerInfo};
pub use claude_json::ClaudeJsonParser;
pub use plain_text::PlainTextParser;