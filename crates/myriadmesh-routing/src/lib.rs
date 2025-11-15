//! MyriadMesh Message Routing
//!
//! This module implements the message router for Phase 2 & Phase 4:
//!
//! ## Phase 2 (Basic Routing)
//! - Priority queue system (5 levels)
//! - Direct and multi-hop routing
//! - Store-and-forward for offline nodes
//! - Message deduplication
//! - Content tag filtering (optional)
//!
//! ## Phase 4 (Advanced Routing)
//! - Geographic routing with location-based path selection
//! - Multi-path routing with parallel transmission
//! - Adaptive routing with dynamic path updates
//! - Quality of Service (QoS) with bandwidth reservation

pub mod adaptive;
pub mod deduplication;
pub mod error;
pub mod fragmentation;
pub mod geographic;
pub mod multipath;
pub mod offline_cache;
pub mod priority_queue;
pub mod qos;
pub mod rate_limiter;
pub mod router;

pub use adaptive::{
    AdaptiveRoutingStats, AdaptiveRoutingTable, CostWeights, LinkMetrics, RoutingPolicy,
};
pub use deduplication::DeduplicationCache;
pub use error::{Result, RoutingError};
pub use fragmentation::{
    fragment_frame, FragmentHeader, FragmentReassembler, FragmentationDecision,
    FragmentationReason,
};
pub use geographic::{GeoCoordinates, GeoRoutingTable, NodeLocation};
pub use multipath::{MultiPathRouter, MultiPathStats, MultiPathStrategy, NetworkPath};
pub use offline_cache::{CacheStats, OfflineMessageCache};
pub use priority_queue::{PriorityLevel, PriorityQueue};
pub use qos::{FlowId, FlowStats, QosClass, QosError, QosManager, QosStats};
pub use rate_limiter::RateLimiter;
pub use router::{Router, RouterStats};

/// Maximum cached messages per destination
pub const MAX_CACHED_MESSAGES_PER_DEST: usize = 100;

/// Maximum cached message age (seconds)
pub const MAX_CACHED_MESSAGE_AGE_SECS: u64 = 24 * 3600; // 24 hours

/// Maximum total cached messages
pub const MAX_TOTAL_CACHED_MESSAGES: usize = 10_000;

/// Maximum cached message size
pub const MAX_CACHED_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB

/// Size of message deduplication cache
pub const MESSAGE_DEDUP_CACHE_SIZE: usize = 10_000;

/// Message deduplication TTL (seconds)
pub const MESSAGE_DEDUP_TTL_SECS: u64 = 3600; // 1 hour

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        assert_eq!(2 + 2, 4);
    }
}
