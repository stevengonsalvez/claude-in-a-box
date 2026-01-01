// ABOUTME: Session list component for displaying workspaces and sessions in hierarchical view

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::app::AppState;
use crate::models::{SessionStatus, Workspace};

pub struct SessionListComponent {
    list_state: ListState,
}

impl Default for SessionListComponent {
    fn default() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self { list_state }
    }
}

impl SessionListComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        // Update list state selection based on app state first
        self.update_selection(state);

        let items = SessionListComponent::build_list_items_static(state);

        // Show focus indicator
        use crate::app::state::FocusedPane;
        let (border_color, title_color) = match state.focused_pane {
            FocusedPane::Sessions => (Color::Cyan, Color::Yellow), // Focused
            FocusedPane::LiveLogs => (Color::Gray, Color::Blue),   // Not focused
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .title("Workspaces")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color))
                    .title_style(Style::default().fg(title_color)),
            )
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn build_list_items_static(state: &AppState) -> Vec<ListItem> {
        let mut items = Vec::new();

        for (workspace_idx, workspace) in state.workspaces.iter().enumerate() {
            let is_selected_workspace = state.selected_workspace_index == Some(workspace_idx);
            let session_count = workspace.sessions.len();
            let workspace_symbol = if session_count == 0 {
                "▷"
            } else if is_selected_workspace {
                "▼"
            } else {
                "▶"
            };

            let workspace_style = if is_selected_workspace {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Add session count badge (only show if sessions exist, use dot separator)
            let count_display = if session_count > 1 {
                format!(" ·{}", session_count)  // Only show count when multiple sessions
            } else {
                String::new()
            };

            items.push(
                ListItem::new(format!("{} {}{}", workspace_symbol, workspace.name, count_display))
                    .style(workspace_style),
            );

            if is_selected_workspace {
                let session_len = workspace.sessions.len();
                for (session_idx, session) in workspace.sessions.iter().enumerate() {
                    let is_selected_session = state.selected_session_index == Some(session_idx);
                    let is_last_session = session_idx == session_len - 1;

                    // Use tree line characters
                    let tree_prefix = if is_last_session { "└─" } else { "├─" };

                    let status_indicator = session.status.indicator();

                    // Tmux status indicator
                    let tmux_indicator = if session.is_attached {
                        "🔗" // Attached to tmux
                    } else if session.tmux_session_name.is_some() {
                        "●"  // Tmux session running
                    } else {
                        "○"  // No tmux session
                    };

                    let changes_text = if session.git_changes.total() > 0 {
                        format!(" ({})", session.git_changes.format())
                    } else {
                        String::new()
                    };

                    let session_style = if is_selected_session {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        match session.status {
                            SessionStatus::Running => Style::default().fg(Color::Green),
                            SessionStatus::Stopped => Style::default().fg(Color::Gray),
                            SessionStatus::Idle => Style::default().fg(Color::Yellow),
                            SessionStatus::Error(_) => Style::default().fg(Color::Red),
                        }
                    };

                    // Show branch name instead of session name (more distinctive)
                    items.push(
                        ListItem::new(format!(
                            "  {} {} {} {}{}",
                            tree_prefix, status_indicator, tmux_indicator, session.branch_name, changes_text
                        ))
                        .style(session_style),
                    );
                }
            }
        }

        if items.is_empty() {
            items
                .push(ListItem::new("No workspaces found").style(Style::default().fg(Color::Gray)));
        }

        items
    }

    fn update_selection(&mut self, state: &AppState) {
        if let Some(workspace_idx) = state.selected_workspace_index {
            let mut current_index = workspace_idx;

            if let Some(session_idx) = state.selected_session_index {
                current_index += session_idx + 1;
            }

            self.list_state.select(Some(current_index));
        } else {
            self.list_state.select(None);
        }
    }
}

#[allow(dead_code)]
fn workspace_running_count(workspace: &Workspace) -> usize {
    workspace.running_sessions().len()
}
