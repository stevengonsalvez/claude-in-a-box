// ABOUTME: Central message router for directing JSON events to appropriate widgets
// Similar to Opcode's StreamMessage.tsx, routes different message types to specialized widgets

use crate::agent_parsers::{AgentEvent, types::StructuredPayload};
use crate::components::live_logs_stream::{LogEntry, LogEntryLevel};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;
use tracing::{debug, warn};

use super::{
    MessageWidget, WidgetOutput, ToolResult,
    BashWidget, EditWidget, TodoWidget, DefaultWidget,
    ReadWidget, WriteWidget, GrepWidget, GlobWidget,
    TaskWidget, WebSearchWidget, WebFetchWidget, ThinkingWidget,
    helpers,
};

/// Central message router that processes AgentEvents and tool results
pub struct MessageRouter {
    /// Map of tool_use_id to ToolResult for matching tool calls with their results
    tool_results: HashMap<String, ToolResult>,

    /// Specialized widgets for different message types
    bash_widget: BashWidget,
    edit_widget: EditWidget,
    todo_widget: TodoWidget,
    read_widget: ReadWidget,
    write_widget: WriteWidget,
    grep_widget: GrepWidget,
    glob_widget: GlobWidget,
    task_widget: TaskWidget,
    websearch_widget: WebSearchWidget,
    webfetch_widget: WebFetchWidget,
    thinking_widget: ThinkingWidget,
    default_widget: DefaultWidget,

    // New widgets to be implemented
    multiedit_widget: Option<Box<dyn MessageWidget>>,
    mcp_widget: Option<Box<dyn MessageWidget>>,
    ls_result_widget: Option<Box<dyn MessageWidget>>,
    system_reminder_widget: Option<Box<dyn MessageWidget>>,
}

impl MessageRouter {
    pub fn new() -> Self {
        Self {
            tool_results: HashMap::new(),
            bash_widget: BashWidget::new(),
            edit_widget: EditWidget::new(),
            todo_widget: TodoWidget::new(),
            read_widget: ReadWidget::new(),
            write_widget: WriteWidget::new(),
            grep_widget: GrepWidget::new(),
            glob_widget: GlobWidget::new(),
            task_widget: TaskWidget::new(),
            websearch_widget: WebSearchWidget::new(),
            webfetch_widget: WebFetchWidget::new(),
            thinking_widget: ThinkingWidget::new(),
            default_widget: DefaultWidget::new(),
            multiedit_widget: None,
            mcp_widget: None,
            ls_result_widget: None,
            system_reminder_widget: None,
        }
    }

    /// Store a tool result for later matching with tool calls
    pub fn add_tool_result(&mut self, tool_use_id: String, result: ToolResult) {
        debug!("Storing tool result for ID: {}", tool_use_id);
        self.tool_results.insert(tool_use_id, result);
    }

    /// Get a tool result by its ID
    fn get_tool_result(&self, tool_use_id: &str) -> Option<&ToolResult> {
        self.tool_results.get(tool_use_id)
    }

    /// Route an event to the appropriate widget based on its type and content
    pub fn route_event(
        &self,
        event: AgentEvent,
        container_name: &str,
        session_id: Uuid,
    ) -> WidgetOutput {
        // Extract event type and payload
        let event_type = event.event_type.as_str();

        match event_type {
            // System initialization message
            "system" if event.subtype.as_deref() == Some("init") => {
                self.render_system_init(event, container_name, session_id)
            }

            // Assistant messages - check content type
            "assistant" => {
                self.route_assistant_message(event, container_name, session_id)
            }

            // User messages - often contain tool results
            "user" => {
                self.route_user_message(event, container_name, session_id)
            }

            // Result messages - execution complete/failed
            "result" => {
                self.render_result_message(event, container_name, session_id)
            }

            // Tool use messages
            "tool_use" => {
                self.route_tool_use(event, container_name, session_id)
            }

            // Tool result messages
            "tool_result" => {
                self.route_tool_result(event, container_name, session_id)
            }

            // Thinking messages
            "thinking" => {
                self.thinking_widget.render(event, container_name, session_id)
            }

            // Default fallback
            _ => {
                self.default_widget.render(event, container_name, session_id)
            }
        }
    }

    /// Route assistant messages based on their content
    fn route_assistant_message(
        &self,
        event: AgentEvent,
        container_name: &str,
        session_id: Uuid,
    ) -> WidgetOutput {
        // Check if the event has structured content
        if let Some(ref payload) = event.payload {
            // Check for content array in the payload
            if let Some(content_array) = payload.data.get("content").and_then(|v| v.as_array()) {
                let mut outputs = Vec::new();

                for content_item in content_array {
                    let content_type = content_item.get("type").and_then(|v| v.as_str());

                    match content_type {
                        Some("text") => {
                            // Text content - render as markdown
                            outputs.push(self.render_text_content(content_item, container_name, session_id));
                        }
                        Some("thinking") => {
                            // Thinking content
                            let thinking_event = AgentEvent {
                                event_type: "thinking".to_string(),
                                payload: Some(StructuredPayload {
                                    data: content_item.clone(),
                                    metadata: HashMap::new(),
                                }),
                                ..event.clone()
                            };
                            outputs.push(self.thinking_widget.render(thinking_event, container_name, session_id));
                        }
                        Some("tool_use") => {
                            // Tool use - route to appropriate widget
                            let tool_event = AgentEvent {
                                event_type: "tool_use".to_string(),
                                payload: Some(StructuredPayload {
                                    data: content_item.clone(),
                                    metadata: HashMap::new(),
                                }),
                                ..event.clone()
                            };
                            outputs.push(self.route_tool_use(tool_event, container_name, session_id));
                        }
                        _ => {
                            // Unknown content type
                            debug!("Unknown content type in assistant message: {:?}", content_type);
                        }
                    }
                }

                // Combine outputs
                if outputs.is_empty() {
                    self.default_widget.render(event, container_name, session_id)
                } else if outputs.len() == 1 {
                    outputs.into_iter().next().unwrap()
                } else {
                    self.combine_widget_outputs(outputs)
                }
            } else {
                // No content array, use default rendering
                self.default_widget.render(event, container_name, session_id)
            }
        } else {
            self.default_widget.render(event, container_name, session_id)
        }
    }

    /// Route tool use events to specific tool widgets
    fn route_tool_use(
        &self,
        event: AgentEvent,
        container_name: &str,
        session_id: Uuid,
    ) -> WidgetOutput {
        // Extract tool name and check for tool result
        let tool_name = event.payload.as_ref()
            .and_then(|p| p.data.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        let tool_id = event.payload.as_ref()
            .and_then(|p| p.data.get("id"))
            .and_then(|v| v.as_str());

        // Get associated tool result if available
        let tool_result = tool_id.and_then(|id| self.get_tool_result(id));

        // Route based on tool name
        match tool_name.as_str() {
            "bash" => {
                if let Some(result) = tool_result {
                    self.bash_widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                } else {
                    self.bash_widget.render(event, container_name, session_id)
                }
            }
            "edit" => {
                if let Some(result) = tool_result {
                    self.edit_widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                } else {
                    self.edit_widget.render(event, container_name, session_id)
                }
            }
            "multiedit" => {
                // Use multiedit widget if available, otherwise fallback
                if let Some(ref widget) = self.multiedit_widget {
                    if let Some(result) = tool_result {
                        widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                    } else {
                        widget.render(event, container_name, session_id)
                    }
                } else {
                    self.default_widget.render(event, container_name, session_id)
                }
            }
            "todowrite" => {
                if let Some(result) = tool_result {
                    self.todo_widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                } else {
                    self.todo_widget.render(event, container_name, session_id)
                }
            }
            "read" => {
                if let Some(result) = tool_result {
                    self.read_widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                } else {
                    self.read_widget.render(event, container_name, session_id)
                }
            }
            "write" => {
                if let Some(result) = tool_result {
                    self.write_widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                } else {
                    self.write_widget.render(event, container_name, session_id)
                }
            }
            "grep" => {
                if let Some(result) = tool_result {
                    self.grep_widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                } else {
                    self.grep_widget.render(event, container_name, session_id)
                }
            }
            "glob" => {
                if let Some(result) = tool_result {
                    self.glob_widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                } else {
                    self.glob_widget.render(event, container_name, session_id)
                }
            }
            "task" => {
                if let Some(result) = tool_result {
                    self.task_widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                } else {
                    self.task_widget.render(event, container_name, session_id)
                }
            }
            "websearch" => {
                if let Some(result) = tool_result {
                    self.websearch_widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                } else {
                    self.websearch_widget.render(event, container_name, session_id)
                }
            }
            "webfetch" => {
                if let Some(result) = tool_result {
                    self.webfetch_widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                } else {
                    self.webfetch_widget.render(event, container_name, session_id)
                }
            }
            name if name.starts_with("mcp__") => {
                // MCP tools
                if let Some(ref widget) = self.mcp_widget {
                    if let Some(result) = tool_result {
                        widget.render_with_result(event, Some(result.clone()), container_name, session_id)
                    } else {
                        widget.render(event, container_name, session_id)
                    }
                } else {
                    self.default_widget.render(event, container_name, session_id)
                }
            }
            _ => {
                // Unknown tool, use default
                self.default_widget.render(event, container_name, session_id)
            }
        }
    }

    /// Route user messages (often contain tool results)
    fn route_user_message(
        &self,
        event: AgentEvent,
        container_name: &str,
        session_id: Uuid,
    ) -> WidgetOutput {
        // Check for tool_result in content
        if let Some(ref payload) = event.payload {
            if let Some(content_array) = payload.data.get("content").and_then(|v| v.as_array()) {
                for content_item in content_array {
                    if content_item.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                        // Store the tool result for matching
                        if let (Some(tool_use_id), Some(content)) = (
                            content_item.get("tool_use_id").and_then(|v| v.as_str()),
                            content_item.get("content")
                        ) {
                            let is_error = content_item.get("is_error")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            let tool_result = ToolResult {
                                tool_use_id: tool_use_id.to_string(),
                                content: content.clone(),
                                is_error,
                            };

                            // Note: In a real implementation, we'd need mutable access here
                            // For now, we'll handle this differently
                            debug!("Found tool result for ID: {}", tool_use_id);
                        }
                    }
                }
            }
        }

        // Render the user message
        self.default_widget.render(event, container_name, session_id)
    }

    /// Route tool result events
    fn route_tool_result(
        &self,
        event: AgentEvent,
        container_name: &str,
        session_id: Uuid,
    ) -> WidgetOutput {
        // Tool results are typically handled inline with tool calls
        // This is for standalone tool results
        self.default_widget.render(event, container_name, session_id)
    }

    /// Render system initialization message
    fn render_system_init(
        &self,
        event: AgentEvent,
        container_name: &str,
        session_id: Uuid,
    ) -> WidgetOutput {
        let mut entries = Vec::new();

        // Extract system info from payload
        if let Some(ref payload) = event.payload {
            let model = payload.data.get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let cwd = payload.data.get("cwd")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            entries.push(helpers::create_log_entry(
                LogEntryLevel::Info,
                container_name,
                format!("🚀 System Initialized"),
                session_id,
                "system_init",
            ));

            entries.push(helpers::create_log_entry(
                LogEntryLevel::Debug,
                container_name,
                format!("   Model: {}", model),
                session_id,
                "system_init",
            ));

            entries.push(helpers::create_log_entry(
                LogEntryLevel::Debug,
                container_name,
                format!("   Working Directory: {}", cwd),
                session_id,
                "system_init",
            ));
        } else {
            entries.push(helpers::create_log_entry(
                LogEntryLevel::Info,
                container_name,
                "🚀 System Initialized".to_string(),
                session_id,
                "system_init",
            ));
        }

        WidgetOutput::MultiLine(entries)
    }

    /// Render result message (execution complete/failed)
    fn render_result_message(
        &self,
        event: AgentEvent,
        container_name: &str,
        session_id: Uuid,
    ) -> WidgetOutput {
        let is_error = event.subtype.as_deref() == Some("error") ||
                       event.payload.as_ref()
                           .and_then(|p| p.data.get("is_error"))
                           .and_then(|v| v.as_bool())
                           .unwrap_or(false);

        let mut entries = Vec::new();

        if is_error {
            entries.push(helpers::create_log_entry(
                LogEntryLevel::Error,
                container_name,
                "❌ Execution Failed".to_string(),
                session_id,
                "result",
            ));
        } else {
            entries.push(helpers::create_log_entry(
                LogEntryLevel::Info,
                container_name,
                "✅ Execution Complete".to_string(),
                session_id,
                "result",
            ));
        }

        // Add cost and duration info if available
        if let Some(ref payload) = event.payload {
            if let Some(cost) = payload.data.get("total_cost_usd").and_then(|v| v.as_f64()) {
                entries.push(helpers::create_log_entry(
                    LogEntryLevel::Debug,
                    container_name,
                    format!("   Cost: ${:.4} USD", cost),
                    session_id,
                    "result",
                ));
            }

            if let Some(duration) = payload.data.get("duration_ms").and_then(|v| v.as_u64()) {
                entries.push(helpers::create_log_entry(
                    LogEntryLevel::Debug,
                    container_name,
                    format!("   Duration: {:.2}s", duration as f64 / 1000.0),
                    session_id,
                    "result",
                ));
            }
        }

        WidgetOutput::MultiLine(entries)
    }

    /// Render text content
    fn render_text_content(
        &self,
        content: &Value,
        container_name: &str,
        session_id: Uuid,
    ) -> WidgetOutput {
        let text = content.get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let entry = helpers::create_log_entry(
            LogEntryLevel::Info,
            container_name,
            format!("Claude: {}", text),
            session_id,
            "text",
        );

        WidgetOutput::Simple(entry)
    }

    /// Combine multiple widget outputs into one
    fn combine_widget_outputs(&self, outputs: Vec<WidgetOutput>) -> WidgetOutput {
        let mut all_entries = Vec::new();

        for output in outputs {
            match output {
                WidgetOutput::Simple(entry) => all_entries.push(entry),
                WidgetOutput::MultiLine(entries) => all_entries.extend(entries),
                WidgetOutput::Hierarchical { header, content, .. } => {
                    all_entries.extend(header);
                    all_entries.extend(content);
                }
                WidgetOutput::Interactive(component) => {
                    all_entries.push(component.base_entry);
                }
            }
        }

        WidgetOutput::MultiLine(all_entries)
    }
}

impl Default for MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}