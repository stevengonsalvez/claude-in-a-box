// ABOUTME: Session list component for displaying workspaces and sessions in hierarchical view

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::app::AppState;
use crate::models::{SessionMode, SessionStatus, Workspace};

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

            // Determine expand state: expanded if selected OR if expand_all is true
            let is_expanded = is_selected_workspace || state.expand_all_workspaces;

            let workspace_symbol = if session_count == 0 {
                "▷"
            } else if is_expanded {
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

            // Show sessions if workspace is expanded (selected OR expand_all is true)
            if is_expanded {
                let session_len = workspace.sessions.len();
                for (session_idx, session) in workspace.sessions.iter().enumerate() {
                    // Session is selected only if this workspace is selected AND session index matches
                    let is_selected_session = is_selected_workspace && state.selected_session_index == Some(session_idx);
                    let is_last_session = session_idx == session_len - 1;

                    // Use tree line characters
                    let tree_prefix = if is_last_session { "└─" } else { "├─" };

                    let status_indicator = session.status.indicator();

                    // Mode indicator (Boss = Docker container, Interactive = host tmux)
                    let mode_indicator = match session.mode {
                        SessionMode::Boss => "🐳", // Docker container
                        SessionMode::Interactive => "🖥️", // Host/Interactive
                    };

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
                    // Format: tree_prefix status_indicator mode_indicator tmux_indicator branch_name changes
                    items.push(
                        ListItem::new(format!(
                            "  {} {} {} {} {}{}",
                            tree_prefix, status_indicator, mode_indicator, tmux_indicator, session.branch_name, changes_text
                        ))
                        .style(session_style),
                    );
                }
            }
        }

        // Add "Other tmux" section if there are other tmux sessions
        if !state.other_tmux_sessions.is_empty() {
            // Add separator line
            if !items.is_empty() {
                items.push(ListItem::new("").style(Style::default()));
            }

            let session_count = state.other_tmux_sessions.len();
            let is_selected_other = state.selected_workspace_index.is_none()
                && state.selected_other_tmux_index.is_some();

            // Header for other tmux section
            let other_symbol = if state.other_tmux_expanded {
                "▼"
            } else {
                "▶"
            };

            let header_style = if state.selected_workspace_index.is_none() {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Magenta)
            };

            items.push(
                ListItem::new(format!("{} Other tmux ·{}", other_symbol, session_count))
                    .style(header_style),
            );

            // Show other tmux sessions if expanded
            if state.other_tmux_expanded {
                let session_len = state.other_tmux_sessions.len();
                for (idx, other_session) in state.other_tmux_sessions.iter().enumerate() {
                    let is_selected = is_selected_other
                        && state.selected_other_tmux_index == Some(idx);
                    let is_last = idx == session_len - 1;

                    let tree_prefix = if is_last { "└─" } else { "├─" };
                    let status = other_session.status_indicator();

                    let windows_text = if other_session.windows > 1 {
                        format!(" ({}w)", other_session.windows)
                    } else {
                        String::new()
                    };

                    let session_style = if is_selected {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else if other_session.attached {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::Gray)
                    };

                    items.push(
                        ListItem::new(format!(
                            "  {} {} {}{}",
                            tree_prefix, status, other_session.name, windows_text
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
            let mut current_index = 0;

            // When expand_all is true, we need to count items from all workspaces
            for (idx, workspace) in state.workspaces.iter().enumerate() {
                if idx == workspace_idx {
                    // Found the selected workspace
                    current_index += idx; // Add workspace line itself (accounting for skipped sessions)

                    // When expand_all, add all sessions from prior workspaces
                    if state.expand_all_workspaces {
                        for prior_workspace in state.workspaces.iter().take(idx) {
                            current_index += prior_workspace.sessions.len();
                        }
                    }

                    // Add session offset if a session is selected
                    if let Some(session_idx) = state.selected_session_index {
                        current_index += session_idx + 1;
                    }
                    break;
                }
            }

            self.list_state.select(Some(current_index));
        } else if state.selected_other_tmux_index.is_some() {
            // Selection is in "Other tmux" section
            let mut current_index = 0;

            // Count all workspace items first
            for workspace in &state.workspaces {
                current_index += 1; // Workspace header
                if state.expand_all_workspaces {
                    current_index += workspace.sessions.len();
                }
            }

            // Add separator + "Other tmux" header
            if !state.workspaces.is_empty() && !state.other_tmux_sessions.is_empty() {
                current_index += 1; // Empty separator line
            }
            current_index += 1; // "Other tmux" header

            // Add offset for selected other session
            if let Some(other_idx) = state.selected_other_tmux_index {
                current_index += other_idx;
            }

            self.list_state.select(Some(current_index));
        } else {
            self.list_state.select(None);
        }
    }

    /// Calculate total visible items for navigation
    pub fn total_visible_items(state: &AppState) -> usize {
        let mut count = 0;

        // Count workspace items
        for workspace in &state.workspaces {
            count += 1; // Workspace header
            if state.expand_all_workspaces {
                count += workspace.sessions.len();
            }
        }

        // Count "Other tmux" section items
        if !state.other_tmux_sessions.is_empty() {
            if !state.workspaces.is_empty() {
                count += 1; // Empty separator line
            }
            count += 1; // "Other tmux" header
            if state.other_tmux_expanded {
                count += state.other_tmux_sessions.len();
            }
        }

        count
    }
}

#[allow(dead_code)]
fn workspace_running_count(workspace: &Workspace) -> usize {
    workspace.running_sessions().len()
}
