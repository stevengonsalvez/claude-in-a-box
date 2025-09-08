// ABOUTME: Claude JSON stream parser - parses Claude's --output-format stream-json output
// Converts Claude-specific JSON events into unified AgentEvent types for display

use super::types::{AgentEvent, AgentOutputParser, McpServerInfo, ParserState, ToolCallInfo};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Parser for Claude's stream-json output format
pub struct ClaudeJsonParser {
    state: ParserState,
}

impl ClaudeJsonParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::default(),
        }
    }
    
    fn parse_json_event(&mut self, json_str: &str) -> Result<Vec<AgentEvent>, String> {
        let value: Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;
        
        let mut events = Vec::new();
        
        // Extract type field
        let event_type = value.get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        
        match event_type {
            "system" => {
                if let Some(subtype) = value.get("subtype").and_then(|v| v.as_str()) {
                    if subtype == "init" {
                        events.push(self.parse_system_init(&value)?);
                    }
                }
            }
            
            "assistant" => {
                events.extend(self.parse_assistant_message(&value)?);
            }
            
            "user" => {
                events.extend(self.parse_user_message(&value)?);
            }
            
            _ => {
                debug!("Unknown event type: {} - {}", event_type, json_str);
            }
        }
        
        Ok(events)
    }
    
    fn parse_system_init(&mut self, value: &Value) -> Result<AgentEvent, String> {
        let model = value.get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        
        let session_id = value.get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        let tools = value.get("tools")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        
        let mcp_servers = value.get("mcp_servers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|server| {
                        let name = server.get("name")?.as_str()?;
                        let status = server.get("status")?.as_str()?;
                        Some(McpServerInfo {
                            name: name.to_string(),
                            status: status.to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            });
        
        Ok(AgentEvent::SessionInfo {
            model,
            tools,
            session_id,
            mcp_servers,
        })
    }
    
    fn parse_assistant_message(&mut self, value: &Value) -> Result<Vec<AgentEvent>, String> {
        let mut events = Vec::new();
        
        // Get message content
        if let Some(message) = value.get("message") {
            let message_id = message.get("id")
                .and_then(|v| v.as_str())
                .map(String::from);
            
            // Store message ID for streaming
            self.state.current_message_id = message_id.clone();
            
            // Check for content array
            if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
                for content_item in content_array {
                    let content_type = content_item.get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    
                    match content_type {
                        "text" => {
                            if let Some(text) = content_item.get("text").and_then(|v| v.as_str()) {
                                // Check if this is a complete message or streaming delta
                                if self.state.current_message.is_some() {
                                    // Streaming delta
                                    self.state.current_message.as_mut().unwrap().push_str(text);
                                    events.push(AgentEvent::StreamingText {
                                        delta: text.to_string(),
                                        message_id: message_id.clone(),
                                    });
                                } else {
                                    // Complete message
                                    events.push(AgentEvent::Message {
                                        content: text.to_string(),
                                        id: message_id.clone(),
                                    });
                                }
                            }
                        }
                        
                        "tool_use" => {
                            let tool_id = content_item.get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            
                            let tool_name = content_item.get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            
                            let input = content_item.get("input")
                                .cloned()
                                .unwrap_or(Value::Null);
                            
                            // Extract description from input if available
                            let description = input.get("description")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            
                            // Track active tool call
                            self.state.active_tool_calls.insert(
                                tool_id.clone(),
                                ToolCallInfo {
                                    id: tool_id.clone(),
                                    name: tool_name.clone(),
                                    started_at: chrono::Utc::now(),
                                }
                            );
                            
                            events.push(AgentEvent::ToolCall {
                                id: tool_id,
                                name: tool_name,
                                input,
                                description,
                            });
                        }
                        
                        _ => {
                            debug!("Unknown content type in assistant message: {}", content_type);
                        }
                    }
                }
            }
            
            // Check for usage information
            if let Some(usage) = message.get("usage") {
                let input_tokens = usage.get("input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                
                let output_tokens = usage.get("output_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                
                let cache_tokens = usage.get("cache_read_input_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32);
                
                events.push(AgentEvent::Usage {
                    input_tokens,
                    output_tokens,
                    cache_tokens,
                    total_cost: None, // Can be calculated externally if needed
                });
            }
        }
        
        Ok(events)
    }
    
    fn parse_user_message(&mut self, value: &Value) -> Result<Vec<AgentEvent>, String> {
        let mut events = Vec::new();
        
        // Check for tool results in user messages
        if let Some(message) = value.get("message") {
            if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
                for content_item in content_array {
                    if let Some(tool_result) = content_item.get("tool_result") {
                        let tool_use_id = content_item.get("tool_use_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        
                        let content = tool_result.get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        
                        let is_error = tool_result.get("is_error")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        
                        // Remove from active tool calls
                        self.state.active_tool_calls.remove(&tool_use_id);
                        
                        events.push(AgentEvent::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        });
                    } else if content_item.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                        // Alternative format for tool results
                        let tool_use_id = content_item.get("tool_use_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        
                        let content = content_item.get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        
                        let is_error = content_item.get("is_error")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        
                        // Remove from active tool calls
                        self.state.active_tool_calls.remove(&tool_use_id);
                        
                        events.push(AgentEvent::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        });
                    }
                }
            }
        }
        
        Ok(events)
    }
}

impl AgentOutputParser for ClaudeJsonParser {
    fn parse_line(&mut self, line: &str) -> Result<Vec<AgentEvent>, String> {
        // Handle incomplete lines by buffering
        let complete_line = if !self.state.line_buffer.is_empty() {
            let buffered = format!("{}{}", self.state.line_buffer, line);
            self.state.line_buffer.clear();
            buffered
        } else {
            line.to_string()
        };
        
        // Skip empty lines
        if complete_line.trim().is_empty() {
            return Ok(vec![]);
        }
        
        // Try to parse as JSON
        match self.parse_json_event(&complete_line) {
            Ok(events) => Ok(events),
            Err(e) => {
                // If parsing fails, it might be an incomplete line
                if line.ends_with('}') {
                    // Complete JSON that failed to parse
                    warn!("Failed to parse complete JSON line: {} - Error: {}", complete_line, e);
                    Err(e)
                } else {
                    // Incomplete line, buffer it
                    self.state.line_buffer = complete_line;
                    Ok(vec![])
                }
            }
        }
    }
    
    fn flush(&mut self) -> Vec<AgentEvent> {
        let mut events = Vec::new();
        
        // Flush any buffered message
        if let Some(message) = self.state.current_message.take() {
            events.push(AgentEvent::Message {
                content: message,
                id: self.state.current_message_id.take(),
            });
        }
        
        // Clear line buffer
        self.state.line_buffer.clear();
        
        events
    }
    
    fn agent_type(&self) -> &str {
        "claude-json"
    }
    
    fn reset(&mut self) {
        self.state = ParserState::default();
    }
}

impl Default for ClaudeJsonParser {
    fn default() -> Self {
        Self::new()
    }
}