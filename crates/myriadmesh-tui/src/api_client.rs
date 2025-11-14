//! API client for communicating with MyriadNode REST API

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MyriadNode API client
#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// Check node health
    #[allow(dead_code)]
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/health", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to check health")?
            .json()
            .await
            .context("Failed to parse health response")?;
        Ok(response)
    }

    /// Get node information
    pub async fn node_info(&self) -> Result<NodeInfo> {
        let url = format!("{}/api/v1/node/info", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get node info")?
            .json()
            .await
            .context("Failed to parse node info")?;
        Ok(response)
    }

    /// Get node status
    pub async fn node_status(&self) -> Result<NodeStatus> {
        let url = format!("{}/api/v1/node/status", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get node status")?
            .json()
            .await
            .context("Failed to parse node status")?;
        Ok(response)
    }

    /// Get adapter list
    pub async fn adapters(&self) -> Result<Vec<AdapterInfo>> {
        let url = format!("{}/api/v1/adapters", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get adapters")?
            .json()
            .await
            .context("Failed to parse adapters")?;
        Ok(response)
    }

    /// Get message list
    pub async fn messages(&self) -> Result<Vec<Message>> {
        let url = format!("{}/api/v1/messages/list", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get messages")?
            .json()
            .await
            .context("Failed to parse messages")?;
        Ok(response)
    }

    /// Send a message
    #[allow(dead_code)]
    pub async fn send_message(&self, request: SendMessageRequest) -> Result<SendMessageResponse> {
        let url = format!("{}/api/v1/messages/send", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send message")?
            .json()
            .await
            .context("Failed to parse send response")?;
        Ok(response)
    }

    /// Get DHT nodes
    pub async fn dht_nodes(&self) -> Result<Vec<DhtNode>> {
        let url = format!("{}/api/v1/dht/nodes", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get DHT nodes")?
            .json()
            .await
            .context("Failed to parse DHT nodes")?;
        Ok(response)
    }

    /// Get heartbeat statistics
    pub async fn heartbeat_stats(&self) -> Result<HeartbeatStats> {
        let url = format!("{}/api/v1/heartbeat/stats", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get heartbeat stats")?
            .json()
            .await
            .context("Failed to parse heartbeat stats")?;
        Ok(response)
    }

    /// Get i2p router status
    pub async fn i2p_status(&self) -> Result<I2pStatus> {
        let url = format!("{}/api/i2p/status", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get i2p status")?
            .json()
            .await
            .context("Failed to parse i2p status")?;
        Ok(response)
    }

    /// Get i2p destination
    pub async fn i2p_destination(&self) -> Result<I2pDestination> {
        let url = format!("{}/api/i2p/destination", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get i2p destination")?
            .json()
            .await
            .context("Failed to parse i2p destination")?;
        Ok(response)
    }

    /// Get i2p tunnels
    pub async fn i2p_tunnels(&self) -> Result<I2pTunnels> {
        let url = format!("{}/api/i2p/tunnels", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get i2p tunnels")?
            .json()
            .await
            .context("Failed to parse i2p tunnels")?;
        Ok(response)
    }
}

// Response types

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub version: String,
    pub uptime_secs: u64,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeStatus {
    pub active_connections: usize,
    pub queued_messages: usize,
    pub known_nodes: usize,
    pub primary_adapter: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdapterInfo {
    pub adapter_id: String,
    pub adapter_type: String,
    pub status: String,
    pub is_primary: bool,
    pub is_backhaul: bool,
    pub capabilities: AdapterCapabilities,
    pub metrics: Option<AdapterMetrics>,
    pub health_status: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdapterCapabilities {
    pub max_message_size: usize,
    pub typical_latency_ms: f64,
    pub typical_bandwidth_bps: u64,
    pub max_range_meters: u64,
    pub power_consumption: String,
    pub supports_broadcast: bool,
    pub supports_multicast: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdapterMetrics {
    pub latency_ms: Option<f64>,
    pub bandwidth_bps: Option<u64>,
    pub packet_loss: Option<f64>,
    pub last_test: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub message_id: String,
    pub from: String,
    pub to: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub status: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SendMessageRequest {
    pub destination: String,
    pub content: String,
    pub priority: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DhtNode {
    pub node_id: String,
    pub address: String,
    pub last_seen: DateTime<Utc>,
    pub reputation: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeartbeatStats {
    pub total_nodes: usize,
    pub nodes_with_location: usize,
    pub adapter_counts: HashMap<String, usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct I2pStatus {
    pub router_status: String,
    pub adapter_status: String,
    pub router_version: String,
    pub tunnels_active: usize,
    pub peers_connected: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct I2pDestination {
    pub destination: String,
    pub created_at: u64,
    pub node_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct I2pTunnels {
    pub inbound_tunnels: Vec<I2pTunnelInfo>,
    pub outbound_tunnels: Vec<I2pTunnelInfo>,
    pub total_bandwidth_bps: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct I2pTunnelInfo {
    pub tunnel_id: String,
    pub peers: Vec<String>,
    pub latency_ms: f64,
    pub bandwidth_bps: u64,
    pub status: String,
}
