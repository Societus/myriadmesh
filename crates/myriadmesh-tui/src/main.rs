//! MyriadMesh TUI - Terminal User Interface
//!
//! A terminal-based user interface for managing MyriadMesh nodes.

mod api_client;
mod app;
mod events;
mod ui;

use anyhow::Result;
use app::{App, View};
use clap::Parser;
use crossterm::{
    event::KeyCode,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use events::{Event, EventHandler};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "myriadmesh-tui")]
#[command(about = "Terminal User Interface for MyriadMesh node management")]
struct Args {
    /// MyriadNode API URL
    #[arg(short, long, default_value = "http://localhost:4000")]
    api_url: String,

    /// Refresh interval in seconds
    #[arg(short, long, default_value = "2")]
    refresh_interval: u64,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    if args.debug {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    }

    info!("Starting MyriadMesh TUI");
    info!("Connecting to: {}", args.api_url);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(args.api_url);

    // Initial data fetch
    if let Err(e) = app.refresh().await {
        app.error = Some(format!("Failed to connect: {}", e));
        app.add_log("ERROR".to_string(), format!("Failed to connect: {}", e));
    } else {
        app.add_log("INFO".to_string(), "Connected to MyriadNode".to_string());
    }

    // Create event handler
    let mut events = EventHandler::new(Duration::from_secs(args.refresh_interval));

    // Main loop
    let result = run_app(&mut terminal, &mut app, &mut events).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    info!("MyriadMesh TUI shutdown");
    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    events: &mut EventHandler,
) -> Result<()> {
    loop {
        // Render UI
        terminal.draw(|f| ui::render(f, app))?;

        // Handle events
        if let Some(event) = events.next().await {
            match event {
                Event::Key(key) => {
                    // Global shortcuts
                    if events::should_quit(&key) {
                        app.quit();
                    } else if key.code == KeyCode::Char('?') {
                        app.show_help();
                    } else if key.code == KeyCode::Char('r') {
                        app.add_log("INFO".to_string(), "Refreshing data...".to_string());
                        if let Err(e) = app.refresh().await {
                            app.error = Some(e.to_string());
                            app.add_log("ERROR".to_string(), e.to_string());
                        }
                    } else if key.code == KeyCode::Tab {
                        app.next_view();
                    } else if key.code == KeyCode::BackTab {
                        app.previous_view();
                    } else {
                        // View-specific shortcuts
                        handle_view_input(app, key).await;
                    }
                }
                Event::Tick => {
                    // Auto-refresh on tick
                    if let Err(e) = app.refresh().await {
                        app.error = Some(e.to_string());
                    }
                }
                Event::Resize(_, _) => {
                    // Terminal resized, will be handled on next render
                }
                Event::Error(e) => {
                    app.error = Some(e.clone());
                    app.add_log("ERROR".to_string(), e);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

async fn handle_view_input(app: &mut App, key: crossterm::event::KeyEvent) {
    match app.current_view {
        View::Dashboard => match key.code {
            KeyCode::Up => app.previous_adapter(),
            KeyCode::Down => app.next_adapter(),
            _ => {}
        },
        View::Messages => match key.code {
            KeyCode::Up => app.previous_message(),
            KeyCode::Down => app.next_message(),
            _ => {}
        },
        View::Logs => match key.code {
            KeyCode::Char('f') => {
                app.toggle_log_follow();
                app.add_log(
                    "INFO".to_string(),
                    format!(
                        "Log follow mode: {}",
                        if app.log_follow { "ON" } else { "OFF" }
                    ),
                );
            }
            KeyCode::Char('c') => {
                app.clear_logs();
                app.add_log("INFO".to_string(), "Logs cleared".to_string());
            }
            _ => {}
        },
        View::Help => {
            // Any key returns to dashboard
            app.current_view = View::Dashboard;
        }
        _ => {}
    }
}
