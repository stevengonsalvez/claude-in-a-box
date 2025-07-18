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

        // Create a full-screen terminal view
        let terminal_content = if state.terminal_process.is_some() {
            vec![
                "🔗 Container Terminal Active".to_string(),
                "".to_string(),
                "Terminal session is running in the background.".to_string(),
                "Interactive shell (bash) is active in the container.".to_string(),
                "".to_string(),
                "The terminal is running via 'docker exec -it' with full TTY support.".to_string(),
                "Input/output is handled by the docker process.".to_string(),
                "".to_string(),
                "Commands:".to_string(),
                "  [d] - Detach from container (keeps container running)".to_string(),
                "  [q] - Detach from container".to_string(),
                "  [k] - Kill container (force stop and remove)".to_string(),
                "  [Esc] - Detach from container".to_string(),
                "".to_string(),
                "Status: ✅ Terminal process active".to_string(),
            ]
        } else {
            vec![
                "🔗 Container Terminal".to_string(),
                "".to_string(),
                "No active terminal process found.".to_string(),
                "".to_string(),
                "Commands:".to_string(),
                "  [d] - Detach from container".to_string(),
                "  [q] - Detach from container".to_string(),
                "  [k] - Kill container (force stop and remove)".to_string(),
                "  [Esc] - Detach from container".to_string(),
                "".to_string(),
                "Status: ❌ No terminal process".to_string(),
            ]
        };

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

        let status_text = "[d] Detach  [k] Kill  [q] Quit  [Esc] Exit";
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