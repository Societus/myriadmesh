//! Application state and navigation

use crate::api_client::{
    AdapterInfo, ApiClient, DhtNode, HeartbeatStats, Message, NodeInfo, NodeStatus,
};
use anyhow::Result;

/// Active view in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard,
    Messages,
    Config,
    Logs,
    Help,
}

impl View {
    /// Get the next view (tab navigation)
    pub fn next(&self) -> Self {
        match self {
            View::Dashboard => View::Messages,
            View::Messages => View::Config,
            View::Config => View::Logs,
            View::Logs => View::Dashboard,
            View::Help => View::Dashboard,
        }
    }

    /// Get the previous view (shift+tab navigation)
    pub fn previous(&self) -> Self {
        match self {
            View::Dashboard => View::Logs,
            View::Messages => View::Dashboard,
            View::Config => View::Messages,
            View::Logs => View::Config,
            View::Help => View::Dashboard,
        }
    }

    /// Get view title
    pub fn title(&self) -> &'static str {
        match self {
            View::Dashboard => "Dashboard",
            View::Messages => "Messages",
            View::Config => "Configuration",
            View::Logs => "Logs",
            View::Help => "Help",
        }
    }
}

/// Application state
pub struct App {
    /// API client
    pub api_client: ApiClient,
    /// Current view
    pub current_view: View,
    /// Should quit
    pub should_quit: bool,
    /// Node information
    pub node_info: Option<NodeInfo>,
    /// Node status
    pub node_status: Option<NodeStatus>,
    /// Adapters
    pub adapters: Vec<AdapterInfo>,
    /// Messages
    pub messages: Vec<Message>,
    /// DHT nodes
    pub dht_nodes: Vec<DhtNode>,
    /// Heartbeat statistics
    pub heartbeat_stats: Option<HeartbeatStats>,
    /// Error message
    pub error: Option<String>,
    /// Loading state
    pub is_loading: bool,
    /// Message input buffer
    pub message_input: String,
    /// Message destination input
    pub message_destination: String,
    /// Selected message index
    pub selected_message: usize,
    /// Selected adapter index
    pub selected_adapter: usize,
    /// Log buffer
    pub logs: Vec<LogEntry>,
    /// Log follow mode
    pub log_follow: bool,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub message: String,
}

impl App {
    /// Create a new application
    pub fn new(api_url: String) -> Self {
        Self {
            api_client: ApiClient::new(api_url),
            current_view: View::Dashboard,
            should_quit: false,
            node_info: None,
            node_status: None,
            adapters: Vec::new(),
            messages: Vec::new(),
            dht_nodes: Vec::new(),
            heartbeat_stats: None,
            error: None,
            is_loading: false,
            message_input: String::new(),
            message_destination: String::new(),
            selected_message: 0,
            selected_adapter: 0,
            logs: Vec::new(),
            log_follow: true,
        }
    }

    /// Refresh all data from API
    pub async fn refresh(&mut self) -> Result<()> {
        self.is_loading = true;
        self.error = None;

        // Fetch all data in parallel
        let (node_info, node_status, adapters, messages, dht_nodes, heartbeat_stats) = tokio::join!(
            self.api_client.node_info(),
            self.api_client.node_status(),
            self.api_client.adapters(),
            self.api_client.messages(),
            self.api_client.dht_nodes(),
            self.api_client.heartbeat_stats(),
        );

        // Update state
        self.node_info = node_info.ok();
        self.node_status = node_status.ok();
        self.adapters = adapters.unwrap_or_default();
        self.messages = messages.unwrap_or_default();
        self.dht_nodes = dht_nodes.unwrap_or_default();
        self.heartbeat_stats = heartbeat_stats.ok();

        self.is_loading = false;
        Ok(())
    }

    /// Navigate to next view
    pub fn next_view(&mut self) {
        self.current_view = self.current_view.next();
    }

    /// Navigate to previous view
    pub fn previous_view(&mut self) {
        self.current_view = self.current_view.previous();
    }

    /// Show help
    pub fn show_help(&mut self) {
        self.current_view = View::Help;
    }

    /// Quit application
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Select next message
    pub fn next_message(&mut self) {
        if !self.messages.is_empty() {
            self.selected_message = (self.selected_message + 1) % self.messages.len();
        }
    }

    /// Select previous message
    pub fn previous_message(&mut self) {
        if !self.messages.is_empty() {
            self.selected_message = if self.selected_message > 0 {
                self.selected_message - 1
            } else {
                self.messages.len() - 1
            };
        }
    }

    /// Select next adapter
    pub fn next_adapter(&mut self) {
        if !self.adapters.is_empty() {
            self.selected_adapter = (self.selected_adapter + 1) % self.adapters.len();
        }
    }

    /// Select previous adapter
    pub fn previous_adapter(&mut self) {
        if !self.adapters.is_empty() {
            self.selected_adapter = if self.selected_adapter > 0 {
                self.selected_adapter - 1
            } else {
                self.adapters.len() - 1
            };
        }
    }

    /// Toggle log follow mode
    pub fn toggle_log_follow(&mut self) {
        self.log_follow = !self.log_follow;
    }

    /// Add log entry
    pub fn add_log(&mut self, level: String, message: String) {
        self.logs.push(LogEntry {
            timestamp: chrono::Utc::now(),
            level,
            message,
        });

        // Keep only last 1000 logs
        if self.logs.len() > 1000 {
            self.logs.drain(0..self.logs.len() - 1000);
        }
    }

    /// Clear logs
    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }
}
