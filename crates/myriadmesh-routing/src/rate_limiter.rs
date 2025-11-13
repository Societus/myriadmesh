//! Rate limiting for message routing

use myriadmesh_protocol::NodeId;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Rate limiter for message routing
#[derive(Debug)]
pub struct RateLimiter {
    /// Messages per minute per node
    per_node_limit: u32,

    /// Total messages per minute globally
    global_limit: u32,

    /// Per-node counters (node_id -> (count, window_start))
    node_counters: HashMap<NodeId, (u32, Instant)>,

    /// Global counter (count, window_start)
    global_counter: (u32, Instant),

    /// Window duration
    window: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(per_node_limit: u32, global_limit: u32) -> Self {
        RateLimiter {
            per_node_limit,
            global_limit,
            node_counters: HashMap::new(),
            global_counter: (0, Instant::now()),
            window: Duration::from_secs(60), // 1 minute window
        }
    }

    /// Check if a message from a node should be accepted
    pub fn check_rate(&mut self, node_id: &NodeId) -> Result<(), RateLimitError> {
        let now = Instant::now();

        // Check per-node limit
        let entry = self.node_counters.entry(*node_id).or_insert((0, now));

        // Reset counter if window expired
        if now.duration_since(entry.1) >= self.window {
            entry.0 = 0;
            entry.1 = now;
        }

        // Increment and check
        entry.0 += 1;
        if entry.0 > self.per_node_limit {
            return Err(RateLimitError::PerNodeLimitExceeded {
                node_id: *node_id,
                limit: self.per_node_limit,
                current: entry.0,
            });
        }

        // Check global limit
        if now.duration_since(self.global_counter.1) >= self.window {
            self.global_counter = (0, now);
        }

        self.global_counter.0 += 1;
        if self.global_counter.0 > self.global_limit {
            return Err(RateLimitError::GlobalLimitExceeded {
                limit: self.global_limit,
                current: self.global_counter.0,
            });
        }

        Ok(())
    }

    /// Get current per-node rate
    pub fn get_node_rate(&self, node_id: &NodeId) -> u32 {
        self.node_counters
            .get(node_id)
            .map(|(count, start)| {
                if Instant::now().duration_since(*start) < self.window {
                    *count
                } else {
                    0
                }
            })
            .unwrap_or(0)
    }

    /// Get current global rate
    pub fn get_global_rate(&self) -> u32 {
        if Instant::now().duration_since(self.global_counter.1) < self.window {
            self.global_counter.0
        } else {
            0
        }
    }

    /// Clear all rate limit counters
    pub fn clear(&mut self) {
        self.node_counters.clear();
        self.global_counter = (0, Instant::now());
    }

    /// Cleanup expired node counters
    pub fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.node_counters
            .retain(|_, (_, start)| now.duration_since(*start) < self.window);
    }
}

/// Rate limit error
#[derive(Debug, Clone)]
pub enum RateLimitError {
    /// Per-node limit exceeded
    PerNodeLimitExceeded {
        node_id: NodeId,
        limit: u32,
        current: u32,
    },

    /// Global limit exceeded
    GlobalLimitExceeded { limit: u32, current: u32 },
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateLimitError::PerNodeLimitExceeded {
                node_id,
                limit,
                current,
            } => write!(
                f,
                "Rate limit exceeded for node {}: {}/{} messages/min",
                node_id, current, limit
            ),
            RateLimitError::GlobalLimitExceeded { limit, current } => write!(
                f,
                "Global rate limit exceeded: {}/{} messages/min",
                current, limit
            ),
        }
    }
}

impl std::error::Error for RateLimitError {}

#[cfg(test)]
mod tests {
    use super::*;
    use myriadmesh_protocol::types::NODE_ID_SIZE;

    fn create_test_node_id(byte: u8) -> NodeId {
        NodeId::from_bytes([byte; NODE_ID_SIZE])
    }

    #[test]
    fn test_per_node_limit() {
        let mut limiter = RateLimiter::new(5, 100);
        let node_id = create_test_node_id(1);

        // Should accept first 5 messages
        for _ in 0..5 {
            assert!(limiter.check_rate(&node_id).is_ok());
        }

        // 6th message should fail
        assert!(limiter.check_rate(&node_id).is_err());
    }

    #[test]
    fn test_global_limit() {
        let mut limiter = RateLimiter::new(100, 10);

        let node1 = create_test_node_id(1);
        let node2 = create_test_node_id(2);

        // Use up global limit with multiple nodes
        for _ in 0..5 {
            assert!(limiter.check_rate(&node1).is_ok());
        }

        for _ in 0..5 {
            assert!(limiter.check_rate(&node2).is_ok());
        }

        // 11th message should fail (global limit)
        assert!(limiter.check_rate(&node1).is_err());
    }

    #[test]
    fn test_multiple_nodes() {
        let mut limiter = RateLimiter::new(5, 100);

        let node1 = create_test_node_id(1);
        let node2 = create_test_node_id(2);

        // Each node should have independent limits
        for _ in 0..5 {
            assert!(limiter.check_rate(&node1).is_ok());
        }

        for _ in 0..5 {
            assert!(limiter.check_rate(&node2).is_ok());
        }

        // Both should fail on next message
        assert!(limiter.check_rate(&node1).is_err());
        assert!(limiter.check_rate(&node2).is_err());
    }

    #[test]
    fn test_get_rates() {
        let mut limiter = RateLimiter::new(10, 100);
        let node_id = create_test_node_id(1);

        assert_eq!(limiter.get_node_rate(&node_id), 0);
        assert_eq!(limiter.get_global_rate(), 0);

        limiter.check_rate(&node_id).unwrap();
        limiter.check_rate(&node_id).unwrap();

        assert_eq!(limiter.get_node_rate(&node_id), 2);
        assert_eq!(limiter.get_global_rate(), 2);
    }

    #[test]
    fn test_clear() {
        let mut limiter = RateLimiter::new(5, 100);
        let node_id = create_test_node_id(1);

        for _ in 0..5 {
            limiter.check_rate(&node_id).unwrap();
        }

        assert!(limiter.check_rate(&node_id).is_err());

        limiter.clear();

        // Should work again after clear
        assert!(limiter.check_rate(&node_id).is_ok());
    }

    #[test]
    fn test_cleanup_expired() {
        let mut limiter = RateLimiter::new(10, 100);

        let node1 = create_test_node_id(1);
        let node2 = create_test_node_id(2);

        limiter.check_rate(&node1).unwrap();
        limiter.check_rate(&node2).unwrap();

        assert_eq!(limiter.node_counters.len(), 2);

        // Cleanup shouldn't remove recent entries
        limiter.cleanup_expired();
        assert_eq!(limiter.node_counters.len(), 2);
    }
}
