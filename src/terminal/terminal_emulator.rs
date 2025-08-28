// ABOUTME: Terminal emulator widget for rendering PTY output in the TUI
// Processes ANSI escape codes and manages terminal state

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};
use std::collections::VecDeque;
use unicode_width::UnicodeWidthStr;
use vt100;

/// Terminal emulator widget for rendering PTY output
pub struct TerminalEmulatorWidget {
    /// VT100 parser for processing ANSI escape codes
    parser: vt100::Parser,

    /// Scrollback buffer
    scrollback: VecDeque<String>,
    max_scrollback: usize,

    /// Current scroll offset (0 = bottom/latest)
    scroll_offset: usize,

    /// Terminal dimensions
    cols: u16,
    rows: u16,

    /// Cursor position tracking
    cursor_visible: bool,
    cursor_x: u16,
    cursor_y: u16,

    /// Selection for copy/paste
    selection_start: Option<(u16, u16)>,
    selection_end: Option<(u16, u16)>,

    /// Title for the terminal block
    title: String,

    /// Border style based on focus state
    border_style: Style,

    /// Show scrollbar
    show_scrollbar: bool,
}

impl TerminalEmulatorWidget {
    /// Create a new terminal emulator widget
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 0),
            scrollback: VecDeque::new(),
            max_scrollback: 10000,
            scroll_offset: 0,
            cols,
            rows,
            cursor_visible: true,
            cursor_x: 0,
            cursor_y: 0,
            selection_start: None,
            selection_end: None,
            title: String::from("Terminal"),
            border_style: Style::default().fg(Color::Gray),
            show_scrollbar: true,
        }
    }

    /// Process PTY output data
    pub fn process_output(&mut self, data: &str) {
        use tracing::trace;

        trace!(
            "Terminal emulator processing {} bytes of output",
            data.len()
        );

        // Feed data to VT100 parser
        self.parser.process(data.as_bytes());

        // Extract info from screen (before mutable borrow)
        let (cursor_y, cursor_x) = self.parser.screen().cursor_position();
        let cursor_visible = !self.parser.screen().hide_cursor();
        let (rows, cols) = self.parser.screen().size();

        trace!(
            "Terminal cursor at ({}, {}), visible: {}",
            cursor_x, cursor_y, cursor_visible
        );

        // Build lines to add to scrollback
        let mut lines_to_add = Vec::new();
        for row in 0..rows {
            let mut line = String::new();
            for col in 0..cols {
                if let Some(cell) = self.parser.screen().cell(row, col) {
                    line.push_str(&cell.contents());
                }
            }

            // Only add non-empty lines to scrollback
            let trimmed = line.trim_end();
            if !trimmed.is_empty() || row == rows - 1 {
                lines_to_add.push(line);
            }
        }

        trace!("Parsed {} lines from terminal output", lines_to_add.len());

        // Now update self fields
        self.cursor_x = cursor_x;
        self.cursor_y = cursor_y;
        self.cursor_visible = cursor_visible;

        // Add lines to scrollback
        for line in lines_to_add {
            self.add_to_scrollback(line);
        }

        // Reset scroll to bottom on new output
        self.scroll_offset = 0;
        trace!(
            "Terminal scrollback now has {} lines",
            self.scrollback.len()
        );
    }

    /// Add a line to the scrollback buffer
    fn add_to_scrollback(&mut self, line: String) {
        self.scrollback.push_back(line);

        // Trim scrollback if it exceeds max size
        while self.scrollback.len() > self.max_scrollback {
            self.scrollback.pop_front();
        }
    }

    /// Clear the terminal
    pub fn clear(&mut self) {
        self.parser = vt100::Parser::new(self.rows, self.cols, 0);
        self.scrollback.clear();
        self.scroll_offset = 0;
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    /// Resize the terminal
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.parser.set_size(rows, cols);
    }

    /// Scroll up by n lines
    pub fn scroll_up(&mut self, n: usize) {
        let max_scroll = self.scrollback.len().saturating_sub(self.rows as usize);
        self.scroll_offset = (self.scroll_offset + n).min(max_scroll);
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll to top
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = self.scrollback.len().saturating_sub(self.rows as usize);
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Check if at bottom
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset == 0
    }

    /// Set terminal title
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Set border style based on focus
    pub fn set_focused(&mut self, focused: bool) {
        self.border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        };
    }

    /// Start text selection
    pub fn start_selection(&mut self, x: u16, y: u16) {
        self.selection_start = Some((x, y));
        self.selection_end = Some((x, y));
    }

    /// Update selection end point
    pub fn update_selection(&mut self, x: u16, y: u16) {
        if self.selection_start.is_some() {
            self.selection_end = Some((x, y));
        }
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
    }

    /// Get selected text
    pub fn get_selected_text(&self) -> Option<String> {
        if let (Some(_start), Some(_end)) = (self.selection_start, self.selection_end) {
            // TODO: Implement text extraction from selection
            // This requires mapping selection coordinates to buffer content
            None
        } else {
            None
        }
    }

    /// Convert VT100 screen to ratatui Text
    fn screen_to_text(&self) -> Text<'static> {
        let screen = self.parser.screen();
        let mut lines = Vec::new();

        // Calculate visible range based on scroll
        let total_lines = self.scrollback.len();
        let visible_start = total_lines.saturating_sub(self.rows as usize + self.scroll_offset);
        let visible_end = visible_start + self.rows as usize;

        // Get lines from scrollback or current screen
        if self.scroll_offset > 0 {
            // Showing scrollback
            for i in visible_start..visible_end.min(total_lines) {
                if let Some(line) = self.scrollback.get(i) {
                    lines.push(Line::from(line.clone()));
                }
            }
        } else {
            // Showing current screen
            let (rows, cols) = screen.size();
            for row in 0..rows {
                let mut spans = Vec::new();
                let mut current_style = Style::default();
                let mut current_text = String::new();

                for col in 0..cols {
                    if let Some(cell) = screen.cell(row, col) {
                        let cell_style = Self::cell_to_style(&cell);

                        // If style changed, push current span and start new one
                        if cell_style != current_style && !current_text.is_empty() {
                            spans.push(Span::styled(current_text.clone(), current_style));
                            current_text.clear();
                            current_style = cell_style;
                        } else if current_text.is_empty() {
                            current_style = cell_style;
                        }

                        current_text.push_str(&cell.contents());
                    } else {
                        current_text.push(' ');
                    }
                }

                // Push final span
                if !current_text.is_empty() {
                    spans.push(Span::styled(current_text, current_style));
                }

                lines.push(Line::from(spans));
            }
        }

        Text::from(lines)
    }

    /// Convert VT100 cell attributes to ratatui Style
    fn cell_to_style(cell: &vt100::Cell) -> Style {
        let mut style = Style::default();

        // Foreground color
        style = match cell.fgcolor() {
            vt100::Color::Default => style,
            vt100::Color::Idx(n) => style.fg(Self::ansi_to_ratatui_color(n)),
            vt100::Color::Rgb(r, g, b) => style.fg(Color::Rgb(r, g, b)),
        };

        // Background color
        style = match cell.bgcolor() {
            vt100::Color::Default => style,
            vt100::Color::Idx(n) => style.bg(Self::ansi_to_ratatui_color(n)),
            vt100::Color::Rgb(r, g, b) => style.bg(Color::Rgb(r, g, b)),
        };

        // Text attributes
        if cell.bold() {
            style = style.add_modifier(Modifier::BOLD);
        }
        if cell.italic() {
            style = style.add_modifier(Modifier::ITALIC);
        }
        if cell.underline() {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        if cell.inverse() {
            style = style.add_modifier(Modifier::REVERSED);
        }

        style
    }

    /// Convert ANSI color index to ratatui Color
    fn ansi_to_ratatui_color(idx: u8) -> Color {
        match idx {
            0 => Color::Black,
            1 => Color::Red,
            2 => Color::Green,
            3 => Color::Yellow,
            4 => Color::Blue,
            5 => Color::Magenta,
            6 => Color::Cyan,
            7 => Color::Gray,
            8 => Color::DarkGray,
            9 => Color::LightRed,
            10 => Color::LightGreen,
            11 => Color::LightYellow,
            12 => Color::LightBlue,
            13 => Color::LightMagenta,
            14 => Color::LightCyan,
            15 => Color::White,
            _ => Color::White, // Default for extended colors
        }
    }

    /// Render scrollbar
    fn render_scrollbar(&self, area: Rect, buf: &mut Buffer) {
        if !self.show_scrollbar || self.scrollback.len() <= self.rows as usize {
            return;
        }

        let scrollbar_x = area.right().saturating_sub(1);
        let scrollbar_height = area.height.saturating_sub(2); // Account for borders

        // Calculate scrollbar position and size
        let total_lines = self.scrollback.len();
        let visible_lines = self.rows as usize;
        let scrollbar_size =
            ((visible_lines as f32 / total_lines as f32) * scrollbar_height as f32).max(1.0) as u16;
        let scrollbar_pos = if self.scroll_offset == 0 {
            scrollbar_height - scrollbar_size
        } else {
            let max_scroll = total_lines.saturating_sub(visible_lines);
            let pos_ratio = 1.0 - (self.scroll_offset as f32 / max_scroll as f32);
            ((pos_ratio * (scrollbar_height - scrollbar_size) as f32) as u16)
                .min(scrollbar_height - scrollbar_size)
        };

        // Draw scrollbar track
        for y in area.top() + 1..area.bottom() - 1 {
            buf.get_mut(scrollbar_x, y)
                .set_symbol("│")
                .set_style(Style::default().fg(Color::DarkGray));
        }

        // Draw scrollbar thumb
        for i in 0..scrollbar_size {
            let y = area.top() + 1 + scrollbar_pos + i;
            if y < area.bottom() - 1 {
                buf.get_mut(scrollbar_x, y)
                    .set_symbol("█")
                    .set_style(Style::default().fg(Color::Gray));
            }
        }
    }
}

impl Widget for TerminalEmulatorWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create block with title and borders
        let block = Block::default()
            .title(self.title.clone())
            .borders(Borders::ALL)
            .border_style(self.border_style);

        // Render the block
        let inner = block.inner(area);
        block.render(area, buf);

        // Get terminal content as Text
        let text = self.screen_to_text();

        // Create paragraph with the terminal content
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

        // Render the paragraph
        paragraph.render(inner, buf);

        // Render cursor if visible and at bottom
        if self.cursor_visible && self.is_at_bottom() {
            let cursor_x = inner.left() + self.cursor_x.min(inner.width - 1);
            let cursor_y = inner.top() + self.cursor_y.min(inner.height - 1);

            if cursor_x < inner.right() && cursor_y < inner.bottom() {
                buf.get_mut(cursor_x, cursor_y)
                    .set_style(Style::default().add_modifier(Modifier::REVERSED));
            }
        }

        // Render scrollbar
        self.render_scrollbar(area, buf);

        // Render scroll indicator
        if self.scroll_offset > 0 {
            let indicator = format!(" ▲ {} lines above ", self.scroll_offset);
            let indicator_x = area.left() + 2;
            let indicator_y = area.top();

            for (i, ch) in indicator.chars().enumerate() {
                if indicator_x + (i as u16) < area.right() - 2 {
                    buf.get_mut(indicator_x + i as u16, indicator_y)
                        .set_symbol(&ch.to_string())
                        .set_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
                }
            }
        }
    }
}
