// ABOUTME: New session creation UI component with repository selection and branch input

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::app::{
    AppState,
    state::{NewSessionState, NewSessionStep},
};

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
                }
                NewSessionStep::InputBranch => {
                    self.render_branch_input(frame, popup_area, session_state)
                }
                NewSessionStep::SelectMode => {
                    self.render_mode_selection(frame, popup_area, session_state)
                }
                NewSessionStep::InputPrompt => {
                    self.render_prompt_input(frame, popup_area, session_state)
                }
                NewSessionStep::ConfigurePermissions => {
                    self.render_permissions_config(frame, popup_area, session_state)
                }
                NewSessionStep::Creating => self.render_creating(frame, popup_area),
            }
        }
    }

    fn render_repo_selection(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
    ) {
        // Draw border around entire dialog
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title("New Session");
        frame.render_widget(block, area);

        // Inner area for content
        let inner = Block::default().borders(Borders::ALL).inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Repository list
                Constraint::Length(3), // Instructions
            ])
            .split(inner);

        // Title
        let title = Paragraph::new("Select Repository")
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
            session_state
                .filtered_repos
                .iter()
                .enumerate()
                .map(|(display_idx, (_, repo))| {
                    let repo_name = repo.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");

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
                    .border_style(Style::default().fg(Color::White)),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_widget(repo_list, chunks[1]);

        // Instructions
        let instructions = Paragraph::new("↑/↓: Navigate • Enter: Select • Esc: Cancel")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, chunks[2]);
    }

    fn render_search_workspace(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
    ) {
        // Draw outer border with gradient-like effect using rounded corners
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(100, 149, 237))) // Cornflower blue
            .title(Span::styled(
                " 🔍 Search Repositories ",
                Style::default()
                    .fg(Color::Rgb(255, 215, 0)) // Gold
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(25, 25, 35))); // Dark background
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Search input
                Constraint::Length(1), // Spacer
                Constraint::Min(0),    // Repository list
                Constraint::Length(1), // Spacer
                Constraint::Length(2), // Instructions
            ])
            .split(inner);

        // Search input with icon and styled placeholder
        let search_text = if session_state.filter_text.is_empty() {
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "Type to search repositories...",
                    Style::default().fg(Color::Rgb(128, 128, 128)).add_modifier(Modifier::ITALIC),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("  ", Style::default().fg(Color::Rgb(100, 200, 100))),
                Span::styled(
                    &session_state.filter_text,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled("█", Style::default().fg(Color::Rgb(100, 200, 100))), // Cursor
            ])
        };

        let search_input = Paragraph::new(search_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(100, 200, 100))) // Green border
                    .style(Style::default().bg(Color::Rgb(35, 35, 45))),
            );
        frame.render_widget(search_input, chunks[0]);

        // Repository list with enhanced styling
        let total_repos = session_state.available_repos.len();
        let filtered_count = session_state.filtered_repos.len();

        let repos: Vec<ListItem> = session_state
            .filtered_repos
            .iter()
            .enumerate()
            .map(|(display_idx, (_, repo))| {
                let repo_name = repo.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
                let parent_path = repo.parent()
                    .and_then(|p| p.to_str())
                    .map(|s| {
                        // Truncate long paths
                        if s.len() > 50 {
                            format!("...{}", &s[s.len()-47..])
                        } else {
                            s.to_string()
                        }
                    })
                    .unwrap_or_default();

                let is_selected = Some(display_idx) == session_state.selected_repo_index;

                if is_selected {
                    // Selected item - highlighted with arrow and full styling
                    let lines = vec![
                        Line::from(vec![
                            Span::styled("  ▶ ", Style::default().fg(Color::Rgb(255, 215, 0))),
                            Span::styled("📁 ", Style::default()),
                            Span::styled(
                                repo_name,
                                Style::default()
                                    .fg(Color::Rgb(255, 215, 0))
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("      ", Style::default()),
                            Span::styled(
                                parent_path,
                                Style::default().fg(Color::Rgb(150, 150, 150)).add_modifier(Modifier::ITALIC),
                            ),
                        ]),
                    ];
                    ListItem::new(lines).style(Style::default().bg(Color::Rgb(45, 45, 60)))
                } else {
                    // Non-selected item
                    let lines = vec![
                        Line::from(vec![
                            Span::styled("    ", Style::default()),
                            Span::styled("📂 ", Style::default()),
                            Span::styled(
                                repo_name,
                                Style::default().fg(Color::Rgb(200, 200, 200)),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("      ", Style::default()),
                            Span::styled(
                                parent_path,
                                Style::default().fg(Color::Rgb(100, 100, 100)),
                            ),
                        ]),
                    ];
                    ListItem::new(lines)
                }
            })
            .collect();

        // Title with count badge
        let count_style = if filtered_count < total_repos {
            Style::default().fg(Color::Rgb(255, 165, 0)) // Orange when filtered
        } else {
            Style::default().fg(Color::Rgb(100, 200, 100)) // Green when showing all
        };

        let title_spans = vec![
            Span::styled(" Repositories ", Style::default().fg(Color::Rgb(200, 200, 200))),
            Span::styled(
                format!("({}/{})", filtered_count, total_repos),
                count_style.add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ];

        let repo_list = List::new(repos)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(70, 70, 90)))
                    .title(Line::from(title_spans))
                    .style(Style::default().bg(Color::Rgb(30, 30, 40))),
            );

        // Update the list state to match the current selection
        self.search_list_state.select(session_state.selected_repo_index);

        frame.render_stateful_widget(repo_list, chunks[2], &mut self.search_list_state);

        // Styled instructions footer
        let instructions = Line::from(vec![
            Span::styled("  ⌨️  ", Style::default()),
            Span::styled("Type", Style::default().fg(Color::Rgb(100, 200, 100))),
            Span::styled(" to filter  ", Style::default().fg(Color::Rgb(128, 128, 128))),
            Span::styled("│", Style::default().fg(Color::Rgb(70, 70, 90))),
            Span::styled("  ↑↓ ", Style::default().fg(Color::Rgb(100, 200, 100))),
            Span::styled("Navigate  ", Style::default().fg(Color::Rgb(128, 128, 128))),
            Span::styled("│", Style::default().fg(Color::Rgb(70, 70, 90))),
            Span::styled("  ⏎ ", Style::default().fg(Color::Rgb(100, 200, 100))),
            Span::styled("Select  ", Style::default().fg(Color::Rgb(128, 128, 128))),
            Span::styled("│", Style::default().fg(Color::Rgb(70, 70, 90))),
            Span::styled("  Esc ", Style::default().fg(Color::Rgb(255, 100, 100))),
            Span::styled("Cancel  ", Style::default().fg(Color::Rgb(128, 128, 128))),
        ]);

        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(25, 25, 35)));
        frame.render_widget(instructions_widget, chunks[4]);
    }

    fn render_branch_input(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        // Draw outer border with modern styling
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(100, 149, 237))) // Cornflower blue
            .title(Span::styled(
                " 🌿 New Session ",
                Style::default()
                    .fg(Color::Rgb(255, 215, 0)) // Gold
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(25, 25, 35))); // Dark background
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(6), // Repository info card
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Branch input
                Constraint::Length(1), // Spacer
                Constraint::Length(2), // Instructions
            ])
            .split(inner);

        // Repository info card with icon
        let (repo_name, repo_path) = if let Some(selected_idx) = session_state.selected_repo_index {
            if let Some((_, repo)) = session_state.filtered_repos.get(selected_idx) {
                let name = repo.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
                let path = repo.to_string_lossy().to_string();
                // Truncate long paths
                let display_path = if path.len() > 60 {
                    format!("...{}", &path[path.len()-57..])
                } else {
                    path
                };
                (name.to_string(), display_path)
            } else {
                ("Unknown".to_string(), "".to_string())
            }
        } else {
            ("None selected".to_string(), "".to_string())
        };

        let repo_lines = vec![
            Line::from(vec![
                Span::styled("  📁 ", Style::default()),
                Span::styled("Repository", Style::default().fg(Color::Rgb(150, 150, 150))),
            ]),
            Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(
                    &repo_name,
                    Style::default()
                        .fg(Color::Rgb(100, 200, 255)) // Light blue
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  📍 ", Style::default()),
                Span::styled("Path", Style::default().fg(Color::Rgb(150, 150, 150))),
            ]),
            Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(
                    repo_path,
                    Style::default().fg(Color::Rgb(180, 180, 180)).add_modifier(Modifier::ITALIC),
                ),
            ]),
        ];

        let repo_display = Paragraph::new(repo_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(70, 70, 90)))
                    .style(Style::default().bg(Color::Rgb(30, 30, 40))),
            );
        frame.render_widget(repo_display, chunks[0]);

        // Branch input with icon and cursor
        let branch_text = if session_state.branch_name.is_empty() {
            Line::from(vec![
                Span::styled("  🔀 ", Style::default().fg(Color::Rgb(100, 200, 100))),
                Span::styled(
                    "agents-in-a-box/",
                    Style::default().fg(Color::Rgb(128, 128, 128)).add_modifier(Modifier::ITALIC),
                ),
                Span::styled("█", Style::default().fg(Color::Rgb(100, 200, 100))),
            ])
        } else {
            Line::from(vec![
                Span::styled("  🔀 ", Style::default().fg(Color::Rgb(100, 200, 100))),
                Span::styled(
                    &session_state.branch_name,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled("█", Style::default().fg(Color::Rgb(100, 200, 100))),
            ])
        };

        let branch_input = Paragraph::new(branch_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(100, 200, 100))) // Green border
                    .title(Span::styled(
                        " Branch Name ",
                        Style::default().fg(Color::Rgb(100, 200, 100)),
                    ))
                    .style(Style::default().bg(Color::Rgb(35, 35, 45))),
            );
        frame.render_widget(branch_input, chunks[2]);

        // Styled instructions footer
        let instructions = Line::from(vec![
            Span::styled("  ⌨️  ", Style::default()),
            Span::styled("Type", Style::default().fg(Color::Rgb(100, 200, 100))),
            Span::styled(" branch name  ", Style::default().fg(Color::Rgb(128, 128, 128))),
            Span::styled("│", Style::default().fg(Color::Rgb(70, 70, 90))),
            Span::styled("  ⏎ ", Style::default().fg(Color::Rgb(100, 200, 100))),
            Span::styled("Create Session  ", Style::default().fg(Color::Rgb(128, 128, 128))),
            Span::styled("│", Style::default().fg(Color::Rgb(70, 70, 90))),
            Span::styled("  Esc ", Style::default().fg(Color::Rgb(255, 100, 100))),
            Span::styled("Cancel  ", Style::default().fg(Color::Rgb(128, 128, 128))),
        ]);

        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(25, 25, 35)));
        frame.render_widget(instructions_widget, chunks[4]);
    }

    fn render_permissions_config(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
    ) {
        // Draw border around entire dialog
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title("New Session");
        frame.render_widget(block, area);

        // Inner area for content
        let inner = Block::default().borders(Borders::ALL).inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(5), // Description
                Constraint::Length(5), // Option display
                Constraint::Length(3), // Instructions
            ])
            .split(inner);

        // Title
        let title = Paragraph::new("Configure Claude Permissions")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Description
        let description = Paragraph::new(
            "Claude can run with or without permission prompts.\n\
             With prompts: Claude will ask before running commands\n\
             Without prompts: Claude runs commands immediately (faster)",
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
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
                    .title("Current Selection"),
            )
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center);
        frame.render_widget(options, chunks[2]);

        // Instructions
        let instructions = Paragraph::new("Space: Toggle • Enter: Continue • Esc: Cancel")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, chunks[3]);
    }

    fn render_creating(&self, frame: &mut Frame, area: Rect) {
        // Draw border around entire dialog
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title("New Session");
        frame.render_widget(block, area);

        // Inner area for content
        let inner = Block::default().borders(Borders::ALL).inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Progress
                Constraint::Length(3), // Instructions
            ])
            .split(inner);

        // Title
        let title = Paragraph::new("Creating Session...")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Progress
        let progress = Paragraph::new(
            "Creating Git worktree and Docker container...\nThis may take a moment.",
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
        frame.render_widget(progress, chunks[1]);

        // Instructions
        let instructions = Paragraph::new("Please wait... • Esc: Cancel")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, chunks[2]);
    }

    fn render_mode_selection(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
    ) {
        use crate::models::SessionMode;

        // Draw outer border with modern styling
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(100, 149, 237))) // Cornflower blue
            .title(Span::styled(
                " 🎯 Choose Session Mode ",
                Style::default()
                    .fg(Color::Rgb(255, 215, 0)) // Gold
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(25, 25, 35))); // Dark background
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(8), // Interactive mode card
                Constraint::Length(1), // Spacer
                Constraint::Length(8), // Boss mode card
                Constraint::Length(1), // Spacer
                Constraint::Length(2), // Instructions
            ])
            .split(inner);

        let is_interactive = session_state.mode == SessionMode::Interactive;
        let is_boss = session_state.mode == SessionMode::Boss;

        // Interactive mode card
        let interactive_border_color = if is_interactive {
            Color::Rgb(100, 200, 100) // Green when selected
        } else {
            Color::Rgb(70, 70, 90) // Gray when not
        };

        let interactive_bg = if is_interactive {
            Color::Rgb(35, 45, 35) // Slightly green tint
        } else {
            Color::Rgb(30, 30, 40)
        };

        let interactive_text = vec![
            Line::from(vec![
                Span::styled(
                    if is_interactive { "  ▶ " } else { "    " },
                    Style::default().fg(Color::Rgb(100, 200, 100)),
                ),
                Span::styled("🖥️  ", Style::default()),
                Span::styled(
                    "Interactive Mode",
                    Style::default()
                        .fg(if is_interactive { Color::Rgb(100, 200, 100) } else { Color::Rgb(200, 200, 200) })
                        .add_modifier(Modifier::BOLD),
                ),
                if is_interactive {
                    Span::styled("  ✓", Style::default().fg(Color::Rgb(100, 200, 100)))
                } else {
                    Span::raw("")
                },
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(100, 149, 237))),
                Span::styled(" Traditional development with shell access", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(100, 149, 237))),
                Span::styled(" Full Claude CLI features and MCP servers", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(100, 149, 237))),
                Span::styled(" Attach to container for development", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
        ];

        let interactive_para = Paragraph::new(interactive_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(interactive_border_color))
                    .style(Style::default().bg(interactive_bg)),
            );
        frame.render_widget(interactive_para, chunks[0]);

        // Boss mode card
        let boss_border_color = if is_boss {
            Color::Rgb(255, 165, 0) // Orange when selected
        } else {
            Color::Rgb(70, 70, 90) // Gray when not
        };

        let boss_bg = if is_boss {
            Color::Rgb(45, 40, 30) // Slightly orange tint
        } else {
            Color::Rgb(30, 30, 40)
        };

        let boss_text = vec![
            Line::from(vec![
                Span::styled(
                    if is_boss { "  ▶ " } else { "    " },
                    Style::default().fg(Color::Rgb(255, 165, 0)),
                ),
                Span::styled("🤖 ", Style::default()),
                Span::styled(
                    "Boss Mode",
                    Style::default()
                        .fg(if is_boss { Color::Rgb(255, 165, 0) } else { Color::Rgb(200, 200, 200) })
                        .add_modifier(Modifier::BOLD),
                ),
                if is_boss {
                    Span::styled("  ✓", Style::default().fg(Color::Rgb(255, 165, 0)))
                } else {
                    Span::raw("")
                },
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(255, 165, 0))),
                Span::styled(" Non-interactive task execution", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(255, 165, 0))),
                Span::styled(" Direct prompt execution with text output", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(255, 165, 0))),
                Span::styled(" Results streamed to TUI logs", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
        ];

        let boss_para = Paragraph::new(boss_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(boss_border_color))
                    .style(Style::default().bg(boss_bg)),
            );
        frame.render_widget(boss_para, chunks[2]);

        // Styled instructions footer
        let instructions = Line::from(vec![
            Span::styled("  ↑↓ ", Style::default().fg(Color::Rgb(100, 200, 100))),
            Span::styled("Switch Mode  ", Style::default().fg(Color::Rgb(128, 128, 128))),
            Span::styled("│", Style::default().fg(Color::Rgb(70, 70, 90))),
            Span::styled("  ⏎ ", Style::default().fg(Color::Rgb(100, 200, 100))),
            Span::styled("Continue  ", Style::default().fg(Color::Rgb(128, 128, 128))),
            Span::styled("│", Style::default().fg(Color::Rgb(70, 70, 90))),
            Span::styled("  Esc ", Style::default().fg(Color::Rgb(255, 100, 100))),
            Span::styled("Cancel  ", Style::default().fg(Color::Rgb(128, 128, 128))),
        ]);

        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(25, 25, 35)));
        frame.render_widget(instructions_widget, chunks[4]);
    }

    fn render_prompt_input(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        // Draw border around entire dialog
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title("New Session - Step 4: Task Prompt");
        frame.render_widget(block, area);

        // Inner area for content
        let inner = Block::default().borders(Borders::ALL).inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(5), // Instructions
                Constraint::Min(0),    // Prompt input area
                Constraint::Length(3), // Controls
            ])
            .split(inner);

        // Title
        let title = Paragraph::new("Enter Boss Mode Prompt")
            .style(Style::default().fg(Color::Yellow).bg(Color::Black))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Instructions - update to mention @ symbol for file finder
        let instructions_text = if session_state.file_finder.is_active {
            vec![
                Line::from("🔍 File Finder Active - Type to filter files:"),
                Line::from("• ↑/↓: Navigate files"),
                Line::from("• Enter: Select file • Esc: Cancel file finder"),
                Line::from("• Type characters to filter by filename"),
            ]
        } else {
            vec![
                Line::from("Enter the task or prompt for Claude to execute:"),
                Line::from("• Direct task: \"Analyze this codebase and suggest improvements\""),
                Line::from(
                    "• File reference: \"Review the file @src/main.rs\" (type @ for file finder)",
                ),
                Line::from("• GitHub issue: \"Fix issue #123\""),
            ]
        };

        let instructions = Paragraph::new(instructions_text)
            .block(Block::default().borders(Borders::ALL).border_style(
                if session_state.file_finder.is_active {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Blue)
                },
            ))
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
                    Constraint::Percentage(50), // Prompt
                    Constraint::Percentage(50), // File finder
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
            "File Finder: ↑/↓ Navigate • Enter: Select • Esc: Cancel • Type: Filter"
        } else {
            "Type to enter prompt • Ctrl+J: New line • arrows: Move cursor • @ for file finder • Enter: Continue • Esc: Cancel"
        };

        let controls = Paragraph::new(controls_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(controls, chunks[3]);
    }

    fn render_file_finder(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Query input
                Constraint::Min(0),    // File list
            ])
            .split(area);

        // Query input
        let query_display = format!("@{}", session_state.file_finder.query);
        let query_input = Paragraph::new(query_display)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title("File Filter"),
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Left);
        frame.render_widget(query_input, chunks[0]);

        // File list
        let file_items: Vec<ListItem> = session_state
            .file_finder
            .matches
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

        let file_list = List::new(file_items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(format!(
                    "Files ({} matches)",
                    session_state.file_finder.matches.len()
                )),
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

    fn render_text_editor(
        &self,
        frame: &mut Frame,
        area: Rect,
        editor: &crate::app::state::TextEditor,
        title: &str,
    ) {
        use ratatui::layout::Alignment;
        use ratatui::style::{Color, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders, Paragraph};

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

            let rendered_lines: Vec<Line> = lines
                .iter()
                .enumerate()
                .map(|(line_idx, line_text)| {
                    if line_idx == cursor_line {
                        // This line contains the cursor
                        let mut spans = Vec::new();

                        if cursor_col == 0 {
                            // Cursor at beginning of line
                            spans.push(Span::styled(
                                "█",
                                Style::default().fg(Color::White).bg(Color::Green),
                            ));
                            if !line_text.is_empty() {
                                spans.push(Span::styled(
                                    line_text,
                                    Style::default().fg(Color::White),
                                ));
                            }
                        } else if cursor_col >= line_text.len() {
                            // Cursor at end of line
                            spans.push(Span::styled(line_text, Style::default().fg(Color::White)));
                            spans.push(Span::styled(
                                "█",
                                Style::default().fg(Color::White).bg(Color::Green),
                            ));
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
                            spans.push(Span::styled(
                                cursor_char,
                                Style::default().fg(Color::White).bg(Color::Green),
                            ));
                            if !after.is_empty() {
                                spans.push(Span::styled(after, Style::default().fg(Color::White)));
                            }
                        }

                        Line::from(spans)
                    } else {
                        // Normal line without cursor
                        Line::from(Span::styled(line_text, Style::default().fg(Color::White)))
                    }
                })
                .collect();

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
