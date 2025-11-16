//! Event handling for the TUI application

use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

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
    // RESOURCE M4: Task handle management for graceful shutdown
    shutdown_tx: broadcast::Sender<()>,
    keyboard_task: JoinHandle<()>,
    tick_task: JoinHandle<()>,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        // RESOURCE M4: Shutdown channel for graceful task termination
        let (shutdown_tx, mut shutdown_rx1) = broadcast::channel::<()>(1);
        let mut shutdown_rx2 = shutdown_tx.subscribe();

        // RESOURCE M4: Spawn keyboard event listener with shutdown handling
        let event_tx = tx.clone();
        let keyboard_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx1.recv() => {
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Poll for events with timeout
                        if event::poll(Duration::from_millis(10)).unwrap_or(false) {
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
                }
            }
        });

        // RESOURCE M4: Spawn tick event generator with shutdown handling
        let tick_tx = tx.clone();
        let tick_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                tokio::select! {
                    _ = shutdown_rx2.recv() => {
                        break;
                    }
                    _ = interval.tick() => {
                        let _ = tick_tx.send(Event::Tick);
                    }
                }
            }
        });

        Self {
            tx,
            rx,
            shutdown_tx,
            keyboard_task,
            tick_task,
        }
    }

    /// Gracefully shutdown event handler and wait for tasks to complete
    /// RESOURCE M4: Prevents task handle leaks and ensures cleanup
    pub async fn shutdown(self) {
        // Send shutdown signal
        let _ = self.shutdown_tx.send(());
        drop(self.shutdown_tx);

        // Wait for tasks to complete
        let _ = self.keyboard_task.await;
        let _ = self.tick_task.await;
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
