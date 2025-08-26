// ABOUTME: Logs viewer component for displaying container logs and session information

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::app::AppState;

pub struct LogsViewerComponent;

impl LogsViewerComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        if let Some(session) = state.selected_session() {
            self.render_session_info(frame, area, state, session);
        } else {
            self.render_empty_state(frame, area);
        }
    }

    fn render_session_info(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        session: &crate::models::Session,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6), // Session info
                Constraint::Min(0),    // Logs
            ])
            .split(area);

        // Session info panel
        let info_text = format!(
            "Session: {}\nStatus: {} {}\nBranch: {}\nChanges: {}\nCreated: {}",
            session.name,
            session.status.indicator(),
            match &session.status {
                crate::models::SessionStatus::Running => "Running",
                crate::models::SessionStatus::Stopped => "Stopped",
                crate::models::SessionStatus::Error(err) => err,
            },
            session.branch_name,
            session.git_changes.format(),
            session.created_at.format("%Y-%m-%d %H:%M:%S")
        );

        let info_paragraph = Paragraph::new(info_text)
            .block(
                Block::default()
                    .title("Session Info")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(info_paragraph, chunks[0]);

        // Logs panel
        let logs_items = self.get_session_logs(state, session);
        let logs_list = List::new(logs_items).block(
            Block::default()
                .title("Logs")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(logs_list, chunks[1]);
    }

    fn render_empty_state(&self, frame: &mut Frame, area: Rect) {
        let paragraph = Paragraph::new("Select a session to view details and logs")
            .block(
                Block::default()
                    .title("Session Details")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    #[allow(elided_lifetimes_in_paths)]
    fn get_session_logs(
        &self,
        state: &AppState,
        session: &crate::models::Session,
    ) -> Vec<ListItem> {
        // First check if we have real logs for this session
        if let Some(logs) = state.logs.get(&session.id) {
            if !logs.is_empty() {
                return logs.iter().map(|log| ListItem::new(log.clone())).collect();
            }
        }

        // Fallback to status-based mock logs
        self.get_mock_logs(session)
    }

    #[allow(elided_lifetimes_in_paths)]
    fn get_mock_logs(&self, session: &crate::models::Session) -> Vec<ListItem> {
        match session.status {
            crate::models::SessionStatus::Running => vec![
                ListItem::new("Starting Claude Code environment...")
                    .style(Style::default().fg(Color::Blue)),
                ListItem::new("Loading MCP servers...").style(Style::default().fg(Color::Blue)),
                ListItem::new("✓ Connected to container claude-abc123")
                    .style(Style::default().fg(Color::Green)),
                ListItem::new("✓ Workspace mounted: /workspace")
                    .style(Style::default().fg(Color::Green)),
                ListItem::new("✓ Git worktree ready").style(Style::default().fg(Color::Green)),
                ListItem::new("Ready! Attached to container.")
                    .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                ListItem::new("").style(Style::default()),
                ListItem::new("> claude help").style(Style::default().fg(Color::Yellow)),
                ListItem::new("Available commands:").style(Style::default()),
                ListItem::new("  help     Show this help message").style(Style::default()),
                ListItem::new("  list     List files in workspace").style(Style::default()),
                ListItem::new("  run      Execute command").style(Style::default()),
            ],
            crate::models::SessionStatus::Stopped => vec![
                ListItem::new("Container stopped").style(Style::default().fg(Color::Gray)),
                ListItem::new("Last active: 2 minutes ago").style(Style::default().fg(Color::Gray)),
            ],
            crate::models::SessionStatus::Error(ref err) => vec![
                ListItem::new("Starting Claude Code environment...")
                    .style(Style::default().fg(Color::Blue)),
                ListItem::new("Loading MCP servers...").style(Style::default().fg(Color::Blue)),
                ListItem::new(format!("✗ Error: {}", err))
                    .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                ListItem::new("Container failed to start").style(Style::default().fg(Color::Red)),
            ],
        }
    }
}

impl Default for LogsViewerComponent {
    fn default() -> Self {
        Self::new()
    }
}
