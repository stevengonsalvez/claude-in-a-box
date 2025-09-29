// ABOUTME: Main layout component handling split-pane arrangement and bottom menu bar

use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

use super::{
    AttachedTerminalComponent, AuthSetupComponent, ClaudeChatComponent,
    ConfirmationDialogComponent, HelpComponent, LiveLogsStreamComponent, LogsViewerComponent,
    NewSessionComponent, NonGitNotificationComponent, SessionListComponent, SplitScreenComponent,
};
use crate::app::{AppState, state::View};

pub struct LayoutComponent {
    session_list: SessionListComponent,
    logs_viewer: LogsViewerComponent,
    claude_chat: ClaudeChatComponent,
    live_logs_stream: LiveLogsStreamComponent,
    help: HelpComponent,
    new_session: NewSessionComponent,
    confirmation_dialog: ConfirmationDialogComponent,
    non_git_notification: NonGitNotificationComponent,
    attached_terminal: AttachedTerminalComponent,
    auth_setup: AuthSetupComponent,
    split_screen: SplitScreenComponent,
}

impl LayoutComponent {
    pub fn new() -> Self {
        Self {
            session_list: SessionListComponent::new(),
            logs_viewer: LogsViewerComponent::new(),
            claude_chat: ClaudeChatComponent::new(),
            live_logs_stream: LiveLogsStreamComponent::new(),
            help: HelpComponent::new(),
            new_session: NewSessionComponent::new(),
            confirmation_dialog: ConfirmationDialogComponent::new(),
            non_git_notification: NonGitNotificationComponent::new(),
            attached_terminal: AttachedTerminalComponent::new(),
            auth_setup: AuthSetupComponent::new(),
            split_screen: SplitScreenComponent::new(),
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

        // Special handling for git view (full screen)
        if state.current_view == View::GitView {
            if let Some(ref git_state) = state.git_view_state {
                crate::components::GitViewComponent::render(frame, frame.size(), git_state);
            }
            return;
        }

        // Special handling for split-screen view (full screen)
        if state.current_view == View::SplitScreen {
            self.split_screen.render(frame, frame.size(), state);
            return;
        }

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Top status bar
                Constraint::Min(0),    // Main content area
                Constraint::Length(5), // Bottom logs area
                Constraint::Length(3), // Bottom menu bar
            ])
            .split(frame.size());

        // Render top status bar
        self.render_status_bar(frame, main_layout[0], state);

        // Simple 2-panel layout: session list | logs (Claude chat is now a popup)
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Session list
                Constraint::Percentage(60), // Live logs stream
            ])
            .split(main_layout[1]);

        // Pass focus information to components
        self.session_list.render(frame, content_chunks[0], state);
        self.live_logs_stream.render(frame, content_chunks[1], state);

        // Render bottom logs area (traditional logs viewer)
        self.logs_viewer.render(frame, main_layout[2], state);

        // Render bottom menu bar
        self.render_menu_bar(frame, main_layout[3]);

        // Render help overlay if visible
        if state.help_visible {
            self.help.render(frame, frame.size());
        }

        // Render new session overlay if visible
        if state.current_view == View::NewSession || state.current_view == View::SearchWorkspace {
            self.new_session.render(frame, frame.size(), state);
        }

        // Render Claude chat popup if visible
        if state.current_view == View::ClaudeChat {
            let popup_area = centered_rect(80, 80, frame.size());
            self.claude_chat.render(frame, popup_area, state);
        }

        // Render confirmation dialog if visible (highest priority overlay)
        if state.confirmation_dialog.is_some() {
            self.confirmation_dialog.render(frame, frame.size(), state);
        }

        // Render quick commit dialog if visible
        if state.is_in_quick_commit_mode() {
            self.render_quick_commit_dialog(frame, frame.size(), state);
        }

        // Render notifications (top-right corner)
        self.render_notifications(frame, frame.size(), state);
    }

    /// Get mutable reference to live logs component for scroll handling
    pub fn live_logs_mut(&mut self) -> &mut LiveLogsStreamComponent {
        &mut self.live_logs_stream
    }

    fn render_menu_bar(&self, frame: &mut Frame, area: Rect) {
        let menu_text = "[n]ew [s]earch [a]ttach [e]restart [g]it [p]commit [c]laude [f]refresh [x]cleanup [Tab]focus [r]e-auth [d]elete [?]help [q]uit";

        let menu = Paragraph::new(menu_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);

        frame.render_widget(menu, area);
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        let mut status_parts = vec![];

        // Current workspace/repo info
        if let Some(workspace_idx) = state.selected_workspace_index {
            if let Some(workspace) = state.workspaces.get(workspace_idx) {
                if let Some(repo_name) = workspace.path.file_name().and_then(|n| n.to_str()) {
                    status_parts.push(format!("📁 {}", repo_name));
                }
            }
        }

        // Active session info
        if let Some(_session_id) = state.get_selected_session_id() {
            if let Some(workspace_idx) = state.selected_workspace_index {
                if let Some(session_idx) = state.selected_session_index {
                    if let Some(workspace) = state.workspaces.get(workspace_idx) {
                        if let Some(session) = workspace.sessions.get(session_idx) {
                            // Branch info
                            status_parts.push(format!("🌿 {}", session.branch_name));

                            // Tmux session info
                            let tmux_name = &session.tmux_session_name;
                            let status_icon = match session.status {
                                crate::models::SessionStatus::Created => "⚪",
                                crate::models::SessionStatus::Running => "🟢",
                                crate::models::SessionStatus::Attached => "🔵",
                                crate::models::SessionStatus::Detached => "🟡",
                                crate::models::SessionStatus::Stopped => "🔴",
                                crate::models::SessionStatus::Error(_) => "❌",
                            };
                            status_parts.push(format!(
                                "{} {} ({})",
                                status_icon, session.name, tmux_name
                            ));
                        }
                    }
                }
            }
        }

        // Claude chat status
        let chat_status = if state.claude_chat_visible {
            "🗨️ ON"
        } else {
            "🗨️ OFF"
        };
        status_parts.push(chat_status.to_string());

        let status_text = if status_parts.is_empty() {
            "Claude-in-a-Box - No active session".to_string()
        } else {
            status_parts.join(" | ")
        };

        let status = Paragraph::new(status_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue))
                    .title("Status"),
            )
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Left);

        frame.render_widget(status, area);
    }

    fn render_notifications(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        let notifications = state.get_current_notifications();
        if notifications.is_empty() {
            return;
        }

        // Position notifications in the top-right corner
        let notification_width = 50;
        let notification_height = notifications.len() as u16 * 3; // 3 lines per notification

        let notification_area = Rect {
            x: area.width.saturating_sub(notification_width + 2),
            y: 1,
            width: notification_width,
            height: notification_height.min(area.height.saturating_sub(2)),
        };

        // Render each notification
        for (i, notification) in notifications.iter().enumerate() {
            let y_offset = i as u16 * 3;
            if y_offset >= notification_area.height {
                break; // Don't render notifications that won't fit
            }

            let single_notification_area = Rect {
                x: notification_area.x,
                y: notification_area.y + y_offset,
                width: notification_area.width,
                height: 3.min(notification_area.height - y_offset),
            };

            let (style, border_color) = match notification.notification_type {
                crate::app::state::NotificationType::Success => {
                    (Style::default().fg(Color::Green), Color::Green)
                }
                crate::app::state::NotificationType::Error => {
                    (Style::default().fg(Color::Red), Color::Red)
                }
                crate::app::state::NotificationType::Warning => {
                    (Style::default().fg(Color::Yellow), Color::Yellow)
                }
                crate::app::state::NotificationType::Info => {
                    (Style::default().fg(Color::Cyan), Color::Cyan)
                }
            };

            let notification_widget = Paragraph::new(notification.message.as_str())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color)),
                )
                .style(style)
                .wrap(ratatui::widgets::Wrap { trim: true });

            frame.render_widget(notification_widget, single_notification_area);
        }
    }

    fn render_quick_commit_dialog(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        // Create a centered dialog area
        let dialog_area = centered_rect(60, 20, area);

        // Clear the background
        let clear = Block::default().style(Style::default().bg(Color::Black));
        frame.render_widget(clear, dialog_area);

        // Create the dialog layout
        let dialog_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(3), // Input field
                Constraint::Length(2), // Instructions
            ])
            .split(dialog_area);

        // Render title
        let title = Paragraph::new("Quick Commit")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title("Git Commit"),
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(title, dialog_layout[0]);

        // Render input field
        let empty_string = String::new();
        let commit_message = state.quick_commit_message.as_ref().unwrap_or(&empty_string);

        // Create the input text with cursor
        let mut display_text = commit_message.clone();
        if state.quick_commit_cursor <= display_text.len() {
            display_text.insert(state.quick_commit_cursor, '|');
        }

        let input_paragraph = Paragraph::new(display_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
                    .title("Commit Message"),
            )
            .style(Style::default().fg(Color::White));
        frame.render_widget(input_paragraph, dialog_layout[1]);

        // Render instructions
        let instructions = Paragraph::new("Enter: Commit & Push | Esc: Cancel")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, dialog_layout[2]);
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
