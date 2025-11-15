//! Message Router with DOS Protection
//!
//! SECURITY M1: Implements comprehensive DOS protection via:
//! - Multi-tier rate limiting (per-node, global, burst)
//! - Message size limits
//! - TTL bounds enforcement
//! - Spam detection heuristics
//! - Reputation-based throttling

use crate::{
    deduplication::DeduplicationCache, priority_queue::PriorityQueue, rate_limiter::RateLimiter,
    RoutingError,
};
use myriadmesh_protocol::{message::Message, NodeId};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, RwLock};

/// Maximum message size (1 MB)
const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Minimum message size (header only, ~200 bytes)
const MIN_MESSAGE_SIZE: usize = 200;

/// Maximum TTL (hops)
const MAX_TTL: u8 = 32;

/// Minimum TTL (hops)
const MIN_TTL: u8 = 1;

/// Burst limit window (5 seconds)
const BURST_WINDOW_SECS: u64 = 5;

/// Maximum messages per burst window
const MAX_BURST_MESSAGES: u32 = 20;

/// Spam detection threshold (messages per minute)
const SPAM_THRESHOLD: u32 = 100;

/// Spam penalty duration (minutes)
const SPAM_PENALTY_DURATION_MINS: u64 = 10;

/// Message deduplication TTL (seconds)
const DEDUP_TTL_SECS: u64 = 3600;

/// Router statistics
#[derive(Debug, Default, Clone)]
pub struct RouterStats {
    pub messages_routed: u64,
    pub messages_dropped: u64,
    pub rate_limit_hits: u64,
    pub spam_detections: u64,
    pub burst_limit_hits: u64,
    pub invalid_messages: u64,
}

/// Spam tracking entry
#[derive(Debug, Clone)]
struct SpamTracker {
    message_count: u32,
    window_start: Instant,
    penalty_until: Option<Instant>,
}

/// Message Router
///
/// SECURITY M1: Comprehensive DOS protection
pub struct Router {
    /// Node ID of this router
    node_id: NodeId,

    /// Priority queue for outbound messages
    outbound_queue: Arc<RwLock<PriorityQueue>>,

    /// Deduplication cache
    dedup_cache: Arc<RwLock<DeduplicationCache>>,

    /// Rate limiter (per-node and global)
    rate_limiter: Arc<RwLock<RateLimiter>>,

    /// Burst protection (node_id -> (count, window_start))
    burst_tracker: Arc<RwLock<HashMap<NodeId, (u32, Instant)>>>,

    /// Spam detection tracker
    spam_tracker: Arc<RwLock<HashMap<NodeId, SpamTracker>>>,

    /// Router statistics
    stats: Arc<RwLock<RouterStats>>,

    /// Local delivery channel (for messages destined for this node)
    local_delivery_tx: Option<mpsc::UnboundedSender<Message>>,
}

impl Router {
    /// Create a new router
    ///
    /// # Arguments
    /// * `node_id` - This node's ID
    /// * `per_node_limit` - Messages per minute per node
    /// * `global_limit` - Total messages per minute
    /// * `queue_capacity` - Messages per priority level
    pub fn new(
        node_id: NodeId,
        per_node_limit: u32,
        global_limit: u32,
        queue_capacity: usize,
    ) -> Self {
        Router {
            node_id,
            outbound_queue: Arc::new(RwLock::new(PriorityQueue::new(queue_capacity))),
            dedup_cache: Arc::new(RwLock::new(DeduplicationCache::new(10_000, DEDUP_TTL_SECS))),
            rate_limiter: Arc::new(RwLock::new(RateLimiter::new(per_node_limit, global_limit))),
            burst_tracker: Arc::new(RwLock::new(HashMap::new())),
            spam_tracker: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(RouterStats::default())),
            local_delivery_tx: None,
        }
    }

    /// Set the local delivery channel
    ///
    /// # Arguments
    /// * `tx` - Channel sender for locally delivered messages
    pub fn set_local_delivery_channel(&mut self, tx: mpsc::UnboundedSender<Message>) {
        self.local_delivery_tx = Some(tx);
    }

    /// Create a channel for receiving locally delivered messages
    ///
    /// Returns a tuple of (sender, receiver) for local message delivery
    pub fn create_local_delivery_channel() -> (
        mpsc::UnboundedSender<Message>,
        mpsc::UnboundedReceiver<Message>,
    ) {
        mpsc::unbounded_channel()
    }

    /// Route an incoming message
    ///
    /// SECURITY M1: Comprehensive validation and rate limiting
    ///
    /// # Security Checks
    /// 1. Message size validation
    /// 2. TTL bounds checking
    /// 3. Deduplication (replay protection)
    /// 4. Rate limiting (per-node and global)
    /// 5. Burst protection
    /// 6. Spam detection
    pub async fn route_message(&self, message: Message) -> Result<(), RoutingError> {
        // SECURITY M1: Validate message size
        let msg_size = self.estimate_message_size(&message);
        if msg_size > MAX_MESSAGE_SIZE {
            let mut stats = self.stats.write().await;
            stats.invalid_messages += 1;
            stats.messages_dropped += 1;
            return Err(RoutingError::InvalidMessage(format!(
                "Message too large: {} bytes (max: {})",
                msg_size, MAX_MESSAGE_SIZE
            )));
        }

        if msg_size < MIN_MESSAGE_SIZE {
            let mut stats = self.stats.write().await;
            stats.invalid_messages += 1;
            stats.messages_dropped += 1;
            return Err(RoutingError::InvalidMessage(format!(
                "Message too small: {} bytes (min: {})",
                msg_size, MIN_MESSAGE_SIZE
            )));
        }

        // SECURITY M1: Validate TTL bounds
        if message.ttl > MAX_TTL {
            let mut stats = self.stats.write().await;
            stats.invalid_messages += 1;
            stats.messages_dropped += 1;
            return Err(RoutingError::InvalidMessage(format!(
                "TTL too large: {} hops (max: {})",
                message.ttl, MAX_TTL
            )));
        }

        if message.ttl < MIN_TTL {
            let mut stats = self.stats.write().await;
            stats.invalid_messages += 1;
            stats.messages_dropped += 1;
            return Err(RoutingError::InvalidMessage(format!(
                "TTL too small: {} hops (min: {})",
                message.ttl, MIN_TTL
            )));
        }

        // SECURITY H8: Check for duplicate (replay protection)
        {
            let mut dedup = self.dedup_cache.write().await;
            if dedup.has_seen(&message.id) {
                let mut stats = self.stats.write().await;
                stats.messages_dropped += 1;
                return Err(RoutingError::DuplicateMessage(message.id));
            }
            dedup.mark_seen(message.id);
        }

        // SECURITY M1: Check spam penalty
        {
            let spam_tracker = self.spam_tracker.read().await;
            if let Some(tracker) = spam_tracker.get(&message.source) {
                if let Some(penalty_until) = tracker.penalty_until {
                    if Instant::now() < penalty_until {
                        let mut stats = self.stats.write().await;
                        stats.messages_dropped += 1;
                        return Err(RoutingError::RateLimited(format!(
                            "Node {:?} under spam penalty",
                            message.source
                        )));
                    }
                }
            }
        }

        // SECURITY M1: Check burst limit
        {
            let mut burst_tracker = self.burst_tracker.write().await;
            let now = Instant::now();
            let entry = burst_tracker.entry(message.source).or_insert((0, now));

            // Reset if window expired
            if now.duration_since(entry.1) >= Duration::from_secs(BURST_WINDOW_SECS) {
                entry.0 = 0;
                entry.1 = now;
            }

            // Check burst limit
            if entry.0 >= MAX_BURST_MESSAGES {
                let mut stats = self.stats.write().await;
                stats.burst_limit_hits += 1;
                stats.messages_dropped += 1;
                return Err(RoutingError::RateLimited(format!(
                    "Burst limit exceeded: {} messages in {} seconds",
                    entry.0, BURST_WINDOW_SECS
                )));
            }

            entry.0 += 1;
        }

        // SECURITY M1: Check rate limits
        {
            let mut rate_limiter = self.rate_limiter.write().await;
            if let Err(e) = rate_limiter.check_rate(&message.source) {
                let mut stats = self.stats.write().await;
                stats.rate_limit_hits += 1;
                stats.messages_dropped += 1;
                return Err(RoutingError::RateLimited(e.to_string()));
            }
        }

        // SECURITY M1: Update spam detection
        {
            let mut spam_tracker = self.spam_tracker.write().await;
            let now = Instant::now();
            let tracker = spam_tracker.entry(message.source).or_insert(SpamTracker {
                message_count: 0,
                window_start: now,
                penalty_until: None,
            });

            // Reset window if expired
            if now.duration_since(tracker.window_start) >= Duration::from_secs(60) {
                tracker.message_count = 0;
                tracker.window_start = now;
            }

            tracker.message_count += 1;

            // Apply spam penalty if threshold exceeded
            if tracker.message_count > SPAM_THRESHOLD && tracker.penalty_until.is_none() {
                tracker.penalty_until =
                    Some(now + Duration::from_secs(SPAM_PENALTY_DURATION_MINS * 60));
                let mut stats = self.stats.write().await;
                stats.spam_detections += 1;
                stats.messages_dropped += 1;
                return Err(RoutingError::RateLimited(format!(
                    "Spam threshold exceeded: {} messages/min (threshold: {})",
                    tracker.message_count, SPAM_THRESHOLD
                )));
            }
        }

        // Route based on destination
        if message.destination == self.node_id {
            // Message is for us - deliver locally
            self.deliver_local(message).await?;
        } else {
            // Forward to next hop
            self.forward_message(message).await?;
        }

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.messages_routed += 1;

        Ok(())
    }

    /// Deliver message to local application
    async fn deliver_local(&self, message: Message) -> Result<(), RoutingError> {
        if let Some(tx) = &self.local_delivery_tx {
            // Send message to local delivery channel
            tx.send(message).map_err(|e| {
                RoutingError::Other(format!("Local delivery channel closed: {}", e))
            })?;

            // Update statistics
            let mut stats = self.stats.write().await;
            stats.messages_routed += 1;

            Ok(())
        } else {
            // No local delivery channel configured, log and drop
            Err(RoutingError::Other(
                "Local delivery channel not configured".to_string(),
            ))
        }
    }

    /// Forward message to next hop
    async fn forward_message(&self, message: Message) -> Result<(), RoutingError> {
        // Add to outbound queue based on priority
        let mut queue = self.outbound_queue.write().await;
        queue
            .enqueue(message)
            .map_err(|e| RoutingError::QueueFull(e.to_string()))?;
        Ok(())
    }

    /// Estimate message size in bytes
    fn estimate_message_size(&self, message: &Message) -> usize {
        // Header (fixed size) + payload
        163 + // Header size (NodeID + NodeID + MessageID + fields)
        message.payload.len()
    }

    /// Get router statistics
    pub async fn get_stats(&self) -> RouterStats {
        self.stats.read().await.clone()
    }

    /// Clear statistics
    pub async fn clear_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = RouterStats::default();
    }

    /// Cleanup expired tracking data
    pub async fn cleanup(&self) {
        // Cleanup rate limiter
        {
            let mut rate_limiter = self.rate_limiter.write().await;
            rate_limiter.cleanup_expired();
        }

        // Cleanup burst tracker
        {
            let mut burst_tracker = self.burst_tracker.write().await;
            let now = Instant::now();
            burst_tracker.retain(|_, (_, start)| {
                now.duration_since(*start) < Duration::from_secs(BURST_WINDOW_SECS)
            });
        }

        // Cleanup spam tracker (remove expired penalties)
        {
            let mut spam_tracker = self.spam_tracker.write().await;
            let now = Instant::now();
            spam_tracker.retain(|_, tracker| {
                // Keep if penalty is still active or recent activity
                if let Some(penalty_until) = tracker.penalty_until {
                    now < penalty_until
                } else {
                    now.duration_since(tracker.window_start) < Duration::from_secs(300)
                }
            });
        }

        // Cleanup deduplication cache
        {
            let mut dedup = self.dedup_cache.write().await;
            dedup.cleanup_expired();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myriadmesh_protocol::{message::MessageType, types::Priority, types::NODE_ID_SIZE};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_node_id(byte: u8) -> NodeId {
        NodeId::from_bytes([byte; NODE_ID_SIZE])
    }

    // Use atomic counter for unique sequence numbers
    use std::sync::atomic::{AtomicU32, Ordering};
    static MESSAGE_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn create_test_message(source: NodeId, dest: NodeId, payload_size: usize) -> Message {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let sequence = MESSAGE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let payload = vec![0u8; payload_size];

        Message {
            id: myriadmesh_protocol::MessageId::generate(
                &source, &dest, &payload, timestamp, sequence,
            ),
            source,
            destination: dest,
            message_type: MessageType::Data,
            priority: Priority::normal(),
            ttl: 16,
            timestamp,
            sequence,
            payload,
        }
    }

    #[tokio::test]
    async fn test_router_creation() {
        let node_id = create_test_node_id(1);
        let router = Router::new(node_id, 60, 1000, 100);

        let stats = router.get_stats().await;
        assert_eq!(stats.messages_routed, 0);
        assert_eq!(stats.messages_dropped, 0);
    }

    #[tokio::test]
    async fn test_message_size_validation() {
        let node_id = create_test_node_id(1);
        let router = Router::new(node_id, 60, 1000, 100);

        let source = create_test_node_id(2);
        let dest = create_test_node_id(3);

        // Too large
        let large_msg = create_test_message(source, dest, MAX_MESSAGE_SIZE);
        assert!(router.route_message(large_msg).await.is_err());

        // Too small
        let small_msg = create_test_message(source, dest, 10);
        assert!(router.route_message(small_msg).await.is_err());

        let stats = router.get_stats().await;
        assert_eq!(stats.invalid_messages, 2);
        assert_eq!(stats.messages_dropped, 2);
    }

    #[tokio::test]
    async fn test_ttl_validation() {
        let node_id = create_test_node_id(1);
        let router = Router::new(node_id, 60, 1000, 100);

        let source = create_test_node_id(2);
        let dest = create_test_node_id(3);

        // TTL too large
        let mut msg = create_test_message(source, dest, 1000);
        msg.ttl = MAX_TTL + 10;
        assert!(router.route_message(msg).await.is_err());

        // TTL too small (0)
        let mut msg = create_test_message(source, dest, 1000);
        msg.ttl = 0;
        assert!(router.route_message(msg).await.is_err());

        let stats = router.get_stats().await;
        assert_eq!(stats.invalid_messages, 2);
    }

    #[tokio::test]
    async fn test_deduplication() {
        let node_id = create_test_node_id(1);
        let router = Router::new(node_id, 60, 1000, 100);

        let source = create_test_node_id(2);
        let dest = create_test_node_id(3);

        let msg = create_test_message(source, dest, 1000);
        let msg_copy = msg.clone();

        // First message should succeed
        assert!(router.route_message(msg).await.is_ok());

        // Duplicate should be rejected
        assert!(router.route_message(msg_copy).await.is_err());

        let stats = router.get_stats().await;
        assert_eq!(stats.messages_routed, 1);
        assert_eq!(stats.messages_dropped, 1);
    }

    #[tokio::test]
    async fn test_burst_protection() {
        let node_id = create_test_node_id(1);
        let router = Router::new(node_id, 1000, 10000, 100); // High limits for rate limiter

        let source = create_test_node_id(2);
        let dest = create_test_node_id(3);

        // Send up to burst limit
        for _ in 0..MAX_BURST_MESSAGES {
            let msg = create_test_message(source, dest, 1000);
            assert!(router.route_message(msg).await.is_ok());
        }

        // Next message should be rejected
        let msg = create_test_message(source, dest, 1000);
        assert!(router.route_message(msg).await.is_err());

        let stats = router.get_stats().await;
        assert_eq!(stats.burst_limit_hits, 1);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let node_id = create_test_node_id(1);
        let router = Router::new(node_id, 5, 1000, 100); // Low per-node limit

        let source = create_test_node_id(2);
        let dest = create_test_node_id(3);

        // Send up to per-node limit
        for _ in 0..5 {
            let msg = create_test_message(source, dest, 1000);
            assert!(router.route_message(msg).await.is_ok());
        }

        // Next message should be rate limited
        let msg = create_test_message(source, dest, 1000);
        assert!(router.route_message(msg).await.is_err());

        let stats = router.get_stats().await;
        assert_eq!(stats.rate_limit_hits, 1);
    }

    #[tokio::test]
    async fn test_dos_protection() {
        // SECURITY M1: Verify that DOS protection prevents message flooding
        // This test verifies that SOME protection mechanism kicks in when flooding
        let node_id = create_test_node_id(1);
        let router = Router::new(node_id, 100, 10000, 200);

        let source = create_test_node_id(2);
        let dest = create_test_node_id(3);

        // Attempt to flood with many messages rapidly
        let mut success_count = 0;
        let mut reject_count = 0;

        for _ in 0..150 {
            let msg = create_test_message(source, dest, 1000);
            match router.route_message(msg).await {
                Ok(_) => success_count += 1,
                Err(_) => reject_count += 1,
            }
        }

        let stats = router.get_stats().await;

        // Verify that DOS protection kicked in (either burst, rate, or spam)
        assert!(
            reject_count > 0,
            "DOS protection should have rejected some messages"
        );
        assert!(
            stats.rate_limit_hits > 0 || stats.burst_limit_hits > 0 || stats.spam_detections > 0,
            "At least one DOS protection mechanism should have triggered"
        );

        // Verify statistics are being tracked
        assert_eq!(
            stats.messages_routed + stats.messages_dropped,
            success_count + reject_count
        );
    }

    #[tokio::test]
    async fn test_cleanup() {
        let node_id = create_test_node_id(1);
        let router = Router::new(node_id, 60, 1000, 100);

        let source = create_test_node_id(2);
        let dest = create_test_node_id(3);

        // Send some messages
        for _ in 0..5 {
            let msg = create_test_message(source, dest, 1000);
            let _ = router.route_message(msg).await;
        }

        // Cleanup should not crash
        router.cleanup().await;

        // Verify router still works
        let msg = create_test_message(source, dest, 1000);
        assert!(router.route_message(msg).await.is_ok());
    }

    #[tokio::test]
    async fn test_statistics_tracking() {
        let node_id = create_test_node_id(1);
        let router = Router::new(node_id, 10, 1000, 100);

        let source = create_test_node_id(2);
        let dest = create_test_node_id(3);

        // Route some valid messages
        for _ in 0..3 {
            let msg = create_test_message(source, dest, 1000);
            let _ = router.route_message(msg).await;
        }

        // Send invalid message
        let invalid_msg = create_test_message(source, dest, MAX_MESSAGE_SIZE);
        let _ = router.route_message(invalid_msg).await;

        let stats = router.get_stats().await;
        assert_eq!(stats.messages_routed, 3);
        assert_eq!(stats.invalid_messages, 1);

        // Clear stats
        router.clear_stats().await;
        let stats = router.get_stats().await;
        assert_eq!(stats.messages_routed, 0);
    }
}
