//! Dashboard view - Node status and adapter overview

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Render dashboard view
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // Node info
            Constraint::Length(8), // Statistics
            Constraint::Min(10),   // Adapters
        ])
        .split(area);

    render_node_info(f, app, chunks[0]);
    render_statistics(f, app, chunks[1]);
    render_adapters(f, app, chunks[2]);
}

/// Render node information card
fn render_node_info(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Node Information");

    let content = if let Some(info) = &app.node_info {
        let uptime = format_uptime(info.uptime_secs);
        vec![
            Line::from(vec![
                Span::styled("Node ID: ", Style::default().fg(Color::Gray)),
                Span::styled(&info.node_id[..16], Style::default().fg(Color::Cyan)),
                Span::raw("..."),
            ]),
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::Gray)),
                Span::styled(&info.name, Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::styled("Version: ", Style::default().fg(Color::Gray)),
                Span::styled(&info.version, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Uptime: ", Style::default().fg(Color::Gray)),
                Span::styled(uptime, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("Primary Node: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    if info.is_primary { "Yes" } else { "No" },
                    Style::default().fg(if info.is_primary {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "Loading node information...",
            Style::default().fg(Color::Gray),
        ))]
    };

    let paragraph = Paragraph::new(content).block(block);
    f.render_widget(paragraph, area);
}

/// Render statistics card
fn render_statistics(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Network Statistics");

    let content = if let Some(status) = &app.node_status {
        vec![
            Line::from(vec![
                Span::styled("Active Connections: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    status.active_connections.to_string(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Queued Messages: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    status.queued_messages.to_string(),
                    Style::default().fg(if status.queued_messages > 10 {
                        Color::Yellow
                    } else {
                        Color::White
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Known Nodes (DHT): ", Style::default().fg(Color::Gray)),
                Span::styled(
                    status.known_nodes.to_string(),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled("Primary Adapter: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    status.primary_adapter.as_deref().unwrap_or("None"),
                    Style::default().fg(Color::Magenta),
                ),
            ]),
            Line::from(vec![
                Span::styled("Active Adapters: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    app.adapters
                        .iter()
                        .filter(|a| a.status == "Active")
                        .count()
                        .to_string(),
                    Style::default().fg(Color::Green),
                ),
                Span::raw(" / "),
                Span::styled(
                    app.adapters.len().to_string(),
                    Style::default().fg(Color::White),
                ),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "Loading statistics...",
            Style::default().fg(Color::Gray),
        ))]
    };

    let paragraph = Paragraph::new(content).block(block);
    f.render_widget(paragraph, area);
}

/// Render adapters list
fn render_adapters(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Network Adapters");

    if app.adapters.is_empty() {
        let text = Paragraph::new("No adapters available").block(block);
        f.render_widget(text, area);
        return;
    }

    let items: Vec<ListItem> = app
        .adapters
        .iter()
        .enumerate()
        .map(|(i, adapter)| {
            let status_color = match adapter.status.as_str() {
                "Active" => Color::Green,
                "Ready" => Color::Cyan,
                "Stopped" => Color::Gray,
                "Error" => Color::Red,
                _ => Color::Yellow,
            };

            let primary_marker = if adapter.is_primary { " [PRIMARY]" } else { "" };
            let backhaul_marker = if adapter.is_backhaul {
                " [BACKHAUL]"
            } else {
                ""
            };

            let health_symbol = match adapter.health_status.as_str() {
                "Healthy" => "●",
                "Degraded" => "◐",
                _ => "○",
            };

            let health_color = match adapter.health_status.as_str() {
                "Healthy" => Color::Green,
                "Degraded" => Color::Yellow,
                _ => Color::Red,
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", health_symbol),
                    Style::default().fg(health_color),
                ),
                Span::styled(
                    adapter.adapter_type.clone(),
                    Style::default()
                        .fg(status_color)
                        .add_modifier(if i == app.selected_adapter {
                            Modifier::BOLD | Modifier::UNDERLINED
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(
                    format!("{}{}", primary_marker, backhaul_marker),
                    Style::default().fg(Color::Magenta),
                ),
                Span::raw(" - "),
                Span::styled(adapter.status.clone(), Style::default().fg(status_color)),
                Span::raw(" - "),
                Span::styled(
                    format!(
                        "{:.0}ms, {:.1}Mbps",
                        adapter.capabilities.typical_latency_ms,
                        adapter.capabilities.typical_bandwidth_bps as f64 / 1_000_000.0
                    ),
                    Style::default().fg(Color::Gray),
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

/// Format uptime in human-readable format
fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}
