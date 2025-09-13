// ABOUTME: Test program to demonstrate hierarchical widget display
// Shows how tool calls and results are rendered with the new system

use claude_box::agent_parsers::AgentEvent;
use claude_box::widgets::{WidgetRegistry, WidgetOutput, ToolResult, MessageWidget};
use claude_box::components::live_logs_stream::LogEntry;
use uuid::Uuid;
use serde_json::json;

fn main() {
    println!("Testing Hierarchical Widget Display\n");
    println!("{}", "=".repeat(50));

    let registry = WidgetRegistry::new();
    let session_id = Uuid::new_v4();
    let container_name = "test-container";

    // Test 1: Bash command without result
    println!("\n1. Bash command (no result yet):");
    println!("{}", "-".repeat(40));

    let bash_event = AgentEvent::ToolCall {
        id: "bash_001".to_string(),
        name: "Bash".to_string(),
        input: json!({
            "command": "cargo test --lib",
            "description": "Running library tests"
        }),
        description: Some("Run unit tests".to_string()),
    };

    let output = registry.render(bash_event.clone(), container_name, session_id);
    print_output(&output);

    // Test 2: Bash command with markdown result
    println!("\n2. Bash command with markdown result:");
    println!("{}", "-".repeat(40));

    let result = ToolResult {
        tool_use_id: "bash_001".to_string(),
        content: json!({
            "content": "# Test Results\n\nRunning **28 tests**\n\n```\ntest widgets::tests::test_bash ... ok\ntest widgets::tests::test_edit ... ok\ntest widgets::tests::test_todo ... ok\n```\n\n✅ All tests passed!"
        }),
        is_error: false,
    };

    // Since we can't directly pass result to render, we'll demonstrate the hierarchical output
    let bash_widget = claude_box::widgets::BashWidget::new();
    let output_with_result = bash_widget.render_with_result(
        bash_event,
        Some(result),
        container_name,
        session_id,
    );
    print_output(&output_with_result);

    // Test 3: Todo widget
    println!("\n3. Todo widget:");
    println!("{}", "-".repeat(40));

    let todo_event = AgentEvent::ToolCall {
        id: "todo_001".to_string(),
        name: "TodoWrite".to_string(),
        input: json!({
            "todos": [
                {
                    "content": "Implement widget system",
                    "status": "completed",
                    "activeForm": "Implementing widget system"
                },
                {
                    "content": "Add markdown parsing",
                    "status": "in_progress",
                    "activeForm": "Adding markdown parsing"
                },
                {
                    "content": "Test hierarchical display",
                    "status": "pending",
                    "activeForm": "Testing hierarchical display"
                }
            ]
        }),
        description: Some("Update task list".to_string()),
    };

    let output = registry.render(todo_event, container_name, session_id);
    print_output(&output);

    // Test 4: Thinking widget
    println!("\n4. Thinking widget:");
    println!("{}", "-".repeat(40));

    let thinking_event = AgentEvent::Thinking {
        content: "Analyzing the code structure...\nLooks like we need to:\n1. Parse markdown\n2. Format hierarchically\n3. Display in TUI".to_string(),
    };

    let output = registry.render(thinking_event, container_name, session_id);
    print_output(&output);

    println!("\n{}", "=".repeat(50));
    println!("Test complete!");
}

fn print_output(output: &WidgetOutput) {
    match output {
        WidgetOutput::Simple(entry) => {
            println!("{}", format_entry(entry));
        }
        WidgetOutput::MultiLine(entries) => {
            for entry in entries {
                println!("{}", format_entry(entry));
            }
        }
        WidgetOutput::Hierarchical { header, content, collapsed } => {
            println!("📦 HIERARCHICAL OUTPUT (collapsed: {}):", collapsed);
            println!("  Header:");
            for entry in header {
                println!("    {}", format_entry(entry));
            }
            if !content.is_empty() {
                println!("  Content:");
                for entry in content {
                    println!("    {}", format_entry(entry));
                }
            }
        }
        WidgetOutput::Interactive(_) => {
            println!("[Interactive widget - not supported in this test]");
        }
    }
}

fn format_entry(entry: &LogEntry) -> String {
    let level_icon = match entry.level {
        claude_box::components::live_logs_stream::LogEntryLevel::Error => "❌",
        claude_box::components::live_logs_stream::LogEntryLevel::Warn => "⚠️",
        claude_box::components::live_logs_stream::LogEntryLevel::Info => "ℹ️",
        claude_box::components::live_logs_stream::LogEntryLevel::Debug => "🔍",
    };
    format!("{} {}", level_icon, entry.message)
}
