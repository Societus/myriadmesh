//! Node information and reputation system

use chrono::{DateTime, Utc};
use myriadmesh_protocol::{types::NodeId, types::AdapterType};
use serde::{Deserialize, Serialize};

/// Information about a node in the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node identifier
    pub node_id: NodeId,

    /// Available network adapters
    pub adapters: Vec<AdapterInfo>,

    /// Last successful communication timestamp
    pub last_seen: DateTime<Utc>,

    /// Round-trip time in milliseconds
    pub rtt_ms: f64,

    /// Consecutive communication failures
    pub failures: u32,

    /// Reputation information
    pub reputation: NodeReputation,
}

/// Network adapter information for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    /// Type of adapter
    pub adapter_type: AdapterType,

    /// Address on this adapter (adapter-specific format)
    pub address: String,

    /// Whether this adapter is currently active
    pub active: bool,
}

/// Node reputation tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeReputation {
    /// Successful message relays
    pub successful_relays: u64,

    /// Failed relay attempts
    pub failed_relays: u64,

    /// Total uptime in seconds
    pub uptime_seconds: u64,

    /// When this node was first seen
    pub first_seen: DateTime<Utc>,

    /// Computed reputation score (0.0 - 1.0)
    pub score: f64,
}

impl NodeReputation {
    /// Minimum reputation to be considered trustworthy
    pub const MIN_REPUTATION: f64 = 0.3;

    /// Good reputation threshold for relay selection
    pub const GOOD_REPUTATION: f64 = 0.7;

    /// Create a new reputation for a newly discovered node
    pub fn new() -> Self {
        Self {
            successful_relays: 0,
            failed_relays: 0,
            uptime_seconds: 0,
            first_seen: Utc::now(),
            score: 0.5, // Neutral starting score
        }
    }

    /// Calculate reputation score (0.0 - 1.0)
    pub fn calculate_score(&mut self) {
        // Relay reliability (50% weight)
        let total_relays = self.successful_relays + self.failed_relays;
        let reliability = if total_relays > 0 {
            self.successful_relays as f64 / total_relays as f64
        } else {
            0.5 // Neutral for new nodes
        };

        // Uptime score (30% weight)
        // Max out at 90 days
        let uptime_score = (self.uptime_seconds as f64 / (90.0 * 86400.0)).min(1.0);

        // Age score (20% weight)
        // Older nodes (more history) are slightly more trusted
        let age_seconds = (Utc::now() - self.first_seen).num_seconds() as f64;
        let age_score = (age_seconds / (30.0 * 86400.0)).min(1.0);

        // Weighted average
        self.score = reliability * 0.5 + uptime_score * 0.3 + age_score * 0.2;
    }

    /// Record a successful relay
    pub fn record_success(&mut self) {
        self.successful_relays += 1;
        self.calculate_score();
    }

    /// Record a failed relay
    pub fn record_failure(&mut self) {
        self.failed_relays += 1;
        self.calculate_score();
    }

    /// Check if reputation is above minimum threshold
    pub fn is_trustworthy(&self) -> bool {
        self.score >= Self::MIN_REPUTATION
    }

    /// Check if reputation is good enough for relay
    pub fn is_good_relay(&self) -> bool {
        self.score >= Self::GOOD_REPUTATION
    }
}

impl Default for NodeReputation {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeInfo {
    /// Create new node info
    pub fn new(node_id: NodeId, adapters: Vec<AdapterInfo>) -> Self {
        Self {
            node_id,
            adapters,
            last_seen: Utc::now(),
            rtt_ms: 0.0,
            failures: 0,
            reputation: NodeReputation::new(),
        }
    }

    /// Update last seen timestamp
    pub fn mark_seen(&mut self) {
        self.last_seen = Utc::now();
        self.failures = 0;
    }

    /// Record a communication failure
    pub fn mark_failure(&mut self) {
        self.failures += 1;
    }

    /// Check if node is considered alive based on last seen time
    pub fn is_alive(&self, timeout_seconds: i64) -> bool {
        let now = Utc::now();
        (now - self.last_seen).num_seconds() < timeout_seconds
    }

    /// Update RTT
    pub fn update_rtt(&mut self, rtt_ms: f64) {
        // Exponential moving average
        const ALPHA: f64 = 0.3;
        if self.rtt_ms > 0.0 {
            self.rtt_ms = ALPHA * rtt_ms + (1.0 - ALPHA) * self.rtt_ms;
        } else {
            self.rtt_ms = rtt_ms;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reputation_new_node() {
        let rep = NodeReputation::new();
        assert_eq!(rep.score, 0.5);
        assert!(!rep.is_good_relay());
        assert!(rep.is_trustworthy());
    }

    #[test]
    fn test_reputation_scoring() {
        let mut rep = NodeReputation::new();

        // Record successes
        for _ in 0..10 {
            rep.record_success();
        }

        // Should have high reliability component
        assert!(rep.score > 0.5);

        // Record some failures
        for _ in 0..5 {
            rep.record_failure();
        }

        // Score should decrease but still be reasonable
        assert!(rep.score < 0.8);
        assert!(rep.score > 0.4);
    }

    #[test]
    fn test_node_info_alive() {
        let adapters = vec![AdapterInfo {
            adapter_type: AdapterType::Ethernet,
            address: "192.168.1.100:4001".to_string(),
            active: true,
        }];

        let node = NodeInfo::new(NodeId::from_bytes([1u8; 32]), adapters);

        // Should be alive immediately
        assert!(node.is_alive(300));

        // Should not be alive after timeout
        // (can't test without time manipulation, but structure is correct)
    }

    #[test]
    fn test_node_info_rtt_ewma() {
        let adapters = vec![];
        let mut node = NodeInfo::new(NodeId::from_bytes([1u8; 32]), adapters);

        // First measurement
        node.update_rtt(100.0);
        assert_eq!(node.rtt_ms, 100.0);

        // Second measurement should be averaged
        node.update_rtt(200.0);
        assert!(node.rtt_ms > 100.0);
        assert!(node.rtt_ms < 200.0);
    }
}
