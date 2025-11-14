//! Event handling for the TUI application

use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use tokio::sync::mpsc;

/// TUI events
#[derive(Debug, Clone)]
pub enum Event {
    /// Key press
    Key(KeyEvent),
    /// Terminal resize
    #[allow(dead_code)]
    Resize(u16, u16),
    /// Tick for periodic updates
    Tick,
    /// Application error
    Error(String),
}

/// Event handler
pub struct EventHandler {
    #[allow(dead_code)]
    tx: mpsc::UnboundedSender<Event>,
    rx: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn keyboard event listener
        let event_tx = tx.clone();
        tokio::spawn(async move {
            loop {
                // Poll for events with timeout
                if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            let _ = event_tx.send(Event::Key(key));
                        }
                        Ok(CrosstermEvent::Resize(w, h)) => {
                            let _ = event_tx.send(Event::Resize(w, h));
                        }
                        Err(e) => {
                            let _ = event_tx.send(Event::Error(e.to_string()));
                        }
                        _ => {}
                    }
                }
            }
        });

        // Spawn tick event generator
        let tick_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                interval.tick().await;
                let _ = tick_tx.send(Event::Tick);
            }
        });

        Self { tx, rx }
    }

    /// Get the next event
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

/// Check if key event should quit the application
pub fn should_quit(key: &KeyEvent) -> bool {
    matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            ..
        } | KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    )
}
