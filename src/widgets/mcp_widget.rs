// ABOUTME: Widget for rendering Model Context Protocol (MCP) tool calls
// Handles MCP server interactions and displays their results

use crate::agent_parsers::{AgentEvent, types::StructuredPayload};
use crate::components::live_logs_stream::{LogEntry, LogEntryLevel};
use serde_json::Value;
use uuid::Uuid;

use super::{MessageWidget, WidgetOutput, ToolResult, helpers};

pub struct McpWidget;

impl McpWidget {
    pub fn new() -> Self {
        Self
    }

    /// Extract the MCP server and method from the tool name (e.g., "mcp__sequential-thinking__sequentialthinking")
    fn parse_mcp_tool_name(name: &str) -> (String, String, String) {
        let parts: Vec<&str> = name.split("__").collect();
        if parts.len() >= 3 {
            (parts[1].to_string(), parts[2].to_string(), name.to_string())
        } else {
            ("unknown".to_string(), "unknown".to_string(), name.to_string())
        }
    }

    /// Format MCP input parameters for display
    fn format_mcp_input(input: &Value) -> Vec<String> {
        let mut lines = Vec::new();

        if let Some(obj) = input.as_object() {
            for (key, value) in obj {
                let value_str = match value {
                    Value::String(s) => {
                        if s.len() > 100 {
                            format!("\"{}...\"", &s[..100])
                        } else {
                            format!("\"{}\"", s)
                        }
                    }
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "null".to_string(),
                    Value::Array(arr) => format!("[{} items]", arr.len()),
                    Value::Object(obj) => format!("{{{}
 fields}}", obj.len()),
                };
                lines.push(format!("      {}: {}", key, value_str));
            }
        }

        lines
    }
}

impl MessageWidget for McpWidget {
    fn can_handle(&self, event: &AgentEvent) -> bool {
        // Check if this is an MCP tool use (starts with "mcp__")
        if event.event_type == "tool_use" {
            if let Some(ref payload) = event.payload {
                if let Some(name) = payload.data.get("name").and_then(|v| v.as_str()) {
                    return name.starts_with("mcp__");
                }
            }
        }
        false
    }

    fn render(&self, event: AgentEvent, container_name: &str, session_id: Uuid) -> WidgetOutput {
        let mut entries = Vec::new();

        // Extract tool name and input from the event
        if let Some(ref payload) = event.payload {
            let tool_name = payload.data.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            let (server, method, _full_name) = Self::parse_mcp_tool_name(tool_name);

            // Header with MCP icon
            entries.push(helpers::create_log_entry(
                LogEntryLevel::Info,
                container_name,
                format!("🔌 MCP: {}::{}", server, method),
                session_id,
                "mcp",
            ));

            // Show input parameters if available
            if let Some(input) = payload.data.get("input") {
                entries.push(helpers::create_log_entry(
                    LogEntryLevel::Debug,
                    container_name,
                    "   Parameters:".to_string(),
                    session_id,
                    "mcp",
                ));

                let formatted_lines = Self::format_mcp_input(input);
                for line in formatted_lines {
                    entries.push(helpers::create_log_entry(
                        LogEntryLevel::Debug,
                        container_name,
                        line,
                        session_id,
                        "mcp",
                    ));
                }
            }
        } else {
            entries.push(helpers::create_log_entry(
                LogEntryLevel::Info,
                container_name,
                "🔌 MCP Tool".to_string(),
                session_id,
                "mcp",
            ));
        }

        // Add separator for visual clarity
        entries.push(helpers::create_separator(container_name, session_id));

        WidgetOutput::MultiLine(entries)
    }

    fn render_with_result(
        &self,
        event: AgentEvent,
        result: Option<ToolResult>,
        container_name: &str,
        session_id: Uuid,
    ) -> WidgetOutput {
        let mut entries = Vec::new();

        // First render the tool call
        let tool_output = self.render(event.clone(), container_name, session_id);
        match tool_output {
            WidgetOutput::MultiLine(tool_entries) => entries.extend(tool_entries),
            WidgetOutput::Simple(entry) => entries.push(entry),
            _ => {}
        }

        // Then render the result if available
        if let Some(tool_result) = result {
            if tool_result.is_error {
                entries.push(helpers::create_log_entry(
                    LogEntryLevel::Error,
                    container_name,
                    "   ❌ MCP call failed".to_string(),
                    session_id,
                    "mcp_result",
                ));

                // Show error message
                if let Some(error_msg) = tool_result.content.as_str() {
                    entries.push(helpers::create_log_entry(
                        LogEntryLevel::Error,
                        container_name,
                        format!("   Error: {}", error_msg),
                        session_id,
                        "mcp_result",
                    ));
                }
            } else {
                entries.push(helpers::create_log_entry(
                    LogEntryLevel::Info,
                    container_name,
                    "   ✅ MCP call successful".to_string(),
                    session_id,
                    "mcp_result",
                ));

                // Show result based on content type
                match &tool_result.content {
                    Value::String(s) => {
                        // For short strings, show inline
                        if s.len() <= 200 && !s.contains('\n') {
                            entries.push(helpers::create_log_entry(
                                LogEntryLevel::Debug,
                                container_name,
                                format!("   Result: {}", s),
                                session_id,
                                "mcp_result",
                            ));
                        } else {
                            // For longer strings, show preview
                            entries.push(helpers::create_log_entry(
                                LogEntryLevel::Debug,
                                container_name,
                                "   Result:".to_string(),
                                session_id,
                                "mcp_result",
                            ));
                            for line in s.lines().take(10) {
                                entries.push(helpers::create_log_entry(
                                    LogEntryLevel::Debug,
                                    container_name,
                                    format!("      {}", line),
                                    session_id,
                                    "mcp_result",
                                ));
                            }
                            if s.lines().count() > 10 {
                                entries.push(helpers::create_log_entry(
                                    LogEntryLevel::Debug,
                                    container_name,
                                    "      ... (truncated)".to_string(),
                                    session_id,
                                    "mcp_result",
                                ));
                            }
                        }
                    }
                    Value::Object(obj) => {
                        entries.push(helpers::create_log_entry(
                            LogEntryLevel::Debug,
                            container_name,
                            format!("   Result: {} fields", obj.len()),
                            session_id,
                            "mcp_result",
                        ));
                        // Show first few fields
                        for (key, value) in obj.iter().take(5) {
                            let value_preview = match value {
                                Value::String(s) if s.len() > 50 => format!("\"{}...\"", &s[..50]),
                                Value::String(s) => format!("\"{}\"", s),
                                Value::Number(n) => n.to_string(),
                                Value::Bool(b) => b.to_string(),
                                Value::Null => "null".to_string(),
                                Value::Array(arr) => format!("[{} items]", arr.len()),
                                Value::Object(obj) => format!("{{{} fields}}", obj.len()),
                            };
                            entries.push(helpers::create_log_entry(
                                LogEntryLevel::Debug,
                                container_name,
                                format!("      {}: {}", key, value_preview),
                                session_id,
                                "mcp_result",
                            ));
                        }
                        if obj.len() > 5 {
                            entries.push(helpers::create_log_entry(
                                LogEntryLevel::Debug,
                                container_name,
                                format!("      ... and {} more fields", obj.len() - 5),
                                session_id,
                                "mcp_result",
                            ));
                        }
                    }
                    Value::Array(arr) => {
                        entries.push(helpers::create_log_entry(
                            LogEntryLevel::Debug,
                            container_name,
                            format!("   Result: {} items", arr.len()),
                            session_id,
                            "mcp_result",
                        ));
                    }
                    _ => {
                        entries.push(helpers::create_log_entry(
                            LogEntryLevel::Debug,
                            container_name,
                            format!("   Result: {:?}", tool_result.content),
                            session_id,
                            "mcp_result",
                        ));
                    }
                }
            }
        }

        WidgetOutput::MultiLine(entries)
    }

    fn name(&self) -> &'static str {
        "McpWidget"
    }
}

impl Default for McpWidget {
    fn default() -> Self {
        Self::new()
    }
}