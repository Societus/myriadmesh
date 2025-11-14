//! Logs view - Real-time log viewer

use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

/// Render logs view
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.log_follow {
        "Logs (Following)"
    } else {
        "Logs (Paused)"
    };

    let block = Block::default().borders(Borders::ALL).title(title);

    if app.logs.is_empty() {
        let text = ratatui::widgets::Paragraph::new(
            "No logs available\n\nPress 'f' to toggle follow mode, 'c' to clear",
        )
        .block(block)
        .style(Style::default().fg(Color::Gray));
        f.render_widget(text, area);
        return;
    }

    let items: Vec<ListItem> = app
        .logs
        .iter()
        .map(|log| {
            let level_color = match log.level.as_str() {
                "ERROR" => Color::Red,
                "WARN" => Color::Yellow,
                "INFO" => Color::Green,
                "DEBUG" => Color::Cyan,
                "TRACE" => Color::Gray,
                _ => Color::White,
            };

            let timestamp = log.timestamp.format("%H:%M:%S").to_string();

            let line = Line::from(vec![
                Span::styled(timestamp, Style::default().fg(Color::Gray)),
                Span::raw(" "),
                Span::styled(
                    format!("[{:5}]", log.level),
                    Style::default()
                        .fg(level_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(&log.message, Style::default().fg(Color::White)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(block);

    f.render_widget(list, area);
}
