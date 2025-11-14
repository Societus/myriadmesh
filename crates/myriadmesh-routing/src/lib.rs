//! MyriadMesh Message Routing
//!
//! This module implements the message router for Phase 2:
//! - Priority queue system (5 levels)
//! - Direct and multi-hop routing
//! - Store-and-forward for offline nodes
//! - Message deduplication
//! - Content tag filtering (optional)

pub mod deduplication;
pub mod error;
pub mod priority_queue;
pub mod rate_limiter;
pub mod router;

pub use deduplication::DeduplicationCache;
pub use error::{Result, RoutingError};
pub use priority_queue::{PriorityLevel, PriorityQueue};
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
