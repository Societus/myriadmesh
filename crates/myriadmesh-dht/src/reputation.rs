//! Node reputation system for Sybil resistance

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Get current Unix timestamp
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Node reputation tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeReputation {
    /// Successful message relays
    pub successful_relays: u64,

    /// Failed relay attempts
    pub failed_relays: u64,

    /// Total uptime (seconds)
    pub uptime_seconds: u64,

    /// First seen timestamp
    pub first_seen: u64,

    /// Last updated timestamp
    pub last_updated: u64,

    /// Cached reputation score (0.0 - 1.0)
    score: f64,
}

impl NodeReputation {
    /// Minimum reputation to be considered trustworthy
    pub const MIN_REPUTATION: f64 = 0.3;

    /// Good reputation for relay selection
    pub const GOOD_REPUTATION: f64 = 0.7;

    /// Create new reputation for a node
    pub fn new() -> Self {
        let now = now();
        NodeReputation {
            successful_relays: 0,
            failed_relays: 0,
            uptime_seconds: 0,
            first_seen: now,
            last_updated: now,
            score: 0.5, // Start with neutral reputation
        }
    }

    /// Record successful relay
    pub fn record_success(&mut self) {
        self.successful_relays += 1;
        self.last_updated = now();
        self.update_score();
    }

    /// Record failed relay
    pub fn record_failure(&mut self) {
        self.failed_relays += 1;
        self.last_updated = now();
        self.update_score();
    }

    /// Update uptime
    pub fn update_uptime(&mut self, uptime: Duration) {
        self.uptime_seconds = uptime.as_secs();
        self.last_updated = now();
        self.update_score();
    }

    /// Calculate reputation score (0.0 - 1.0)
    fn update_score(&mut self) {
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
        let age_seconds = now().saturating_sub(self.first_seen);
        let age_score = (age_seconds as f64 / (30.0 * 86400.0)).min(1.0);

        // Weighted average
        self.score = reliability * 0.5 + uptime_score * 0.3 + age_score * 0.2;
    }

    /// Get current reputation score
    pub fn score(&self) -> f64 {
        self.score
    }

    /// Check if node is trustworthy
    pub fn is_trustworthy(&self) -> bool {
        self.score >= Self::MIN_REPUTATION
    }

    /// Check if node has good reputation for relay
    pub fn is_good_relay(&self) -> bool {
        self.score >= Self::GOOD_REPUTATION
    }
}

impl Default for NodeReputation {
    fn default() -> Self {
        Self::new()
    }
}

/// Reputation manager for tracking multiple nodes
pub struct ReputationManager {
    // Could add persistence layer here in future
}

impl ReputationManager {
    pub fn new() -> Self {
        ReputationManager {}
    }
}

impl Default for ReputationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_node_neutral_reputation() {
        let rep = NodeReputation::new();
        assert_eq!(rep.score(), 0.5);
        assert!(rep.is_trustworthy());
        assert!(!rep.is_good_relay());
    }

    #[test]
    fn test_successful_relays_increase_score() {
        let mut rep = NodeReputation::new();

        // Add some uptime and age to allow score to exceed 0.5
        rep.update_uptime(Duration::from_secs(7 * 86400)); // 7 days uptime

        for _ in 0..100 {
            rep.record_success();
        }

        // With 100% reliability (0.5) + some uptime (0.3 * 7/90) + minimal age (0.2 * ~0)
        // Score should be > 0.5
        assert!(rep.score() > 0.5);
        assert!(rep.is_trustworthy());
    }

    #[test]
    fn test_failed_relays_decrease_score() {
        let mut rep = NodeReputation::new();

        for _ in 0..100 {
            rep.record_failure();
        }

        assert!(rep.score() < 0.5);
        assert!(!rep.is_trustworthy());
    }

    #[test]
    fn test_mixed_relays() {
        let mut rep = NodeReputation::new();

        // Add uptime to allow score contribution beyond reliability
        rep.update_uptime(Duration::from_secs(14 * 86400)); // 14 days uptime

        // 80% success rate
        for _ in 0..80 {
            rep.record_success();
        }
        for _ in 0..20 {
            rep.record_failure();
        }

        // With 80% reliability (0.4) + 14 days uptime (0.3 * 14/90 ≈ 0.047) + minimal age
        // Score should be > 0.4 and trustworthy (>= 0.3)
        assert!(rep.score() > 0.4);
        assert!(rep.is_trustworthy());
    }
}
