//! MyriadMesh Routing Module
//!
//! Message routing and priority queue system
//!
//! This crate provides:
//! - 5-level priority queue for message scheduling
//! - Message deduplication using LRU cache
//! - Rate limiting with token bucket algorithm
//! - Message router with direct/multi-hop/store-and-forward
//! - Content tag filtering for relay nodes

pub mod dedup_cache;
pub mod priority_queue;
pub mod rate_limiter;
pub mod router;

// Re-export main types
pub use dedup_cache::DedupCache;
pub use priority_queue::{PriorityQueue, QueueError, QueueStats, QueuedMessage, NUM_QUEUES};
pub use rate_limiter::{RateLimiter, RateLimiterConfig};
pub use router::{DropReason, MessageRouter, RouteDecision, RouterConfig, RoutingError};

#[cfg(test)]
mod tests {
    #[test]
    fn test_exports() {
        // Just verify constants are exported
        assert_eq!(super::NUM_QUEUES, 5);
    }
}
