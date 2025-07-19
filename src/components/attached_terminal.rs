// ABOUTME: Attached terminal component for full-screen container interaction

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
    style::{Color, Modifier, Style},
};

use crate::app::AppState;

pub struct AttachedTerminalComponent;

impl AttachedTerminalComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        if let Some(session_id) = state.attached_session_id {
            self.render_attached_terminal(frame, area, state, session_id);
        } else {
            self.render_error_state(frame, area);
        }
    }

    fn render_attached_terminal(&self, frame: &mut Frame, area: Rect, state: &AppState, session_id: uuid::Uuid) {
        // Get session info
        let session = state.workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id);

        let title = if let Some(session) = session {
            format!("Attached to: {} ({})", session.name, session_id.to_string()[..8].to_string())
        } else {
            format!("Attached to session: {}", session_id.to_string()[..8].to_string())
        };

        // Display session information since we use blocking execution
        let terminal_content = vec![
            "🔗 Session Container".to_string(),
            "".to_string(),
            "This session has an active container with Claude CLI and MCP servers.".to_string(),
            "".to_string(),
            "Available MCP Servers:".to_string(),
            "  • Serena - AI coding agent toolkit".to_string(),
            "  • Context7 - Library documentation and examples".to_string(),
            "  • Twilio - SMS notifications (if configured)".to_string(),
            "".to_string(),
            "Actions:".to_string(),
            "  • Press [a] to attach to Claude CLI".to_string(),
            "  • Press [k] to kill container".to_string(),
            "  • Press [Esc] to return to session list".to_string(),
            "".to_string(),
            "Note: Attaching will temporarily exit this interface".to_string(),
            "and give you full terminal control inside the container.".to_string(),
        ];

        let content_text = terminal_content.join("\n");

        let paragraph = Paragraph::new(content_text)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
            )
            .style(Style::default().fg(Color::White))
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(paragraph, area);

        // Add status bar at the bottom
        let status_area = Rect {
            x: area.x,
            y: area.y + area.height - 3,
            width: area.width,
            height: 3,
        };

        let status_text = "[a] Attach to Claude CLI  |  [k] Kill Container  |  [Esc] Return to Session List";
        let status_paragraph = Paragraph::new(status_text)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::Yellow))
            )
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);

        frame.render_widget(status_paragraph, status_area);
    }

    fn render_error_state(&self, frame: &mut Frame, area: Rect) {
        let error_text = "Error: No attached session found";
        
        let paragraph = Paragraph::new(error_text)
            .block(
                Block::default()
                    .title("Terminal Error")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red))
            )
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }
}

impl Default for AttachedTerminalComponent {
    fn default() -> Self {
        Self::new()
    }
}