// ABOUTME: Session list component for displaying workspaces and sessions in hierarchical view

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
    style::{Color, Modifier, Style},
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
        
        let list = List::new(items)
            .block(
                Block::default()
                    .title("Workspaces")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            )
            .highlight_symbol("▶ ");
        
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn build_list_items_static(state: &AppState) -> Vec<ListItem> {
        let mut items = Vec::new();
        
        for (workspace_idx, workspace) in state.workspaces.iter().enumerate() {
            let is_selected_workspace = state.selected_workspace_index == Some(workspace_idx);
            let workspace_symbol = if workspace.sessions.is_empty() {
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

            items.push(ListItem::new(format!("{} {}", workspace_symbol, workspace.name))
                .style(workspace_style));

            if is_selected_workspace {
                for (session_idx, session) in workspace.sessions.iter().enumerate() {
                    let is_selected_session = state.selected_session_index == Some(session_idx);
                    let status_indicator = session.status.indicator();
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
                            SessionStatus::Error(_) => Style::default().fg(Color::Red),
                        }
                    };

                    items.push(ListItem::new(format!("  {} {}{}", status_indicator, session.name, changes_text))
                        .style(session_style));
                }
            }
        }

        if items.is_empty() {
            items.push(ListItem::new("No workspaces found")
                .style(Style::default().fg(Color::Gray)));
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