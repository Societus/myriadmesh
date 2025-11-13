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

/// Node reputation tracking with Byzantine resistance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeReputation {
    /// Successful message relays
    pub successful_relays: u64,

    /// Failed relay attempts
    pub failed_relays: u64,

    /// Total uptime (seconds) - verified by observation, not self-reported
    pub uptime_seconds: u64,

    /// First seen timestamp
    pub first_seen: u64,

    /// Last updated timestamp
    pub last_updated: u64,

    /// Last activity timestamp (for decay calculation)
    pub last_activity: u64,

    /// Cached reputation score (0.0 - 1.0)
    score: f64,

    /// SECURITY C7: Penalty counter for suspicious behavior
    /// Incremented for: rapid reputation growth, contradictory reports, etc.
    pub penalty_count: u32,

    /// Recent activity rate (messages per hour) for growth rate analysis
    recent_activity_rate: f64,
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
            last_activity: now,
            score: 0.2, // SECURITY C7: Start with low reputation (trust must be earned)
            penalty_count: 0,
            recent_activity_rate: 0.0,
        }
    }

    /// Record successful relay
    pub fn record_success(&mut self) {
        self.successful_relays += 1;
        let current_time = now();

        // SECURITY C7: Detect suspiciously rapid reputation growth
        self.check_activity_rate(current_time);

        self.last_updated = current_time;
        self.last_activity = current_time;
        self.update_score();
    }

    /// Record failed relay
    pub fn record_failure(&mut self) {
        self.failed_relays += 1;
        let current_time = now();

        self.last_updated = current_time;
        self.last_activity = current_time;
        self.update_score();
    }

    /// SECURITY C7: Check for suspiciously rapid activity (Sybil indicator)
    fn check_activity_rate(&mut self, current_time: u64) {
        let time_since_first_seen = current_time.saturating_sub(self.first_seen);

        if time_since_first_seen > 0 {
            let total_activity = self.successful_relays + self.failed_relays;
            let activity_per_hour = (total_activity as f64 * 3600.0) / time_since_first_seen as f64;

            // Suspicious if > 1000 messages per hour for new nodes (< 24 hours old)
            if time_since_first_seen < 86400 && activity_per_hour > 1000.0 {
                self.penalty_count += 1;
            }

            // Suspicious if sudden spike in activity rate
            if self.recent_activity_rate > 0.0
                && activity_per_hour > self.recent_activity_rate * 10.0
            {
                self.penalty_count += 1;
            }

            self.recent_activity_rate = activity_per_hour;
        }
    }

    /// Update uptime (verified by observation, not self-reported)
    ///
    /// SECURITY C7: Uptime is calculated based on observed activity,
    /// not self-reported values. This prevents attackers from claiming
    /// false uptime to boost reputation.
    pub fn update_uptime(&mut self, uptime: Duration) {
        let current_time = now();
        let observed_age = current_time.saturating_sub(self.first_seen);

        // SECURITY C7: Cap uptime to observed age (prevent fake uptime claims)
        let claimed_uptime = uptime.as_secs();
        self.uptime_seconds = claimed_uptime.min(observed_age);

        // Penalize if claim exceeds observation significantly
        if claimed_uptime > observed_age * 2 {
            self.penalty_count += 1;
        }

        self.last_updated = current_time;
        self.update_score();
    }

    /// Calculate reputation score (0.0 - 1.0) with Byzantine resistance
    ///
    /// SECURITY C7: Implements multiple defenses against reputation manipulation:
    /// - Time decay for inactive nodes
    /// - Penalty for suspicious behavior
    /// - Minimum activity threshold
    /// - Slower reputation growth for new nodes
    fn update_score(&mut self) {
        let current_time = now();

        // SECURITY C7: Apply time decay for inactivity
        let time_since_activity = current_time.saturating_sub(self.last_activity);
        let decay_factor = if time_since_activity > 86400 {
            // Decay by 10% per day of inactivity
            let days_inactive = time_since_activity as f64 / 86400.0;
            0.9_f64.powf(days_inactive).max(0.1)
        } else {
            1.0
        };

        // Relay reliability (50% weight)
        let total_relays = self.successful_relays + self.failed_relays;
        let reliability = if total_relays > 0 {
            let success_rate = self.successful_relays as f64 / total_relays as f64;

            // SECURITY C7: Require minimum activity before high reputation
            // New nodes with few relays get capped reputation
            if total_relays < 100 {
                success_rate * (total_relays as f64 / 100.0)
            } else {
                success_rate
            }
        } else {
            0.2 // Low reputation for nodes with no activity
        };

        // Uptime score (25% weight)
        // Max out at 90 days
        let uptime_score = (self.uptime_seconds as f64 / (90.0 * 86400.0)).min(1.0);

        // Age score (15% weight)
        // Older nodes (more history) are slightly more trusted
        let age_seconds = current_time.saturating_sub(self.first_seen);
        let age_score = (age_seconds as f64 / (30.0 * 86400.0)).min(1.0);

        // SECURITY C7: Penalty factor - multiplicative reduction
        // Each penalty reduces score by 10%, multiplicative
        let penalty_factor = if self.penalty_count == 0 {
            1.0
        } else {
            0.9_f64.powf(self.penalty_count as f64).max(0.1)
        };

        // Weighted average: reliability(50%) + uptime(25%) + age(15%) + base(10%)
        let base_score = reliability * 0.5 + uptime_score * 0.25 + age_score * 0.15 + 0.1;

        // Apply decay and penalties multiplicatively
        self.score = (base_score * decay_factor * penalty_factor).clamp(0.0, 1.0);
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

    /// SECURITY C7: Apply penalty for Byzantine behavior
    ///
    /// This should be called when:
    /// - Node reports contradictory information
    /// - Node fails to relay when expected
    /// - Node exhibits Sybil-like behavior
    /// - Other nodes consistently report failures for this node
    pub fn apply_penalty(&mut self, reason: &str) {
        self.penalty_count += 1;
        self.last_updated = now();
        self.update_score();

        // Log penalty for debugging (in production, use proper logging)
        #[cfg(debug_assertions)]
        eprintln!(
            "Penalty applied to node (count={}): {}",
            self.penalty_count, reason
        );
    }

    /// SECURITY C7: Get penalty count for monitoring
    pub fn get_penalty_count(&self) -> u32 {
        self.penalty_count
    }

    /// Force reputation recalculation (for time-based decay)
    pub fn recalculate(&mut self) {
        self.update_score();
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
        // SECURITY C7: New nodes start with low reputation (must earn trust)
        let rep = NodeReputation::new();
        assert_eq!(rep.score(), 0.2);
        assert!(!rep.is_trustworthy()); // Below MIN_REPUTATION (0.3)
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

#[test]
fn test_reputation_growth_with_activity() {
    // SECURITY C7: Reputation grows with successful relays
    let mut rep = NodeReputation::new();

    // Simulate 100 successful relays
    for _ in 0..100 {
        rep.record_success();
    }

    // Should have good reputation after 100 successes
    assert!(rep.score() > 0.5, "Score should improve with activity");
    assert!(rep.is_trustworthy());
}

#[test]
fn test_fake_uptime_penalty() {
    // SECURITY C7: Fake uptime claims are penalized
    let mut rep = NodeReputation::new();

    // Wait 1 second
    std::thread::sleep(Duration::from_secs(1));

    // Claim 1 year of uptime (clearly fake)
    rep.update_uptime(Duration::from_secs(365 * 86400));

    // Should have penalty
    assert!(
        rep.get_penalty_count() > 0,
        "Fake uptime should be penalized"
    );

    // Uptime should be capped to observed age
    assert!(
        rep.uptime_seconds < 10,
        "Uptime should be capped to observed age"
    );
}

#[test]
fn test_rapid_activity_penalty() {
    // SECURITY C7: Suspiciously rapid activity is penalized
    let mut rep = NodeReputation::new();

    // Wait 1 second to allow time measurement
    std::thread::sleep(Duration::from_secs(1));

    // Simulate 2000 relays in ~1 second (suspicious: 7,200,000/hour)
    for _ in 0..2000 {
        rep.record_success();
    }

    // Should have penalties for suspicious activity rate
    assert!(
        rep.get_penalty_count() > 0,
        "Rapid activity should trigger penalty"
    );
}

#[test]
fn test_minimum_activity_threshold() {
    // SECURITY C7: Need minimum activity before high reputation
    let mut rep = NodeReputation::new();

    // Only 10 successful relays (below threshold of 100)
    for _ in 0..10 {
        rep.record_success();
    }

    // Perfect success rate, but low activity
    // Score should be capped
    assert!(rep.score() < 0.3, "Low activity should cap reputation");
}

#[test]
fn test_reputation_decay() {
    // SECURITY C7: Reputation decays with inactivity
    let mut rep = NodeReputation::new();

    // Build up reputation
    for _ in 0..200 {
        rep.record_success();
    }

    let initial_score = rep.score();
    assert!(initial_score > 0.5);

    // Simulate 3 days of inactivity
    rep.last_activity = now() - (3 * 86400);
    rep.recalculate();

    let decayed_score = rep.score();

    // Score should have decayed
    assert!(
        decayed_score < initial_score,
        "Score should decay with inactivity: {} >= {}",
        decayed_score,
        initial_score
    );
}

#[test]
fn test_manual_penalty_application() {
    // SECURITY C7: Can manually apply penalties for Byzantine behavior
    let mut rep = NodeReputation::new();

    // Build reputation
    for _ in 0..100 {
        rep.record_success();
    }

    let score_before = rep.score();

    // Apply penalty for contradictory report
    rep.apply_penalty("Contradictory routing information");

    let score_after = rep.score();

    // Reputation should decrease
    assert!(
        score_after < score_before,
        "Penalty should decrease reputation"
    );
    assert_eq!(rep.get_penalty_count(), 1);
}

#[test]
fn test_multiple_penalties_compound() {
    // SECURITY C7: Multiple penalties compound
    let mut rep = NodeReputation::new();

    // Build reputation
    for _ in 0..100 {
        rep.record_success();
    }

    let initial_score = rep.score();

    // Apply multiple penalties
    for i in 1..=5 {
        rep.apply_penalty("Suspicious behavior");
        let current_score = rep.score();

        // Each penalty should further decrease score
        assert!(
            current_score < initial_score,
            "Penalty {} should decrease score",
            i
        );
    }

    // With 5 penalties, score should be significantly reduced
    assert!(
        rep.score() < initial_score * 0.6,
        "Multiple penalties should compound"
    );
}

#[test]
fn test_failure_impact() {
    // Test that failures significantly impact reputation
    let mut rep = NodeReputation::new();

    // 50 successes, 50 failures (50% reliability)
    for _ in 0..50 {
        rep.record_success();
    }
    for _ in 0..50 {
        rep.record_failure();
    }

    // With 50% reliability, should not be a good relay
    assert!(
        !rep.is_good_relay(),
        "50% reliability should not be good relay"
    );
}
