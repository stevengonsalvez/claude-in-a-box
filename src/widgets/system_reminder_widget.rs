// ABOUTME: Widget for rendering system reminder messages
// Displays important system notifications and reminders in a highlighted format

use crate::agent_parsers::{AgentEvent, types::StructuredPayload};
use crate::components::live_logs_stream::{LogEntry, LogEntryLevel};
use serde_json::Value;
use uuid::Uuid;

use super::{MessageWidget, WidgetOutput, helpers};

pub struct SystemReminderWidget;

impl SystemReminderWidget {
    pub fn new() -> Self {
        Self
    }

    /// Extract system reminder content from various formats
    fn extract_reminder_content(content: &str) -> Option<String> {
        // Look for <system-reminder> tags
        if let Some(start) = content.find("<system-reminder>") {
            if let Some(end) = content.find("</system-reminder>") {
                let reminder_start = start + "<system-reminder>".len();
                if reminder_start < end {
                    return Some(content[reminder_start..end].trim().to_string());
                }
            }
        }
        None
    }

    /// Format reminder message for display
    fn format_reminder_message(message: &str) -> Vec<String> {
        let mut lines = Vec::new();

        // Split message into lines, respecting paragraph breaks
        for paragraph in message.split("\n\n") {
            // Word wrap long paragraphs
            let words: Vec<&str> = paragraph.split_whitespace().collect();
            let mut current_line = String::new();
            const MAX_WIDTH: usize = 100;

            for word in words {
                if current_line.is_empty() {
                    current_line = word.to_string();
                } else if current_line.len() + word.len() + 1 <= MAX_WIDTH {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    lines.push(current_line);
                    current_line = word.to_string();
                }
            }

            if !current_line.is_empty() {
                lines.push(current_line);
            }

            // Add blank line between paragraphs
            if !lines.is_empty() {
                lines.push(String::new());
            }
        }

        // Remove trailing empty line
        if lines.last() == Some(&String::new()) {
            lines.pop();
        }

        lines
    }
}

impl MessageWidget for SystemReminderWidget {
    fn can_handle(&self, event: &AgentEvent) -> bool {
        // Check if this is a system reminder
        if event.event_type == "system_reminder" {
            return true;
        }

        // Also check for system reminders embedded in tool results
        if event.event_type == "tool_result" {
            if let Some(ref payload) = event.payload {
                if let Some(content) = payload.data.get("content").and_then(|v| v.as_str()) {
                    return content.contains("<system-reminder>");
                }
            }
        }

        false
    }

    fn render(&self, event: AgentEvent, container_name: &str, session_id: Uuid) -> WidgetOutput {
        let mut entries = Vec::new();

        // Extract reminder content
        let reminder_content = if let Some(ref payload) = event.payload {
            if event.event_type == "system_reminder" {
                // Direct system reminder event
                payload.data.get("message")
                    .or_else(|| payload.data.get("content"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                // Embedded in tool result
                payload.data.get("content")
                    .and_then(|v| v.as_str())
                    .and_then(Self::extract_reminder_content)
            }
        } else {
            None
        };

        if let Some(message) = reminder_content {
            // Header with attention icon
            entries.push(helpers::create_log_entry(
                LogEntryLevel::Warn,
                container_name,
                "⚠️  System Reminder".to_string(),
                session_id,
                "system_reminder",
            ));

            // Add top border
            entries.push(helpers::create_log_entry(
                LogEntryLevel::Warn,
                container_name,
                "   ┌────────────────────────────────────────┐".to_string(),
                session_id,
                "system_reminder",
            ));

            // Format and display the message
            let formatted_lines = Self::format_reminder_message(&message);
            for line in formatted_lines {
                if line.is_empty() {
                    entries.push(helpers::create_log_entry(
                        LogEntryLevel::Warn,
                        container_name,
                        "   │                                        │".to_string(),
                        session_id,
                        "system_reminder",
                    ));
                } else {
                    // Pad line to fit in box
                    let padded = format!("{:<40}", line);
                    entries.push(helpers::create_log_entry(
                        LogEntryLevel::Warn,
                        container_name,
                        format!("   │ {} │", padded),
                        session_id,
                        "system_reminder",
                    ));
                }
            }

            // Add bottom border
            entries.push(helpers::create_log_entry(
                LogEntryLevel::Warn,
                container_name,
                "   └────────────────────────────────────────┘".to_string(),
                session_id,
                "system_reminder",
            ));
        } else {
            // Fallback if no content found
            entries.push(helpers::create_log_entry(
                LogEntryLevel::Warn,
                container_name,
                "⚠️  System Reminder (no content)".to_string(),
                session_id,
                "system_reminder",
            ));
        }

        // Add separator for visual clarity
        entries.push(helpers::create_separator(container_name, session_id));

        WidgetOutput::MultiLine(entries)
    }

    fn name(&self) -> &'static str {
        "SystemReminderWidget"
    }
}

impl Default for SystemReminderWidget {
    fn default() -> Self {
        Self::new()
    }
}