//! Messages view - Message management

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Render messages view
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),   // Message list
            Constraint::Length(5), // Send message form
        ])
        .split(area);

    render_message_list(f, app, chunks[0]);
    render_send_form(f, app, chunks[1]);
}

/// Render message list
fn render_message_list(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Messages ({})", app.messages.len()));

    if app.messages.is_empty() {
        let text = Paragraph::new("No messages")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        f.render_widget(text, area);
        return;
    }

    let items: Vec<ListItem> = app
        .messages
        .iter()
        .enumerate()
        .map(|(i, msg)| {
            let status_color = match msg.status.as_str() {
                "Sent" | "Delivered" => Color::Green,
                "Pending" => Color::Yellow,
                "Failed" => Color::Red,
                _ => Color::Gray,
            };

            let timestamp = msg.timestamp.format("%H:%M:%S").to_string();
            let from_preview = if msg.from.len() >= 8 {
                msg.from[..8].to_string()
            } else {
                msg.from.clone()
            };
            let to_preview = if msg.to.len() >= 8 {
                msg.to[..8].to_string()
            } else {
                msg.to.clone()
            };

            let line = Line::from(vec![
                Span::styled(timestamp, Style::default().fg(Color::Gray)),
                Span::raw(" "),
                Span::styled(msg.status.clone(), Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(from_preview, Style::default().fg(Color::Cyan)),
                Span::raw(" → "),
                Span::styled(to_preview, Style::default().fg(Color::Magenta)),
                Span::raw(": "),
                Span::styled(
                    msg.content.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(if i == app.selected_message {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    f.render_widget(list, area);
}

/// Render send message form
fn render_send_form(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Send Message (Coming Soon)");

    let content = vec![
        Line::from(vec![
            Span::styled("To: ", Style::default().fg(Color::Gray)),
            Span::styled(&app.message_destination, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Message: ", Style::default().fg(Color::Gray)),
            Span::styled(&app.message_input, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press 's' to compose a new message",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )),
    ];

    let paragraph = Paragraph::new(content).block(block);
    f.render_widget(paragraph, area);
}
