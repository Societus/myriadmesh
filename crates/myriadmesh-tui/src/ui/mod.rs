//! UI rendering module

pub mod dashboard;
pub mod help;
pub mod i2p;
pub mod logs;
pub mod messages;

use crate::app::{App, View};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Tabs},
    Frame,
};

/// Render the entire UI
pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Render header with tabs
    render_header(f, app, chunks[0]);

    // Render current view
    match app.current_view {
        View::Dashboard => dashboard::render(f, app, chunks[1]),
        View::Messages => messages::render(f, app, chunks[1]),
        View::I2p => i2p::render(f, app, chunks[1]),
        View::Logs => logs::render(f, app, chunks[1]),
        View::Help => help::render(f, app, chunks[1]),
    }

    // Render footer
    render_footer(f, app, chunks[2]);
}

/// Render header with navigation tabs
fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec![
        View::Dashboard.title(),
        View::Messages.title(),
        View::I2p.title(),
        View::Logs.title(),
    ];

    let selected = match app.current_view {
        View::Dashboard => 0,
        View::Messages => 1,
        View::I2p => 2,
        View::Logs => 3,
        View::Help => 0,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("MyriadMesh TUI"),
        )
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

/// Render footer with status and shortcuts
fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let mut footer_text = vec![
        Span::raw("Tab: Navigate | "),
        Span::raw("?: Help | "),
        Span::raw("r: Refresh | "),
        Span::raw("q: Quit"),
    ];

    // Add status indicator
    if app.is_loading {
        footer_text.insert(
            0,
            Span::styled(" LOADING ", Style::default().fg(Color::Yellow)),
        );
        footer_text.insert(1, Span::raw(" | "));
    }

    if let Some(error) = &app.error {
        footer_text.insert(
            0,
            Span::styled(
                format!(" ERROR: {} ", error),
                Style::default().fg(Color::Red),
            ),
        );
        footer_text.insert(1, Span::raw(" | "));
    }

    let footer = Line::from(footer_text);
    let footer_widget = ratatui::widgets::Paragraph::new(footer)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    f.render_widget(footer_widget, area);
}
