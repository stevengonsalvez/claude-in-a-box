// ABOUTME: Live Docker log streaming component for real-time container monitoring

use crate::app::AppState;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    style::{Color, Style, Modifier},
};

pub struct LiveLogsStreamComponent {
    auto_scroll: bool,
    scroll_offset: usize,
    max_visible_lines: usize,
    show_timestamps: bool,
    filter_level: LogLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    All,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::All => "ALL",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN", 
            LogLevel::Error => "ERROR",
        }
    }

    fn next(&self) -> Self {
        match self {
            LogLevel::All => LogLevel::Info,
            LogLevel::Info => LogLevel::Warn,
            LogLevel::Warn => LogLevel::Error,
            LogLevel::Error => LogLevel::All,
        }
    }
}

impl LiveLogsStreamComponent {
    pub fn new() -> Self {
        Self {
            auto_scroll: true,
            scroll_offset: 0,
            max_visible_lines: 20,
            show_timestamps: true,
            filter_level: LogLevel::All,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        // Get logs from the selected session
        let session_logs = self.get_session_logs(state);
        
        // Filter logs based on level
        let filtered_logs = self.filter_logs(&session_logs);
        
        let title = self.build_title(state, filtered_logs.len(), session_logs.len());
        
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_style(Style::default().fg(Color::Blue))
            .border_style(Style::default().fg(Color::Gray));

        if filtered_logs.is_empty() {
            let empty_message = match self.filter_level {
                LogLevel::All => "No logs available\n\nLogs will appear here when containers are active.",
                _ => &format!("No {} level logs\n\nAdjust filter level with 'f' key.", self.filter_level.as_str().to_lowercase()),
            };
            
            frame.render_widget(
                Paragraph::new(empty_message)
                    .block(block)
                    .style(Style::default().fg(Color::Gray))
                    .alignment(Alignment::Center),
                area
            );
            return;
        }

        // Create list items from logs
        let log_items = self.create_log_items(&filtered_logs);
        
        let list = List::new(log_items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        frame.render_widget(list, area);

        // Render controls hint
        self.render_controls_hint(frame, area);

        // Auto-scroll to bottom if enabled
        if self.auto_scroll && !filtered_logs.is_empty() {
            self.scroll_to_bottom(filtered_logs.len());
        }
    }

    fn get_session_logs(&self, state: &AppState) -> Vec<LogEntry> {
        // Get logs from currently selected session or all active sessions
        if let Some(session) = state.selected_session() {
            // Get logs for specific session
            state.live_logs
                .get(&session.id)
                .cloned()
                .unwrap_or_default()
        } else {
            // Aggregate logs from all active sessions
            let mut all_logs = Vec::new();
            for workspace in &state.workspaces {
                for session in &workspace.sessions {
                    if let Some(logs) = state.live_logs.get(&session.id) {
                        all_logs.extend(logs.iter().cloned());
                    }
                }
            }
            
            // Sort by timestamp
            all_logs.sort_by_key(|log| log.timestamp);
            all_logs
        }
    }

    fn filter_logs<'a>(&self, logs: &'a [LogEntry]) -> Vec<&'a LogEntry> {
        logs.iter()
            .filter(|log| self.should_include_log(log))
            .collect()
    }

    fn should_include_log(&self, log: &LogEntry) -> bool {
        match self.filter_level {
            LogLevel::All => true,
            LogLevel::Info => matches!(log.level, LogEntryLevel::Info | LogEntryLevel::Warn | LogEntryLevel::Error),
            LogLevel::Warn => matches!(log.level, LogEntryLevel::Warn | LogEntryLevel::Error),
            LogLevel::Error => matches!(log.level, LogEntryLevel::Error),
        }
    }

    fn build_title(&self, state: &AppState, filtered_count: usize, total_count: usize) -> String {
        let session_info = if let Some(session) = state.selected_session() {
            format!(" {} ", session.branch_name)
        } else {
            " All Sessions ".to_string()
        };

        let filter_info = if self.filter_level != LogLevel::All {
            format!(" [{}] ", self.filter_level.as_str())
        } else {
            String::new()
        };

        let count_info = if filtered_count != total_count {
            format!(" ({}/{}) ", filtered_count, total_count)
        } else {
            format!(" ({}) ", total_count)
        };

        format!("🔴 Live Logs{}{}{}", session_info, filter_info, count_info)
    }

    fn create_log_items(&self, logs: &[&LogEntry]) -> Vec<ListItem> {
        let start_idx = if self.auto_scroll {
            logs.len().saturating_sub(self.max_visible_lines)
        } else {
            self.scroll_offset
        };

        logs.iter()
            .skip(start_idx)
            .take(self.max_visible_lines)
            .map(|log| self.format_log_entry(log))
            .collect()
    }

    fn format_log_entry(&self, log: &LogEntry) -> ListItem {
        let timestamp_str = if self.show_timestamps {
            format!("[{}] ", log.timestamp.format("%H:%M:%S"))
        } else {
            String::new()
        };

        let (level_icon, level_color) = match log.level {
            LogEntryLevel::Debug => ("🔍", Color::DarkGray),
            LogEntryLevel::Info => ("ℹ️", Color::Blue),
            LogEntryLevel::Warn => ("⚠️", Color::Yellow),
            LogEntryLevel::Error => ("❌", Color::Red),
        };

        let source_str = if !log.source.is_empty() {
            format!("[{}] ", log.source)
        } else {
            String::new()
        };

        let content = format!("{}{} {}{}", 
            timestamp_str, 
            level_icon,
            source_str,
            log.message
        );

        ListItem::new(content).style(Style::default().fg(level_color))
    }

    fn render_controls_hint(&self, frame: &mut Frame, area: Rect) {
        if area.height < 4 {
            return; // Not enough space
        }

        let controls = format!("[f]Filter:{} [t]Time [↑↓]Scroll [Space]AutoScroll:{}", 
            self.filter_level.as_str(), 
            if self.auto_scroll { "ON" } else { "OFF" }
        );

        let hint_area = Rect {
            x: area.x + 1,
            y: area.y + area.height - 2,
            width: area.width.saturating_sub(2),
            height: 1,
        };

        frame.render_widget(
            Paragraph::new(controls)
                .style(Style::default().fg(Color::DarkGray)),
            hint_area
        );
    }

    /// Toggle auto-scroll mode
    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
    }

    /// Toggle timestamp display
    pub fn toggle_timestamps(&mut self) {
        self.show_timestamps = !self.show_timestamps;
    }

    /// Cycle through filter levels
    pub fn cycle_filter_level(&mut self) {
        self.filter_level = self.filter_level.next();
    }

    /// Scroll up manually
    pub fn scroll_up(&mut self) {
        if !self.auto_scroll && self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Scroll down manually
    pub fn scroll_down(&mut self, total_logs: usize) {
        if !self.auto_scroll && self.scroll_offset + self.max_visible_lines < total_logs {
            self.scroll_offset += 1;
        }
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self, total_logs: usize) {
        self.scroll_offset = if total_logs > self.max_visible_lines {
            total_logs - self.max_visible_lines
        } else {
            0
        };
    }

    /// Update max visible lines based on area height
    pub fn update_max_visible(&mut self, area_height: u16) {
        self.max_visible_lines = ((area_height as usize).saturating_sub(4)).max(5);
    }
}

impl Default for LiveLogsStreamComponent {
    fn default() -> Self {
        Self::new()
    }
}

// Log entry types that correspond to app state
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: LogEntryLevel,
    pub source: String,  // Container name or source
    pub message: String,
    pub session_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogEntryLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogEntry {
    pub fn new(level: LogEntryLevel, source: String, message: String) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            level,
            source,
            message,
            session_id: None,
        }
    }

    pub fn with_session(mut self, session_id: uuid::Uuid) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Parse log level from Docker log line
    pub fn parse_level_from_message(message: &str) -> LogEntryLevel {
        let lower_msg = message.to_lowercase();
        if lower_msg.contains("error") || lower_msg.contains("fatal") {
            LogEntryLevel::Error
        } else if lower_msg.contains("warn") || lower_msg.contains("warning") {
            LogEntryLevel::Warn
        } else if lower_msg.contains("debug") {
            LogEntryLevel::Debug
        } else {
            LogEntryLevel::Info
        }
    }

    /// Create from raw Docker log line
    pub fn from_docker_log(container_name: &str, log_line: &str, session_id: Option<uuid::Uuid>) -> Self {
        let level = Self::parse_level_from_message(log_line);
        Self {
            timestamp: chrono::Utc::now(),
            level,
            source: container_name.to_string(),
            message: log_line.to_string(),
            session_id,
        }
    }
}