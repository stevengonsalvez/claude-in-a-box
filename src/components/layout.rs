// ABOUTME: Main layout component handling split-pane arrangement and bottom menu bar

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
    style::{Color, Style},
};

use crate::app::{AppState, state::View};
use super::{SessionListComponent, LogsViewerComponent, ClaudeChatComponent, LiveLogsStreamComponent, HelpComponent, NewSessionComponent, ConfirmationDialogComponent, NonGitNotificationComponent, AttachedTerminalComponent, AuthSetupComponent};

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

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),   // Top status bar
                Constraint::Min(0),      // Main content area
                Constraint::Length(5),   // Bottom logs area  
                Constraint::Length(3),   // Bottom menu bar
            ])
            .split(frame.size());

        // Render top status bar
        self.render_status_bar(frame, main_layout[0], state);

        // Simple 2-panel layout: session list | logs (Claude chat is now a popup)
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40),  // Session list
                Constraint::Percentage(60),  // Live logs stream
            ])
            .split(main_layout[1]);

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
    }

    fn render_menu_bar(&self, frame: &mut Frame, area: Rect) {
        let menu_text = "[n]ew [s]earch [a]ttach [c]laude [f]refresh [t]ime [r]e-auth [d]elete [?]help [q]uit";
        
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
        if let Some(session_id) = state.get_selected_session_id() {
            if let Some(workspace_idx) = state.selected_workspace_index {
                if let Some(session_idx) = state.selected_session_index {
                    if let Some(workspace) = state.workspaces.get(workspace_idx) {
                        if let Some(session) = workspace.sessions.get(session_idx) {
                            // Branch info
                            status_parts.push(format!("🌿 {}", session.branch_name));
                            
                            // Container info
                            if let Some(container_id) = &session.container_id {
                                let short_id = &container_id[..8.min(container_id.len())];
                                let status_icon = match session.status {
                                    crate::models::SessionStatus::Running => "🟢",
                                    crate::models::SessionStatus::Stopped => "🔴", 
                                    crate::models::SessionStatus::Error(_) => "❌",
                                };
                                status_parts.push(format!("{} {} ({})", status_icon, session.name, short_id));
                            }
                        }
                    }
                }
            }
        }
        
        // Claude chat status
        let chat_status = if state.claude_chat_visible { "🗨️ ON" } else { "🗨️ OFF" };
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
                    .title("Status")
            )
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Left);

        frame.render_widget(status, area);
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