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
                NewSessionStep::SelectMode => self.render_mode_selection(frame, popup_area, session_state),
                NewSessionStep::InputPrompt => self.render_prompt_input(frame, popup_area, session_state),
                NewSessionStep::ConfigurePermissions => self.render_permissions_config(frame, popup_area, session_state),
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

    fn render_permissions_config(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Length(5),  // Description
                Constraint::Length(5),  // Option display
                Constraint::Length(3),  // Instructions
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Configure Claude Permissions")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title("New Session")
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Description
        let description = Paragraph::new(
            "Claude can run with or without permission prompts.\n\
             With prompts: Claude will ask before running commands\n\
             Without prompts: Claude runs commands immediately (faster)"
        )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
            )
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(description, chunks[1]);

        // Options
        let option_text = if session_state.skip_permissions {
            "🚀 Skip permission prompts (--dangerously-skip-permissions)\n\n\
             Claude will execute commands without asking"
        } else {
            "🛡️  Keep permission prompts (default)\n\n\
             Claude will ask before executing commands"
        };
        
        let options = Paragraph::new(option_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
                    .title("Current Selection")
            )
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center);
        frame.render_widget(options, chunks[2]);

        // Instructions
        let instructions = Paragraph::new("Space: Toggle • Enter: Continue • Esc: Cancel")
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

    fn render_mode_selection(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        use crate::models::SessionMode;
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Min(0),     // Mode selection
                Constraint::Length(3),  // Instructions
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Select Session Mode")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title("Step 3: Choose Mode")
            )
            .style(Style::default().fg(Color::Yellow).bg(Color::Black))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Mode selection
        let mode_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(chunks[1]);

        // Interactive mode option
        let interactive_style = if session_state.mode == SessionMode::Interactive {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else {
            Style::default().fg(Color::White)
        };
        
        let interactive_text = vec![
            Line::from(vec![Span::styled("● Interactive Mode", interactive_style.add_modifier(Modifier::BOLD))]),
            Line::from(vec![Span::styled("  Traditional development with shell access", interactive_style)]),
            Line::from(vec![Span::styled("  Full Claude CLI features and MCP servers", interactive_style)]),
            Line::from(vec![Span::styled("  Attach to container for development", interactive_style)]),
        ];
        
        let interactive_para = Paragraph::new(interactive_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(if session_state.mode == SessionMode::Interactive {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Gray)
                    })
            )
            .alignment(Alignment::Left);
        frame.render_widget(interactive_para, mode_chunks[0]);

        // Boss mode option
        let boss_style = if session_state.mode == SessionMode::Boss {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else {
            Style::default().fg(Color::White)
        };
        
        let boss_text = vec![
            Line::from(vec![Span::styled("● Boss Mode", boss_style.add_modifier(Modifier::BOLD))]),
            Line::from(vec![Span::styled("  Non-interactive task execution", boss_style)]),
            Line::from(vec![Span::styled("  Direct prompt execution with JSON output", boss_style)]),
            Line::from(vec![Span::styled("  Results streamed to TUI logs", boss_style)]),
        ];
        
        let boss_para = Paragraph::new(boss_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(if session_state.mode == SessionMode::Boss {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Gray)
                    })
            )
            .alignment(Alignment::Left);
        frame.render_widget(boss_para, mode_chunks[1]);

        // Instructions
        let instructions = Paragraph::new("↑/↓ or j/k: Switch Mode • Enter: Continue • Esc: Cancel")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray))
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, chunks[2]);
    }

    fn render_prompt_input(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Length(5),  // Instructions
                Constraint::Min(0),     // Prompt input area
                Constraint::Length(3),  // Controls
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Enter Boss Mode Prompt")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title("Step 4: Task Prompt")
            )
            .style(Style::default().fg(Color::Yellow).bg(Color::Black))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Instructions - update to mention @ symbol for file finder
        let instructions_text = if session_state.file_finder.is_active {
            vec![
                Line::from("🔍 File Finder Active - Type to filter files:"),
                Line::from("• ↑/↓ or j/k: Navigate files"),
                Line::from("• Enter: Select file • Esc: Cancel file finder"),
                Line::from("• Type characters to filter by filename"),
            ]
        } else {
            vec![
                Line::from("Enter the task or prompt for Claude to execute:"),
                Line::from("• Direct task: \"Analyze this codebase and suggest improvements\""),
                Line::from("• File reference: \"Review the file @src/main.rs\" (type @ for file finder)"),
                Line::from("• GitHub issue: \"Fix issue #123\""),
            ]
        };
        
        let instructions = Paragraph::new(instructions_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(if session_state.file_finder.is_active {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::Blue)
                    })
            )
            .style(if session_state.file_finder.is_active {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Cyan)
            })
            .alignment(Alignment::Left);
        frame.render_widget(instructions, chunks[1]);

        // Split the prompt input area if file finder is active
        if session_state.file_finder.is_active {
            let input_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50),  // Prompt
                    Constraint::Percentage(50),  // File finder
                ])
                .split(chunks[2]);

            // Render prompt on the left
            self.render_text_editor(frame, input_chunks[0], &session_state.boss_prompt, "Prompt");

            // Render file finder on the right
            self.render_file_finder(frame, input_chunks[1], session_state);
        } else {
            // Normal full-width prompt input
            self.render_text_editor(frame, chunks[2], &session_state.boss_prompt, "Prompt");
        }

        // Controls - update based on file finder state
        let controls_text = if session_state.file_finder.is_active {
            "File Finder: ↑/↓ or j/k Navigate • Enter: Select • Esc: Cancel • Type: Filter"
        } else {
            "Type to enter prompt • Ctrl+J: New line • hjkl/arrows: Move cursor • @ for file finder • Enter: Continue • Esc: Cancel"
        };
        
        let controls = Paragraph::new(controls_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray))
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(controls, chunks[3]);
    }

    fn render_file_finder(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Query input
                Constraint::Min(0),     // File list
            ])
            .split(area);

        // Query input
        let query_display = format!("@{}", session_state.file_finder.query);
        let query_input = Paragraph::new(query_display)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title("File Filter")
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Left);
        frame.render_widget(query_input, chunks[0]);

        // File list
        let file_items: Vec<ListItem> = session_state.file_finder.matches
            .iter()
            .enumerate()
            .map(|(idx, file_match)| {
                let style = if idx == session_state.file_finder.selected_index {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };
                
                ListItem::new(file_match.relative_path.as_str()).style(style)
            })
            .collect();

        let file_list = List::new(file_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(format!("Files ({} matches)", session_state.file_finder.matches.len()))
            );

        frame.render_widget(file_list, chunks[1]);
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

    fn render_text_editor(&self, frame: &mut Frame, area: Rect, editor: &crate::app::state::TextEditor, title: &str) {
        use ratatui::widgets::{Block, Borders, Paragraph};
        use ratatui::style::{Color, Style};
        use ratatui::text::{Line, Span};
        use ratatui::layout::Alignment;

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title(title);
        
        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        if editor.is_empty() {
            // Show placeholder text
            let placeholder = Paragraph::new("Type your prompt here...")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Left);
            frame.render_widget(placeholder, inner_area);
        } else {
            // Render text with cursor
            let (cursor_line, cursor_col) = editor.get_cursor_position();
            let lines = editor.get_lines();
            
            let rendered_lines: Vec<Line> = lines.iter().enumerate().map(|(line_idx, line_text)| {
                if line_idx == cursor_line {
                    // This line contains the cursor
                    let mut spans = Vec::new();
                    
                    if cursor_col == 0 {
                        // Cursor at beginning of line
                        spans.push(Span::styled("█", Style::default().fg(Color::White).bg(Color::Green)));
                        if !line_text.is_empty() {
                            spans.push(Span::styled(line_text, Style::default().fg(Color::White)));
                        }
                    } else if cursor_col >= line_text.len() {
                        // Cursor at end of line
                        spans.push(Span::styled(line_text, Style::default().fg(Color::White)));
                        spans.push(Span::styled("█", Style::default().fg(Color::White).bg(Color::Green)));
                    } else {
                        // Cursor in middle of line
                        let (before, rest) = line_text.split_at(cursor_col);
                        let (cursor_char, after) = if rest.len() > 1 {
                            rest.split_at(1)
                        } else {
                            (rest, "")
                        };
                        
                        if !before.is_empty() {
                            spans.push(Span::styled(before, Style::default().fg(Color::White)));
                        }
                        spans.push(Span::styled(cursor_char, Style::default().fg(Color::White).bg(Color::Green)));
                        if !after.is_empty() {
                            spans.push(Span::styled(after, Style::default().fg(Color::White)));
                        }
                    }
                    
                    Line::from(spans)
                } else {
                    // Normal line without cursor
                    Line::from(Span::styled(line_text, Style::default().fg(Color::White)))
                }
            }).collect();
            
            let paragraph = Paragraph::new(rendered_lines)
                .alignment(Alignment::Left)
                .wrap(ratatui::widgets::Wrap { trim: false }); // Don't trim to preserve exact formatting
            
            frame.render_widget(paragraph, inner_area);
        }
    }
}

impl Default for NewSessionComponent {
    fn default() -> Self {
        Self::new()
    }
}