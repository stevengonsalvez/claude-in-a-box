// ABOUTME: Tmux preview pane component for displaying live session output
//
// Provides a split-pane TUI component showing:
// - Normal mode: Last N lines with auto-scroll
// - Scroll mode: Full history with manual navigation
// - Status footer with mode indicators and keyboard shortcuts
// - Colored output using ANSI escape sequence parsing

#![allow(dead_code)]

use ansi_to_tui::IntoText;
use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::app::AppState;
use crate::models::Session;

/// Preview mode for the tmux pane
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    /// Normal mode: Shows last N lines, auto-scrolling
    Normal,
    /// Scroll mode: Shows full history with manual scrolling
    Scroll,
}

/// Component for displaying tmux session preview
#[derive(Debug)]
pub struct TmuxPreviewPane {
    /// Current preview mode
    preview_mode: PreviewMode,
    /// Vertical scroll offset in scroll mode
    scroll_offset: usize,
    /// Maximum scroll offset (updated when rendering)
    max_scroll: usize,
}

impl TmuxPreviewPane {
    /// Create a new tmux preview pane
    pub fn new() -> Self {
        Self {
            preview_mode: PreviewMode::Normal,
            scroll_offset: 0,
            max_scroll: 0,
        }
    }

    /// Render the preview pane
    ///
    /// # Arguments
    /// * `frame` - The ratatui Frame to render to
    /// * `area` - The area to render the component in
    /// * `state` - The application state
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        if let Some(session) = state.selected_session() {
            if session.is_attached {
                self.render_attached_notice(frame, area);
            } else {
                self.render_preview(frame, area, session);
            }
        } else {
            self.render_empty_state(frame, area);
        }
    }

    /// Render the preview content for a session
    fn render_preview(&mut self, frame: &mut Frame, area: Rect, session: &Session) {
        let title = match self.preview_mode {
            PreviewMode::Normal => format!("Session Preview: {}", session.name),
            PreviewMode::Scroll => format!("Session Preview: {} [SCROLL MODE]", session.name),
        };

        let border_color = match self.preview_mode {
            PreviewMode::Normal => Color::Cyan,
            PreviewMode::Scroll => Color::Yellow,
        };

        // Split area for content and footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),   // Content
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // Render content based on mode
        match &session.preview_content {
            Some(content) => {
                let content_area = chunks[0];
                self.render_content(frame, content_area, content, border_color, &title);
            }
            None => {
                self.render_placeholder(frame, chunks[0], &title);
            }
        }

        // Render footer
        self.render_footer(frame, chunks[1]);
    }

    /// Render the actual content
    fn render_content(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        content: &str,
        border_color: Color,
        title: &str,
    ) {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders

        // Calculate max scroll offset
        self.max_scroll = total_lines.saturating_sub(visible_height);

        let display_text = match self.preview_mode {
            PreviewMode::Normal => {
                // Show last N lines (auto-scroll to bottom)
                let start = total_lines.saturating_sub(visible_height);
                lines
                    .iter()
                    .skip(start)
                    .take(visible_height)
                    .map(|s| *s)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            PreviewMode::Scroll => {
                // Show from scroll offset
                let start = self.scroll_offset.min(self.max_scroll);
                lines
                    .iter()
                    .skip(start)
                    .take(visible_height)
                    .map(|s| *s)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        };

        // Convert ANSI escape sequences to ratatui styled text for colored output
        let styled_text = display_text
            .into_text()
            .unwrap_or_else(|_| Text::raw(&display_text));

        let paragraph = Paragraph::new(styled_text)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color)),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);

        // Render scrollbar in scroll mode
        if self.preview_mode == PreviewMode::Scroll && total_lines > visible_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::new(self.max_scroll)
                .position(self.scroll_offset.min(self.max_scroll));

            frame.render_stateful_widget(
                scrollbar,
                area.inner(&Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }
    }

    /// Render placeholder when no content available
    fn render_placeholder(&self, frame: &mut Frame, area: Rect, title: &str) {
        let paragraph = Paragraph::new("Starting session...\n\nWaiting for output...")
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    /// Render empty state when no session selected
    fn render_empty_state(&self, frame: &mut Frame, area: Rect) {
        let paragraph = Paragraph::new("Select a session to view its live preview")
            .block(
                Block::default()
                    .title("Session Preview")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    /// Render notice when user is attached to session
    fn render_attached_notice(&self, frame: &mut Frame, area: Rect) {
        let paragraph = Paragraph::new(
            "You are currently attached to this session.\n\n\
            Press Ctrl+Q to detach and return to this view.",
        )
        .block(
            Block::default()
                .title("Session Preview [ATTACHED]")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    /// Render the footer with keyboard hints
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer_text = match self.preview_mode {
            PreviewMode::Normal => {
                "Press 'a' to attach | Shift+↑/↓ to enter scroll mode | 'k' to kill session"
            }
            PreviewMode::Scroll => {
                "↑↓ to scroll | PgUp/PgDn for fast scroll | ESC to exit scroll mode"
            }
        };

        let footer_style = match self.preview_mode {
            PreviewMode::Normal => Style::default().fg(Color::DarkGray),
            PreviewMode::Scroll => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        };

        let paragraph = Paragraph::new(footer_text)
            .style(footer_style)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Enter scroll mode
    pub fn enter_scroll_mode(&mut self) {
        self.preview_mode = PreviewMode::Scroll;
        // Start at the bottom
        self.scroll_offset = self.max_scroll;
    }

    /// Exit scroll mode
    pub fn exit_scroll_mode(&mut self) {
        self.preview_mode = PreviewMode::Normal;
        self.scroll_offset = 0;
    }

    /// Scroll up by one line
    pub fn scroll_up(&mut self) {
        if self.preview_mode == PreviewMode::Scroll {
            self.scroll_offset = self.scroll_offset.saturating_sub(1);
        }
    }

    /// Scroll down by one line
    pub fn scroll_down(&mut self) {
        if self.preview_mode == PreviewMode::Scroll {
            self.scroll_offset = (self.scroll_offset + 1).min(self.max_scroll);
        }
    }

    /// Scroll up by a page
    pub fn scroll_page_up(&mut self) {
        if self.preview_mode == PreviewMode::Scroll {
            self.scroll_offset = self.scroll_offset.saturating_sub(10);
        }
    }

    /// Scroll down by a page
    pub fn scroll_page_down(&mut self) {
        if self.preview_mode == PreviewMode::Scroll {
            self.scroll_offset = (self.scroll_offset + 10).min(self.max_scroll);
        }
    }

    /// Get the current preview mode
    pub fn mode(&self) -> PreviewMode {
        self.preview_mode
    }

    /// Check if the preview is currently in scroll mode
    pub fn is_scroll_mode(&self) -> bool {
        self.preview_mode == PreviewMode::Scroll
    }
}

impl Default for TmuxPreviewPane {
    fn default() -> Self {
        Self::new()
    }
}
