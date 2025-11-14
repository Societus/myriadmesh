//! I2P network view - Router status, destination, and tunnels

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Render i2p view
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),  // Router status
            Constraint::Length(7),  // Destination info
            Constraint::Min(10),    // Tunnels
        ])
        .split(area);

    render_router_status(f, app, chunks[0]);
    render_destination_info(f, app, chunks[1]);
    render_tunnels(f, app, chunks[2]);
}

/// Render router status card
fn render_router_status(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("I2P Router Status");

    let content = if let Some(status) = &app.i2p_status {
        let router_color = match status.router_status.as_str() {
            "running" => Color::Green,
            "starting" => Color::Yellow,
            "stopped" => Color::Gray,
            "error" => Color::Red,
            _ => Color::White,
        };

        let adapter_color = match status.adapter_status.as_str() {
            "ready" => Color::Green,
            "initializing" => Color::Yellow,
            "unavailable" => Color::Gray,
            "error" => Color::Red,
            _ => Color::White,
        };

        vec![
            Line::from(vec![
                Span::styled("Router Status: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &status.router_status,
                    Style::default()
                        .fg(router_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Adapter Status: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &status.adapter_status,
                    Style::default()
                        .fg(adapter_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Router Version: ", Style::default().fg(Color::Gray)),
                Span::styled(&status.router_version, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Active Tunnels: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    status.tunnels_active.to_string(),
                    Style::default().fg(if status.tunnels_active > 0 {
                        Color::Green
                    } else {
                        Color::Yellow
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Connected Peers: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    status.peers_connected.to_string(),
                    Style::default().fg(if status.peers_connected > 0 {
                        Color::Green
                    } else {
                        Color::Yellow
                    }),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    if status.router_status == "running" {
                        "✓ I2P network is operational"
                    } else {
                        "⚠ I2P network is not fully operational"
                    },
                    Style::default().fg(if status.router_status == "running" {
                        Color::Green
                    } else {
                        Color::Yellow
                    }),
                ),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "Loading I2P router status...",
            Style::default().fg(Color::Gray),
        ))]
    };

    let paragraph = Paragraph::new(content).block(block);
    f.render_widget(paragraph, area);
}

/// Render destination info card
fn render_destination_info(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("I2P Destination");

    let content = if let Some(dest) = &app.i2p_destination {
        let age = if dest.created_at > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let age_secs = now.saturating_sub(dest.created_at);
            format_duration(age_secs)
        } else {
            "Unknown".to_string()
        };

        vec![
            Line::from(vec![
                Span::styled("Destination: ", Style::default().fg(Color::Gray)),
                Span::styled(&dest.destination, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Node ID: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    if dest.node_id.len() > 16 {
                        format!("{}...", &dest.node_id[..16])
                    } else {
                        dest.node_id.clone()
                    },
                    Style::default().fg(Color::Magenta),
                ),
            ]),
            Line::from(vec![
                Span::styled("Created: ", Style::default().fg(Color::Gray)),
                Span::styled(age, Style::default().fg(Color::White)),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Use this destination for anonymous communication over I2P",
                Style::default().fg(Color::Gray),
            )]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "Loading I2P destination...",
            Style::default().fg(Color::Gray),
        ))]
    };

    let paragraph = Paragraph::new(content).block(block);
    f.render_widget(paragraph, area);
}

/// Render tunnels list
fn render_tunnels(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("I2P Tunnels");

    if let Some(tunnels) = &app.i2p_tunnels {
        let total_tunnels = tunnels.inbound_tunnels.len() + tunnels.outbound_tunnels.len();

        if total_tunnels == 0 {
            let text = Paragraph::new(vec![
                Line::from("No active tunnels"),
                Line::from(""),
                Line::from(Span::styled(
                    "Tunnels will be established as connections are made",
                    Style::default().fg(Color::Gray),
                )),
            ])
            .block(block);
            f.render_widget(text, area);
            return;
        }

        let mut items: Vec<ListItem> = Vec::new();

        // Inbound tunnels
        if !tunnels.inbound_tunnels.is_empty() {
            items.push(ListItem::new(Line::from(vec![Span::styled(
                "Inbound Tunnels:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )])));

            for tunnel in &tunnels.inbound_tunnels {
                let status_color = match tunnel.status.as_str() {
                    "active" => Color::Green,
                    "establishing" => Color::Yellow,
                    _ => Color::Red,
                };

                items.push(ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&tunnel.tunnel_id, Style::default().fg(Color::White)),
                    Span::raw(" - "),
                    Span::styled(&tunnel.status, Style::default().fg(status_color)),
                    Span::raw(format!(
                        " ({} peers, {:.0}ms, {:.1} Kbps)",
                        tunnel.peers.len(),
                        tunnel.latency_ms,
                        tunnel.bandwidth_bps as f64 / 1000.0
                    )),
                ])));
            }
        }

        // Outbound tunnels
        if !tunnels.outbound_tunnels.is_empty() {
            if !tunnels.inbound_tunnels.is_empty() {
                items.push(ListItem::new(Line::from("")));
            }

            items.push(ListItem::new(Line::from(vec![Span::styled(
                "Outbound Tunnels:",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )])));

            for tunnel in &tunnels.outbound_tunnels {
                let status_color = match tunnel.status.as_str() {
                    "active" => Color::Green,
                    "establishing" => Color::Yellow,
                    _ => Color::Red,
                };

                items.push(ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&tunnel.tunnel_id, Style::default().fg(Color::White)),
                    Span::raw(" - "),
                    Span::styled(&tunnel.status, Style::default().fg(status_color)),
                    Span::raw(format!(
                        " ({} peers, {:.0}ms, {:.1} Kbps)",
                        tunnel.peers.len(),
                        tunnel.latency_ms,
                        tunnel.bandwidth_bps as f64 / 1000.0
                    )),
                ])));
            }
        }

        // Summary
        items.push(ListItem::new(Line::from("")));
        items.push(ListItem::new(Line::from(vec![
            Span::styled("Total Bandwidth: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:.2} Mbps", tunnels.total_bandwidth_bps as f64 / 1_000_000.0),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ])));

        let list = List::new(items).block(block);
        f.render_widget(list, area);
    } else {
        let text = Paragraph::new("Loading tunnel information...").block(block);
        f.render_widget(text, area);
    }
}

/// Format duration in human-readable format
fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;

    if days > 0 {
        format!("{}d {}h ago", days, hours)
    } else if hours > 0 {
        format!("{}h {}m ago", hours, minutes)
    } else if minutes > 0 {
        format!("{}m ago", minutes)
    } else {
        "Just now".to_string()
    }
}
