// ABOUTME: Split-screen component that shows session list on left and tmux content on right

use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::state::AppState;
use super::{SessionListComponent, AttachedTerminalComponent};

pub struct SplitScreenComponent {
    session_list: SessionListComponent,
    terminal: AttachedTerminalComponent,
}

impl SplitScreenComponent {
    pub fn new() -> Self {
        Self {
            session_list: SessionListComponent::new(),
            terminal: AttachedTerminalComponent::new(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        // Create horizontal split layout
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Session list - left pane
                Constraint::Percentage(60), // Tmux content - right pane
            ])
            .split(area);

        // Render session list on the left
        self.session_list.render(frame, chunks[0], state);

        // Render tmux content on the right
        self.render_tmux_content(frame, chunks[1], state);
    }

    fn render_tmux_content(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        let block = Block::default()
            .title("Live Session View")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        if let Some(session) = state.get_selected_session() {
            // TODO: Implement tmux capture-pane functionality
            // For now, show a placeholder
            let content = format!(
                "Session: {}\nWorkspace: {}\nStatus: {:?}\n\n[Live tmux content will appear here]",
                session.tmux_session_name,
                session.workspace_path,
                session.status
            );

            let paragraph = Paragraph::new(content)
                .block(block)
                .wrap(ratatui::widgets::Wrap { trim: true });

            frame.render_widget(paragraph, area);
        } else {
            let paragraph = Paragraph::new("No session selected")
                .block(block)
                .style(Style::default().fg(Color::Gray));

            frame.render_widget(paragraph, area);
        }
    }
}