// ABOUTME: Help overlay component displaying keyboard shortcuts and commands

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem},
    style::{Color, Modifier, Style},
};

pub struct HelpComponent;

impl HelpComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let popup_area = self.centered_rect(60, 80, area);
        
        frame.render_widget(Clear, popup_area);
        
        let help_items = vec![
            ListItem::new("Navigation:").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ListItem::new("  j/↓        Move down"),
            ListItem::new("  k/↑        Move up"),
            ListItem::new("  h/←        Previous workspace"),
            ListItem::new("  l/→        Next workspace"),
            ListItem::new("  g          Go to top"),
            ListItem::new("  G          Go to bottom"),
            ListItem::new(""),
            ListItem::new("Session Actions:").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ListItem::new("  n          New session"),
            ListItem::new("  a          Attach to session"),
            ListItem::new("  s          Start/Stop session"),
            ListItem::new("  d          Delete session"),
            ListItem::new(""),
            ListItem::new("Views:").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ListItem::new("  Tab        Switch between views"),
            ListItem::new(""),
            ListItem::new("General:").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ListItem::new("  ?          Toggle this help"),
            ListItem::new("  q/Esc      Quit application"),
            ListItem::new("  Ctrl+C     Force quit"),
        ];

        let help_list = List::new(help_items)
            .block(
                Block::default()
                    .title("Help - Press ? or Esc to close")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
            );

        frame.render_widget(help_list, popup_area);
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

impl Default for HelpComponent {
    fn default() -> Self {
        Self::new()
    }
}