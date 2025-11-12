//! Node information for DHT routing table

use myriadmesh_protocol::types::AdapterType;
use myriadmesh_protocol::NodeId as ProtocolNodeId;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::reputation::NodeReputation;

/// Get current timestamp
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Information about a network adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    /// Adapter type
    pub adapter_type: AdapterType,

    /// Address for this adapter (protocol-specific)
    pub address: String,

    /// Whether this adapter is currently active
    pub active: bool,
}

/// Node capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// Can relay messages
    pub can_relay: bool,

    /// Can store DHT data
    pub can_store: bool,

    /// Maximum message size this node can handle
    pub max_message_size: usize,

    /// Available storage (bytes)
    pub available_storage: u64,
}

impl Default for NodeCapabilities {
    fn default() -> Self {
        NodeCapabilities {
            can_relay: true,
            can_store: true,
            max_message_size: 1024 * 1024, // 1MB
            available_storage: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// Information about a node in the DHT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node identifier (32 bytes)
    pub node_id: ProtocolNodeId,

    /// Available network adapters
    pub adapters: Vec<AdapterInfo>,

    /// Last successful communication (Unix timestamp)
    pub last_seen: u64,

    /// Round-trip time in milliseconds
    pub rtt_ms: f64,

    /// Consecutive failures
    pub failures: u32,

    /// Reputation tracking
    pub reputation: NodeReputation,

    /// Node capabilities
    pub capabilities: NodeCapabilities,

    /// First seen timestamp
    pub first_seen: u64,

    /// Total successful communications
    pub total_successes: u64,
}

impl NodeInfo {
    /// Create new node info
    pub fn new(node_id: ProtocolNodeId) -> Self {
        let now = now();
        NodeInfo {
            node_id,
            adapters: Vec::new(),
            last_seen: now,
            rtt_ms: 0.0,
            failures: 0,
            reputation: NodeReputation::new(),
            capabilities: NodeCapabilities::default(),
            first_seen: now,
            total_successes: 0,
        }
    }

    /// Create with adapters
    pub fn with_adapters(node_id: ProtocolNodeId, adapters: Vec<AdapterInfo>) -> Self {
        let mut info = Self::new(node_id);
        info.adapters = adapters;
        info
    }

    /// Record successful communication
    pub fn record_success(&mut self, rtt_ms: f64) {
        self.last_seen = now();
        self.rtt_ms = rtt_ms;
        self.failures = 0;
        self.total_successes += 1;
        self.reputation.record_success();
    }

    /// Record failed communication
    pub fn record_failure(&mut self) {
        self.failures += 1;
        self.reputation.record_failure();
    }

    /// Check if node is likely stale
    pub fn is_stale(&self, max_age_secs: u64) -> bool {
        let age = now().saturating_sub(self.last_seen);
        age > max_age_secs
    }

    /// Check if node should be evicted
    pub fn should_evict(&self, max_failures: u32, max_age_secs: u64) -> bool {
        self.failures >= max_failures || self.is_stale(max_age_secs)
    }

    /// Get best adapter for communication
    pub fn get_best_adapter(&self) -> Option<&AdapterInfo> {
        self.adapters.iter().find(|a| a.active)
    }

    /// Calculate XOR distance to another node
    pub fn distance_to(&self, other: &ProtocolNodeId) -> [u8; 32] {
        self.node_id.distance(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node() -> NodeInfo {
        NodeInfo::new(ProtocolNodeId::from_bytes([1u8; 32]))
    }

    #[test]
    fn test_new_node() {
        let node = create_test_node();
        assert_eq!(node.failures, 0);
        assert_eq!(node.total_successes, 0);
        assert!(node.reputation.is_trustworthy());
    }

    #[test]
    fn test_record_success() {
        let mut node = create_test_node();

        node.record_success(10.5);
        assert_eq!(node.failures, 0);
        assert_eq!(node.total_successes, 1);
        assert_eq!(node.rtt_ms, 10.5);
    }

    #[test]
    fn test_record_failure() {
        let mut node = create_test_node();

        node.record_failure();
        assert_eq!(node.failures, 1);
    }

    #[test]
    fn test_should_evict() {
        let mut node = create_test_node();

        // Fresh node should not be evicted
        assert!(!node.should_evict(3, 3600));

        // Too many failures
        node.failures = 5;
        assert!(node.should_evict(3, 3600));
    }

    #[test]
    fn test_is_stale() {
        let mut node = create_test_node();

        // Fresh node is not stale
        assert!(!node.is_stale(3600));

        // Old node is stale
        node.last_seen = now() - 7200; // 2 hours ago
        assert!(node.is_stale(3600)); // Max age 1 hour
    }

    #[test]
    fn test_with_adapters() {
        let node_id = ProtocolNodeId::from_bytes([1u8; 32]);
        let adapters = vec![AdapterInfo {
            adapter_type: AdapterType::Ethernet,
            address: "192.168.1.1:4001".to_string(),
            active: true,
        }];

        let node = NodeInfo::with_adapters(node_id, adapters.clone());
        assert_eq!(node.adapters.len(), 1);
        assert!(node.get_best_adapter().is_some());
    }
}
