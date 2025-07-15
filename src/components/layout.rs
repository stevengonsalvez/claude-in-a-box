// ABOUTME: Main layout component handling split-pane arrangement and bottom menu bar

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
    style::{Color, Style},
};

use crate::app::AppState;
use super::{SessionListComponent, LogsViewerComponent, HelpComponent};

pub struct LayoutComponent {
    session_list: SessionListComponent,
    logs_viewer: LogsViewerComponent,
    help: HelpComponent,
}

impl LayoutComponent {
    pub fn new() -> Self {
        Self {
            session_list: SessionListComponent::new(),
            logs_viewer: LogsViewerComponent::new(),
            help: HelpComponent::new(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, state: &AppState) {
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),     // Main content
                Constraint::Length(3),  // Bottom menu bar
            ])
            .split(frame.size());

        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40),  // Session list
                Constraint::Percentage(60),  // Right pane
            ])
            .split(main_chunks[0]);

        // Render session list
        self.session_list.render(frame, content_chunks[0], state);

        // Render right pane (logs/terminal/etc)
        self.logs_viewer.render(frame, content_chunks[1], state);

        // Render bottom menu bar
        self.render_menu_bar(frame, main_chunks[1]);

        // Render help overlay if visible
        if state.help_visible {
            self.help.render(frame, frame.size());
        }
    }

    fn render_menu_bar(&self, frame: &mut Frame, area: Rect) {
        let menu_text = "[n]ew [a]ttach [s]tart/stop [d]elete [w]orkspace [?]help [q]uit";
        
        let menu = Paragraph::new(menu_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);

        frame.render_widget(menu, area);
    }
}

impl Default for LayoutComponent {
    fn default() -> Self {
        Self::new()
    }
}