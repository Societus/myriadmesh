//! Rate limiting for message routing
//!
//! Token bucket algorithm for rate limiting

use dashmap::DashMap;
use myriadmesh_protocol::NodeId;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Default rate limit: 100 messages per second
const DEFAULT_RATE: u32 = 100;

/// Default burst size: 200 messages
const DEFAULT_BURST: u32 = 200;

/// Token bucket for rate limiting
#[derive(Debug)]
struct TokenBucket {
    /// Current number of tokens
    tokens: f64,

    /// Maximum tokens (burst size)
    capacity: f64,

    /// Token refill rate (tokens per second)
    refill_rate: f64,

    /// Last refill time
    last_refill: Instant,
}

impl TokenBucket {
    fn new(rate: u32, burst: u32) -> Self {
        Self {
            tokens: burst as f64,
            capacity: burst as f64,
            refill_rate: rate as f64,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume one token
    /// Returns true if allowed, false if rate limited
    fn try_consume(&mut self) -> bool {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        // Add tokens based on elapsed time
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }

    /// Get current token count
    fn available_tokens(&mut self) -> f64 {
        self.refill();
        self.tokens
    }
}

/// Rate limiter
pub struct RateLimiter {
    /// Per-node rate limits
    node_limits: Arc<DashMap<NodeId, TokenBucket>>,

    /// Global rate limit
    global_limit: Arc<tokio::sync::Mutex<TokenBucket>>,

    /// Configuration
    config: RateLimiterConfig,
}

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Per-node rate (messages/second)
    pub per_node_rate: u32,

    /// Per-node burst size
    pub per_node_burst: u32,

    /// Global rate (messages/second)
    pub global_rate: u32,

    /// Global burst size
    pub global_burst: u32,

    /// Enable rate limiting
    pub enabled: bool,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            per_node_rate: DEFAULT_RATE,
            per_node_burst: DEFAULT_BURST,
            global_rate: DEFAULT_RATE * 10, // 1000/sec global
            global_burst: DEFAULT_BURST * 10,
            enabled: true,
        }
    }
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimiterConfig) -> Self {
        Self {
            node_limits: Arc::new(DashMap::new()),
            global_limit: Arc::new(tokio::sync::Mutex::new(TokenBucket::new(
                config.global_rate,
                config.global_burst,
            ))),
            config,
        }
    }

    /// Check if a message from a node is allowed
    /// Returns true if allowed, false if rate limited
    pub async fn check_rate_limit(&self, node_id: &NodeId) -> bool {
        if !self.config.enabled {
            return true;
        }

        // Check global limit first
        let mut global = self.global_limit.lock().await;
        if !global.try_consume() {
            return false;
        }
        drop(global);

        // Check per-node limit
        let mut entry = self.node_limits.entry(*node_id).or_insert_with(|| {
            TokenBucket::new(self.config.per_node_rate, self.config.per_node_burst)
        });

        entry.try_consume()
    }

    /// Get available tokens for a node
    pub fn available_tokens(&self, node_id: &NodeId) -> f64 {
        if let Some(mut entry) = self.node_limits.get_mut(node_id) {
            entry.available_tokens()
        } else {
            self.config.per_node_burst as f64
        }
    }

    /// Get global available tokens
    pub async fn global_available_tokens(&self) -> f64 {
        let mut global = self.global_limit.lock().await;
        global.available_tokens()
    }

    /// Clear all node limits (for testing)
    pub fn clear(&self) {
        self.node_limits.clear();
    }

    /// Get number of tracked nodes
    pub fn tracked_nodes(&self) -> usize {
        self.node_limits.len()
    }

    /// Cleanup inactive nodes (not used in the last duration)
    pub async fn cleanup_inactive(&self, max_age: Duration) {
        let now = Instant::now();
        let cutoff = now - max_age;

        self.node_limits.retain(|_, bucket| {
            bucket.last_refill > cutoff
        });
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimiterConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_node_id(byte: u8) -> NodeId {
        NodeId::from_bytes([byte; 32])
    }

    #[tokio::test]
    async fn test_rate_limit_basic() {
        let mut config = RateLimiterConfig::default();
        config.per_node_rate = 10;
        config.per_node_burst = 10;
        let limiter = RateLimiter::new(config);

        let node = create_node_id(1);

        // Should allow up to burst size
        for _ in 0..10 {
            assert!(limiter.check_rate_limit(&node).await);
        }

        // 11th message should be rate limited
        assert!(!limiter.check_rate_limit(&node).await);
    }

    #[tokio::test]
    async fn test_rate_limit_refill() {
        let mut config = RateLimiterConfig::default();
        config.per_node_rate = 100; // 100/sec
        config.per_node_burst = 10;
        let limiter = RateLimiter::new(config);

        let node = create_node_id(1);

        // Consume all tokens
        for _ in 0..10 {
            assert!(limiter.check_rate_limit(&node).await);
        }

        // Should be rate limited
        assert!(!limiter.check_rate_limit(&node).await);

        // Wait for refill (100 tokens/sec = 10ms per token)
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should have ~10 tokens now
        assert!(limiter.check_rate_limit(&node).await);
    }

    #[tokio::test]
    async fn test_global_rate_limit() {
        let mut config = RateLimiterConfig::default();
        config.per_node_rate = 1000;
        config.per_node_burst = 1000;
        config.global_rate = 10;
        config.global_burst = 10;
        let limiter = RateLimiter::new(config);

        // Different nodes
        let nodes: Vec<_> = (0..5).map(|i| create_node_id(i)).collect();

        let mut allowed = 0;
        for node in &nodes {
            for _ in 0..5 {
                if limiter.check_rate_limit(node).await {
                    allowed += 1;
                }
            }
        }

        // Should allow ~10 messages total (global burst)
        assert!(allowed <= 10);
    }

    #[tokio::test]
    async fn test_disabled_rate_limit() {
        let mut config = RateLimiterConfig::default();
        config.enabled = false;
        let limiter = RateLimiter::new(config);

        let node = create_node_id(1);

        // Should allow unlimited messages
        for _ in 0..1000 {
            assert!(limiter.check_rate_limit(&node).await);
        }
    }

    #[tokio::test]
    async fn test_available_tokens() {
        let config = RateLimiterConfig::default();
        let limiter = RateLimiter::new(config);

        let node = create_node_id(1);

        // Initially should have burst amount
        assert_eq!(
            limiter.available_tokens(&node),
            config.per_node_burst as f64
        );

        // Consume some tokens
        for _ in 0..5 {
            limiter.check_rate_limit(&node).await;
        }

        // Should have fewer tokens
        assert!(limiter.available_tokens(&node) < config.per_node_burst as f64);
    }

    #[tokio::test]
    async fn test_cleanup_inactive() {
        let config = RateLimiterConfig::default();
        let limiter = RateLimiter::new(config);

        // Add some nodes
        for i in 0..5 {
            let node = create_node_id(i);
            limiter.check_rate_limit(&node).await;
        }

        assert_eq!(limiter.tracked_nodes(), 5);

        // Wait and cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;
        limiter
            .cleanup_inactive(Duration::from_millis(50))
            .await;

        // All nodes should be cleaned up
        assert_eq!(limiter.tracked_nodes(), 0);
    }

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(100, 200);

        // Should have full capacity
        assert_eq!(bucket.available_tokens(), 200.0);

        // Consume tokens
        for _ in 0..100 {
            assert!(bucket.try_consume());
        }

        assert!(bucket.available_tokens() < 101.0);
        assert!(bucket.available_tokens() > 99.0);
    }
}
