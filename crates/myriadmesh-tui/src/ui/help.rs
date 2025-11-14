//! Help view - Keyboard shortcuts and documentation

use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render help view
pub fn render(f: &mut Frame, _app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Help - Keyboard Shortcuts");

    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Global Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tab / Shift+Tab  ", Style::default().fg(Color::Yellow)),
            Span::raw("Navigate between views"),
        ]),
        Line::from(vec![
            Span::styled("  r                 ", Style::default().fg(Color::Yellow)),
            Span::raw("Refresh data from node"),
        ]),
        Line::from(vec![
            Span::styled("  ?                 ", Style::default().fg(Color::Yellow)),
            Span::raw("Show this help screen"),
        ]),
        Line::from(vec![
            Span::styled("  q / Ctrl+C        ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit application"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Dashboard View",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑ / ↓             ", Style::default().fg(Color::Yellow)),
            Span::raw("Navigate adapters"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Messages View",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑ / ↓             ", Style::default().fg(Color::Yellow)),
            Span::raw("Navigate messages"),
        ]),
        Line::from(vec![
            Span::styled("  s                 ", Style::default().fg(Color::Yellow)),
            Span::raw("Send new message (coming soon)"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Logs View",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  f                 ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle follow mode"),
        ]),
        Line::from(vec![
            Span::styled("  c                 ", Style::default().fg(Color::Yellow)),
            Span::raw("Clear logs"),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![Span::styled(
            "MyriadMesh TUI v0.1.0",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )]),
        Line::from(vec![Span::styled(
            "Press any key to return to dashboard",
            Style::default().fg(Color::Gray),
        )]),
    ];

    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, area);
}
