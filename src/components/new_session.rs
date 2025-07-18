// ABOUTME: New session creation UI component with repository selection and branch input

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Clear},
    style::{Color, Style, Modifier},
};

use crate::app::{AppState, state::{NewSessionStep, NewSessionState}};

pub struct NewSessionComponent {
    search_list_state: ListState,
}

impl NewSessionComponent {
    pub fn new() -> Self {
        Self {
            search_list_state: ListState::default(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        if let Some(ref session_state) = state.new_session_state {
            // Create a centered popup
            let popup_area = self.centered_rect(80, 70, area);
            
            // Clear the background
            frame.render_widget(Clear, popup_area);
            
            match session_state.step {
                NewSessionStep::SelectRepo => {
                    if state.current_view == crate::app::state::View::SearchWorkspace {
                        self.render_search_workspace(frame, popup_area, session_state)
                    } else {
                        self.render_repo_selection(frame, popup_area, session_state)
                    }
                },
                NewSessionStep::InputBranch => self.render_branch_input(frame, popup_area, session_state),
                NewSessionStep::Creating => self.render_creating(frame, popup_area),
            }
        }
    }

    fn render_repo_selection(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Min(0),     // Repository list
                Constraint::Length(3),  // Instructions
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Select Repository")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title("New Session")
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Repository list (showing only folder names)
        let repos: Vec<ListItem> = if session_state.filtered_repos.is_empty() {
            vec![
                ListItem::new("No repositories found in default paths")
                    .style(Style::default().fg(Color::Gray)),
                ListItem::new("Try searching in common directories like:")
                    .style(Style::default().fg(Color::Gray)),
                ListItem::new("  ~/projects, ~/code, ~/dev, ~/src")
                    .style(Style::default().fg(Color::Gray)),
                ListItem::new("Type to filter or add custom paths")
                    .style(Style::default().fg(Color::Yellow)),
            ]
        } else {
            session_state.filtered_repos
                .iter()
                .enumerate()
                .map(|(display_idx, (_, repo))| {
                    let repo_name = repo.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    
                    let style = if Some(display_idx) == session_state.selected_repo_index {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    
                    ListItem::new(repo_name).style(style)
                })
                .collect()
        };

        let repo_list = List::new(repos)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
            )
            .highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_widget(repo_list, chunks[1]);

        // Instructions
        let instructions = Paragraph::new("↑/↓ or j/k: Navigate • Enter: Select • Esc: Cancel")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray))
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, chunks[2]);
    }

    fn render_search_workspace(&mut self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Length(3),  // Search input
                Constraint::Min(0),     // Repository list
                Constraint::Length(3),  // Instructions
            ])
            .split(area);

        // Title - Use solid background to prevent text bleeding
        let title = Paragraph::new("Search Workspaces")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title("Search Repositories")
            )
            .style(Style::default().fg(Color::Yellow).bg(Color::Black))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Search input - Use a solid background to prevent text bleeding
        let search_input = Paragraph::new(session_state.filter_text.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
                    .title("Filter")
                    .style(Style::default().bg(Color::Black))
            )
            .style(Style::default().fg(Color::White).bg(Color::Black));
        frame.render_widget(search_input, chunks[1]);

        // Repository list (showing only folder names)
        let repos: Vec<ListItem> = session_state.filtered_repos
            .iter()
            .enumerate()
            .map(|(display_idx, (_, repo))| {
                let repo_name = repo.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                
                let style = if Some(display_idx) == session_state.selected_repo_index {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                
                ListItem::new(repo_name).style(style)
            })
            .collect();

        let repo_list = List::new(repos)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
                    .title(format!("Repositories ({}/{})", 
                        session_state.filtered_repos.len(), 
                        session_state.available_repos.len()))
                    .style(Style::default().bg(Color::Black))
            )
            .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::Yellow));

        // Update the list state to match the current selection
        self.search_list_state.select(session_state.selected_repo_index);

        frame.render_stateful_widget(repo_list, chunks[2], &mut self.search_list_state);

        // Instructions - Use solid background to prevent text bleeding
        let instructions = Paragraph::new("Type to filter • ↑/↓ or j/k: Navigate • Enter: Select • Esc: Cancel")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray))
            )
            .style(Style::default().fg(Color::Gray).bg(Color::Black))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, chunks[3]);
    }

    fn render_branch_input(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Length(5),  // Repository info
                Constraint::Length(3),  // Branch input
                Constraint::Length(3),  // Instructions
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Enter Branch Name")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title("New Session")
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Repository info
        let repo_info = if let Some(selected_idx) = session_state.selected_repo_index {
            if let Some((_, repo)) = session_state.filtered_repos.get(selected_idx) {
                let repo_name = repo.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                format!("Repository: {}\nPath: {}", repo_name, repo.to_string_lossy())
            } else {
                "Repository: Unknown".to_string()
            }
        } else {
            "Repository: None selected".to_string()
        };

        let repo_display = Paragraph::new(repo_info)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
            )
            .style(Style::default().fg(Color::White));
        frame.render_widget(repo_display, chunks[1]);

        // Branch input
        let branch_input = Paragraph::new(session_state.branch_name.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
                    .title("Branch Name")
            )
            .style(Style::default().fg(Color::White));
        frame.render_widget(branch_input, chunks[2]);

        // Instructions
        let instructions = Paragraph::new("Type branch name • Enter: Create Session • Esc: Cancel")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray))
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, chunks[3]);
    }

    fn render_creating(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Min(0),     // Progress
                Constraint::Length(3),  // Instructions
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Creating Session...")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title("New Session")
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Progress
        let progress = Paragraph::new("Creating Git worktree and Docker container...\nThis may take a moment.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
            )
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center);
        frame.render_widget(progress, chunks[1]);

        // Instructions
        let instructions = Paragraph::new("Please wait... • Esc: Cancel")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray))
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, chunks[2]);
    }

    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
}

impl Default for NewSessionComponent {
    fn default() -> Self {
        Self::new()
    }
}