// ABOUTME: Main layout component handling split-pane arrangement and bottom menu bar

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
    style::{Color, Style},
};

use crate::app::{AppState, state::View};
use super::{SessionListComponent, LogsViewerComponent, HelpComponent, NewSessionComponent, ConfirmationDialogComponent, NonGitNotificationComponent, AttachedTerminalComponent, AuthSetupComponent};

pub struct LayoutComponent {
    session_list: SessionListComponent,
    logs_viewer: LogsViewerComponent,
    help: HelpComponent,
    new_session: NewSessionComponent,
    confirmation_dialog: ConfirmationDialogComponent,
    non_git_notification: NonGitNotificationComponent,
    attached_terminal: AttachedTerminalComponent,
    auth_setup: AuthSetupComponent,
}

impl LayoutComponent {
    pub fn new() -> Self {
        Self {
            session_list: SessionListComponent::new(),
            logs_viewer: LogsViewerComponent::new(),
            help: HelpComponent::new(),
            new_session: NewSessionComponent::new(),
            confirmation_dialog: ConfirmationDialogComponent::new(),
            non_git_notification: NonGitNotificationComponent::new(),
            attached_terminal: AttachedTerminalComponent::new(),
            auth_setup: AuthSetupComponent::new(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, state: &AppState) {
        // Special handling for auth setup view (full screen)
        if state.current_view == View::AuthSetup {
            let centered_area = centered_rect(60, 60, frame.size());
            self.auth_setup.render(frame, centered_area, state);
            return;
        }
        
        // Special handling for non-git notification view
        if state.current_view == View::NonGitNotification {
            self.non_git_notification.render(frame, frame.size(), state);
            return;
        }

        // Special handling for attached terminal view (full screen)
        if state.current_view == View::AttachedTerminal {
            self.attached_terminal.render(frame, frame.size(), state);
            return;
        }

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

        // Render new session overlay if visible
        if state.current_view == View::NewSession || state.current_view == View::SearchWorkspace {
            self.new_session.render(frame, frame.size(), state);
        }

        // Render confirmation dialog if visible (highest priority overlay)
        if state.confirmation_dialog.is_some() {
            self.confirmation_dialog.render(frame, frame.size(), state);
        }
    }

    fn render_menu_bar(&self, frame: &mut Frame, area: Rect) {
        let menu_text = "[n]ew [s]earch [a]ttach [r]un/stop [d]elete [f]refresh [?]help [q]uit";
        
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

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}