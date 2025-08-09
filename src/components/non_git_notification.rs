// ABOUTME: Notification component for non-git directory warning and guidance

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::AppState;

pub struct NonGitNotificationComponent;

impl NonGitNotificationComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, _state: &AppState) {
        let text = vec![
            Line::from(vec![Span::styled(
                "⚠️  Not a Git Repository",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from("The current directory is not a Git repository."),
            Line::from("Claude-in-a-Box requires a Git repository to create development sessions."),
            Line::from(""),
            Line::from("Options:"),
            Line::from(vec![
                Span::styled(
                    "  s",
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" - Search for workspaces"),
            ]),
            Line::from(vec![
                Span::styled(
                    "  q",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" - Quit application"),
            ]),
            Line::from(""),
            Line::from(
                "Tip: Navigate to a Git repository directory and run claude-in-a-box again.",
            ),
        ];

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title("Claude-in-a-Box")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, area);
    }
}

impl Default for NonGitNotificationComponent {
    fn default() -> Self {
        Self::new()
    }
}
