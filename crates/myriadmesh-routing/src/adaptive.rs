//! Adaptive Routing - Dynamic path selection based on network conditions
//!
//! Implements adaptive routing algorithms that continuously monitor network
//! performance and adjust routing decisions in real-time to optimize for
//! latency, reliability, bandwidth, and other metrics.

use myriadmesh_protocol::NodeId;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Link performance metrics
#[derive(Debug, Clone)]
pub struct LinkMetrics {
    /// Average latency (milliseconds)
    pub latency_ms: f64,
    /// Packet loss rate (0.0-1.0)
    pub loss_rate: f64,
    /// Available bandwidth (bits per second)
    pub bandwidth_bps: u64,
    /// Link utilization (0.0-1.0)
    pub utilization: f64,
    /// Jitter (millisecond standard deviation)
    pub jitter_ms: f64,
    /// Last update time
    pub last_updated: Instant,
    /// Number of samples
    pub sample_count: u64,
}

impl LinkMetrics {
    /// Create new link metrics with defaults
    pub fn new() -> Self {
        Self {
            latency_ms: 0.0,
            loss_rate: 0.0,
            bandwidth_bps: 0,
            utilization: 0.0,
            jitter_ms: 0.0,
            last_updated: Instant::now(),
            sample_count: 0,
        }
    }

    /// Update metrics with new measurement
    pub fn update(&mut self, latency_ms: f64, loss: bool, bandwidth_bps: u64, utilization: f64) {
        let alpha = 0.125; // Exponential moving average factor (1/8)

        // Update latency with EMA
        if self.sample_count == 0 {
            self.latency_ms = latency_ms;
        } else {
            self.latency_ms = alpha * latency_ms + (1.0 - alpha) * self.latency_ms;
        }

        // Update loss rate with EMA
        let loss_value = if loss { 1.0 } else { 0.0 };
        if self.sample_count == 0 {
            self.loss_rate = loss_value;
        } else {
            self.loss_rate = alpha * loss_value + (1.0 - alpha) * self.loss_rate;
        }

        // Update bandwidth (use most recent)
        self.bandwidth_bps = bandwidth_bps;

        // Update utilization with EMA
        if self.sample_count == 0 {
            self.utilization = utilization;
        } else {
            self.utilization = alpha * utilization + (1.0 - alpha) * self.utilization;
        }

        // Update jitter (simplified)
        if self.sample_count > 0 {
            let delta = (latency_ms - self.latency_ms).abs();
            self.jitter_ms = alpha * delta + (1.0 - alpha) * self.jitter_ms;
        }

        self.last_updated = Instant::now();
        self.sample_count += 1;
    }

    /// Calculate link cost (lower is better)
    pub fn calculate_cost(&self, weights: &CostWeights) -> f64 {
        weights.latency * self.latency_ms
            + weights.loss * self.loss_rate * 1000.0 // Scale up loss for visibility
            + weights.jitter * self.jitter_ms
            + weights.utilization * self.utilization * 100.0 // Scale up utilization
    }

    /// Check if metrics are stale
    pub fn is_stale(&self, ttl: Duration) -> bool {
        self.last_updated.elapsed() > ttl
    }

    /// Get link quality score (0.0-1.0, higher is better)
    pub fn quality_score(&self) -> f64 {
        // Combine multiple factors into a quality score
        let latency_score = if self.latency_ms > 0.0 {
            (100.0 / (self.latency_ms + 10.0)).min(1.0)
        } else {
            1.0
        };

        let loss_score = 1.0 - self.loss_rate;
        let jitter_score = if self.jitter_ms > 0.0 {
            (10.0 / (self.jitter_ms + 1.0)).min(1.0)
        } else {
            1.0
        };

        let util_score = 1.0 - self.utilization;

        // Weighted average
        (latency_score * 0.3 + loss_score * 0.4 + jitter_score * 0.2 + util_score * 0.1).min(1.0)
    }
}

impl Default for LinkMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Cost calculation weights
#[derive(Debug, Clone)]
pub struct CostWeights {
    pub latency: f64,
    pub loss: f64,
    pub jitter: f64,
    pub utilization: f64,
}

impl Default for CostWeights {
    fn default() -> Self {
        Self {
            latency: 1.0,
            loss: 10.0,
            jitter: 0.5,
            utilization: 2.0,
        }
    }
}

/// Routing policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingPolicy {
    /// Minimize latency
    LowLatency,
    /// Maximize reliability (minimize loss)
    HighReliability,
    /// Balance latency and reliability
    Balanced,
    /// Avoid congested links
    LoadBalanced,
    /// Custom weights
    Custom,
}

impl RoutingPolicy {
    /// Get cost weights for this policy
    pub fn weights(&self) -> CostWeights {
        match self {
            RoutingPolicy::LowLatency => CostWeights {
                latency: 10.0,
                loss: 1.0,
                jitter: 5.0,
                utilization: 0.5,
            },
            RoutingPolicy::HighReliability => CostWeights {
                latency: 0.5,
                loss: 20.0,
                jitter: 1.0,
                utilization: 1.0,
            },
            RoutingPolicy::Balanced => CostWeights::default(),
            RoutingPolicy::LoadBalanced => CostWeights {
                latency: 1.0,
                loss: 5.0,
                jitter: 0.5,
                utilization: 15.0,
            },
            RoutingPolicy::Custom => CostWeights::default(),
        }
    }
}

/// Adaptive routing table
pub struct AdaptiveRoutingTable {
    /// Link metrics for each node pair
    metrics: HashMap<(NodeId, NodeId), LinkMetrics>,
    /// Routing policy
    policy: RoutingPolicy,
    /// Custom cost weights (used when policy is Custom)
    custom_weights: Option<CostWeights>,
    /// Metrics time-to-live
    metrics_ttl: Duration,
}

impl AdaptiveRoutingTable {
    /// Create a new adaptive routing table
    pub fn new(policy: RoutingPolicy, metrics_ttl: Duration) -> Self {
        Self {
            metrics: HashMap::new(),
            policy,
            custom_weights: None,
            metrics_ttl,
        }
    }

    /// Set custom cost weights
    pub fn set_custom_weights(&mut self, weights: CostWeights) {
        self.custom_weights = Some(weights);
    }

    /// Update link metrics
    pub fn update_link(
        &mut self,
        from: NodeId,
        to: NodeId,
        latency_ms: f64,
        loss: bool,
        bandwidth_bps: u64,
        utilization: f64,
    ) {
        let metrics = self.metrics.entry((from, to)).or_default();
        metrics.update(latency_ms, loss, bandwidth_bps, utilization);
    }

    /// Get link metrics
    pub fn get_link_metrics(&self, from: &NodeId, to: &NodeId) -> Option<&LinkMetrics> {
        self.metrics.get(&(*from, *to))
    }

    /// Calculate link cost
    pub fn link_cost(&self, from: &NodeId, to: &NodeId) -> Option<f64> {
        self.get_link_metrics(from, to).map(|metrics| {
            let weights = match &self.custom_weights {
                Some(w) if self.policy == RoutingPolicy::Custom => w.clone(),
                _ => self.policy.weights(),
            };
            metrics.calculate_cost(&weights)
        })
    }

    /// Select best next hop from neighbors
    pub fn select_best_neighbor(
        &self,
        current: &NodeId,
        neighbors: &[NodeId],
    ) -> Option<(NodeId, f64)> {
        let mut best: Option<(NodeId, f64)> = None;

        for neighbor in neighbors {
            if let Some(cost) = self.link_cost(current, neighbor) {
                match &best {
                    None => best = Some((*neighbor, cost)),
                    Some((_, best_cost)) => {
                        if cost < *best_cost {
                            best = Some((*neighbor, cost));
                        }
                    }
                }
            }
        }

        best
    }

    /// Cleanup stale metrics
    pub fn cleanup_stale(&mut self) {
        self.metrics
            .retain(|_, metrics| !metrics.is_stale(self.metrics_ttl));
    }

    /// Get total number of tracked links
    pub fn link_count(&self) -> usize {
        self.metrics.len()
    }

    /// Change routing policy
    pub fn set_policy(&mut self, policy: RoutingPolicy) {
        self.policy = policy;
    }

    /// Get routing statistics
    pub fn stats(&self) -> AdaptiveRoutingStats {
        let total_links = self.metrics.len();

        let avg_latency = if total_links > 0 {
            self.metrics.values().map(|m| m.latency_ms).sum::<f64>() / total_links as f64
        } else {
            0.0
        };

        let avg_loss_rate = if total_links > 0 {
            self.metrics.values().map(|m| m.loss_rate).sum::<f64>() / total_links as f64
        } else {
            0.0
        };

        let avg_quality = if total_links > 0 {
            self.metrics
                .values()
                .map(|m| m.quality_score())
                .sum::<f64>()
                / total_links as f64
        } else {
            0.0
        };

        AdaptiveRoutingStats {
            total_links,
            avg_latency_ms: avg_latency,
            avg_loss_rate,
            avg_quality_score: avg_quality,
        }
    }
}

/// Adaptive routing statistics
#[derive(Debug, Clone)]
pub struct AdaptiveRoutingStats {
    pub total_links: usize,
    pub avg_latency_ms: f64,
    pub avg_loss_rate: f64,
    pub avg_quality_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_metrics_update() {
        let mut metrics = LinkMetrics::new();

        // First update
        metrics.update(50.0, false, 1_000_000, 0.5);
        assert_eq!(metrics.latency_ms, 50.0);
        assert_eq!(metrics.loss_rate, 0.0);
        assert_eq!(metrics.sample_count, 1);

        // Second update (should use EMA)
        metrics.update(100.0, true, 1_000_000, 0.6);
        assert!(metrics.latency_ms > 50.0 && metrics.latency_ms < 100.0);
        assert!(metrics.loss_rate > 0.0 && metrics.loss_rate < 1.0);
        assert_eq!(metrics.sample_count, 2);
    }

    #[test]
    fn test_link_quality_score() {
        let mut metrics = LinkMetrics::new();
        metrics.update(10.0, false, 1_000_000, 0.1);

        let quality = metrics.quality_score();
        assert!(quality > 0.0 && quality <= 1.0);

        // Higher latency and loss should reduce quality
        let mut bad_metrics = LinkMetrics::new();
        bad_metrics.update(500.0, true, 1_000_000, 0.9);

        let bad_quality = bad_metrics.quality_score();
        assert!(bad_quality < quality);
    }

    #[test]
    fn test_routing_policy_weights() {
        let low_latency = RoutingPolicy::LowLatency.weights();
        let high_reliability = RoutingPolicy::HighReliability.weights();

        // Low latency should weight latency more heavily
        assert!(low_latency.latency > high_reliability.latency);
        // High reliability should weight loss more heavily
        assert!(high_reliability.loss > low_latency.loss);
    }

    #[test]
    fn test_adaptive_routing_table() {
        let mut table = AdaptiveRoutingTable::new(RoutingPolicy::Balanced, Duration::from_secs(60));

        let node1 = NodeId::from_bytes([0u8; 64]);
        let node2 = NodeId::from_bytes([1u8; 64]);

        table.update_link(node1, node2, 50.0, false, 1_000_000, 0.3);

        let cost = table.link_cost(&node1, &node2);
        assert!(cost.is_some());
        assert!(cost.unwrap() > 0.0);
    }

    fn create_test_node_id(value: u8) -> NodeId {
        let mut bytes = [0u8; 64];
        bytes[0] = value;
        NodeId::from_bytes(bytes)
    }

    #[test]
    fn test_select_best_neighbor() {
        let mut table =
            AdaptiveRoutingTable::new(RoutingPolicy::LowLatency, Duration::from_secs(60));

        let current = create_test_node_id(1);
        let neighbor1 = create_test_node_id(2);
        let neighbor2 = create_test_node_id(3);

        // Neighbor 1 has lower latency
        table.update_link(current, neighbor1, 10.0, false, 1_000_000, 0.2);
        // Neighbor 2 has higher latency
        table.update_link(current, neighbor2, 50.0, false, 1_000_000, 0.3);

        let neighbors = vec![neighbor1, neighbor2];
        let best = table.select_best_neighbor(&current, &neighbors);

        assert!(best.is_some());
        let (best_neighbor, _cost) = best.unwrap();
        // Should select neighbor1 (lower latency)
        assert_eq!(best_neighbor, neighbor1);
    }
}
