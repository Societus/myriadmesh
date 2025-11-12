//! Message Router - Main coordinator for message routing

use crate::deduplication::DeduplicationCache;
use crate::error::{Result, RoutingError};
use crate::priority_queue::{PriorityQueue, QueuedMessage};
use crate::rate_limiter::RateLimiter;
use crate::{
    MAX_CACHED_MESSAGES_PER_DEST, MESSAGE_DEDUP_CACHE_SIZE, MESSAGE_DEDUP_TTL_SECS,
};
use myriadmesh_dht::DhtManager;
use myriadmesh_protocol::{Message, MessageId, NodeId};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Get current timestamp in seconds
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Message router configuration
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Per-node rate limit (messages/minute)
    pub per_node_rate_limit: u32,

    /// Global rate limit (messages/minute)
    pub global_rate_limit: u32,

    /// Maximum messages per priority queue
    pub max_queue_size: usize,

    /// Enable content tag filtering
    pub enable_filtering: bool,

    /// Blocked content tags
    pub blocked_tags: Vec<String>,

    /// Allowed content tags (empty = allow all)
    pub allowed_tags: Vec<String>,

    /// Always relay sensitive messages
    pub always_relay_sensitive: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        RouterConfig {
            per_node_rate_limit: 1000,
            global_rate_limit: 10000,
            max_queue_size: 1000,
            enable_filtering: false,
            blocked_tags: Vec::new(),
            allowed_tags: Vec::new(),
            always_relay_sensitive: true,
        }
    }
}

/// Cached message for store-and-forward
#[derive(Debug, Clone)]
pub struct CachedMessage {
    pub message_id: MessageId,
    pub destination: NodeId,
    pub message: Message,
    pub cached_at: u64,
    pub expires_at: u64,
}

/// Routing statistics
#[derive(Debug, Clone, Default)]
pub struct RoutingStats {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_relayed: u64,
    pub messages_delivered: u64,
    pub messages_cached: u64,
    pub messages_dropped: u64,
    pub invalid_signatures: u64,
    pub replays_detected: u64,
    pub invalid_timestamps: u64,
    pub ttl_exceeded: u64,
    pub filtered_messages: u64,
    pub rate_limit_exceeded: u64,
}

/// Message Router - Coordinates message routing
pub struct MessageRouter {
    /// Local node ID
    local_node_id: NodeId,

    /// Priority queue for outbound messages
    outbound_queue: Arc<RwLock<PriorityQueue>>,

    /// Message deduplication cache
    dedup_cache: Arc<RwLock<DeduplicationCache>>,

    /// Rate limiter
    rate_limiter: Arc<RwLock<RateLimiter>>,

    /// Cached messages for offline nodes (dest -> messages)
    cached_messages: Arc<RwLock<HashMap<NodeId, Vec<CachedMessage>>>>,

    /// DHT manager reference
    dht: Arc<DhtManager>,

    /// Configuration
    config: RouterConfig,

    /// Routing statistics
    stats: Arc<RwLock<RoutingStats>>,
}

impl MessageRouter {
    /// Create a new message router
    pub fn new(
        local_node_id: NodeId,
        dht: Arc<DhtManager>,
        config: RouterConfig,
    ) -> Self {
        MessageRouter {
            local_node_id,
            outbound_queue: Arc::new(RwLock::new(PriorityQueue::new(config.max_queue_size))),
            dedup_cache: Arc::new(RwLock::new(DeduplicationCache::new(
                MESSAGE_DEDUP_CACHE_SIZE,
                MESSAGE_DEDUP_TTL_SECS,
            ))),
            rate_limiter: Arc::new(RwLock::new(RateLimiter::new(
                config.per_node_rate_limit,
                config.global_rate_limit,
            ))),
            cached_messages: Arc::new(RwLock::new(HashMap::new())),
            dht,
            config,
            stats: Arc::new(RwLock::new(RoutingStats::default())),
        }
    }

    /// Route an outbound message
    ///
    /// This enqueues the message for sending
    pub async fn route_message(&self, message: Message) -> Result<()> {
        // Check rate limit for our own messages
        self.rate_limiter
            .write()
            .await
            .check_rate(&self.local_node_id)
            .map_err(|e| RoutingError::RateLimitExceeded(e.to_string()))?;

        // Enqueue message
        self.outbound_queue
            .write()
            .await
            .enqueue(message)
            .map_err(|e| RoutingError::QueueFull(e))?;

        self.stats.write().await.messages_sent += 1;

        Ok(())
    }

    /// Handle incoming message
    ///
    /// Returns Some(message) if it's for us, None if relayed or cached
    pub async fn handle_incoming(&self, message: Message) -> Result<Option<Message>> {
        let source_id = message.source;
        let message_id = message.id;

        // Check rate limit
        if let Err(e) = self.rate_limiter.write().await.check_rate(&source_id) {
            self.stats.write().await.rate_limit_exceeded += 1;
            return Err(RoutingError::RateLimitExceeded(e.to_string()));
        }

        // Check for replay
        if self.dedup_cache.read().await.has_seen(&message_id) {
            self.stats.write().await.replays_detected += 1;
            return Err(RoutingError::ReplayDetected);
        }

        // Mark as seen
        self.dedup_cache.write().await.mark_seen(message_id);

        // Check timestamp (±5 minutes)
        let now_secs = now() as i64;
        let msg_timestamp = message.timestamp as i64;
        if (now_secs - msg_timestamp).abs() > 5 * 60 {
            self.stats.write().await.invalid_timestamps += 1;
            return Err(RoutingError::InvalidTimestamp);
        }

        self.stats.write().await.messages_received += 1;

        // Check if message is for us
        if message.destination == self.local_node_id {
            // Deliver to application
            self.stats.write().await.messages_delivered += 1;
            Ok(Some(message))
        } else {
            // Relay to next hop
            self.relay_message(message).await?;
            Ok(None)
        }
    }

    /// Relay a message to the next hop
    async fn relay_message(&self, mut message: Message) -> Result<()> {
        // Check if we should relay (content filtering)
        if !self.should_relay(&message) {
            self.stats.write().await.filtered_messages += 1;
            return Ok(());
        }

        // Check TTL
        if message.ttl == 0 {
            self.stats.write().await.ttl_exceeded += 1;
            return Err(RoutingError::TtlExceeded);
        }

        // Decrement TTL
        message.ttl = message.ttl.saturating_sub(1);

        // Try to route to destination
        match self.route_to_destination(message.clone()).await {
            Ok(()) => {
                // Successfully relayed
                self.stats.write().await.messages_relayed += 1;

                // Update relay statistics for reputation
                self.dht.record_successful_relay(self.local_node_id).await;
                Ok(())
            }
            Err(RoutingError::DestinationUnknown) => {
                // Cache for later delivery
                self.cache_message(message).await?;
                self.stats.write().await.messages_cached += 1;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Check if a message should be relayed
    fn should_relay(&self, _message: &Message) -> bool {
        // TODO: Implement content tag filtering when Message has routing_flags and content_tags
        // For now, always relay
        true

        /* Future implementation:
        // Always relay SENSITIVE messages
        if message.routing_flags.contains(RoutingFlags::SENSITIVE) {
            return true;
        }

        // If filtering disabled, relay everything
        if !self.config.enable_filtering {
            return true;
        }

        // If no tags, relay (E2E_STRICT)
        if !message.routing_flags.contains(RoutingFlags::RELAY_FILTERABLE) {
            return true;
        }

        // Check blocked tags
        for tag in &message.content_tags {
            if self.config.blocked_tags.contains(tag) {
                return false;
            }
        }

        // Check allowed tags (if specified)
        if !self.config.allowed_tags.is_empty() {
            let has_allowed = message.content_tags.iter()
                .any(|tag| self.config.allowed_tags.contains(tag));
            if !has_allowed {
                return false;
            }
        }

        true
        */
    }

    /// Route message to destination
    async fn route_to_destination(&self, message: Message) -> Result<()> {
        // Look up destination in DHT
        let dest_node = self
            .dht
            .find_node_local(&message.destination)
            .await
            .ok_or(RoutingError::DestinationUnknown)?;

        // TODO: Send message via network adapter
        // This will be implemented when integrating with AdapterManager
        // For now, return success if we found the destination
        let _ = dest_node;
        Ok(())
    }

    /// Cache a message for offline destination
    async fn cache_message(&self, message: Message) -> Result<()> {
        let mut cache = self.cached_messages.write().await;

        // Check per-destination limit
        let cached_count = cache.get(&message.destination).map(|v| v.len()).unwrap_or(0);

        if cached_count >= MAX_CACHED_MESSAGES_PER_DEST {
            return Err(RoutingError::CacheFull);
        }

        // Create cache record
        let cache_record = CachedMessage {
            message_id: message.id,
            destination: message.destination,
            message: message.clone(),
            cached_at: now(),
            expires_at: now() + (24 * 3600), // 24 hours
        };

        // Store locally
        cache
            .entry(message.destination)
            .or_insert_with(Vec::new)
            .push(cache_record.clone());

        // TODO: Store in DHT for redundancy
        Ok(())
    }

    /// Retrieve cached messages for a destination
    pub async fn retrieve_cached_messages(&self, destination: &NodeId) -> Vec<Message> {
        let mut cache = self.cached_messages.write().await;

        if let Some(cached) = cache.remove(destination) {
            // Filter out expired messages
            cached
                .into_iter()
                .filter(|msg| msg.expires_at > now())
                .map(|cached_msg| cached_msg.message)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Dequeue and send next outbound message
    ///
    /// Returns the message that was sent, or None if queue is empty
    pub async fn send_next_message(&self) -> Option<QueuedMessage> {
        let mut queue = self.outbound_queue.write().await;
        queue.dequeue()
    }

    /// Get routing statistics
    pub async fn get_stats(&self) -> RoutingStats {
        self.stats.read().await.clone()
    }

    /// Get number of cached messages for a destination
    pub async fn get_cached_count(&self, destination: &NodeId) -> usize {
        self.cached_messages
            .read()
            .await
            .get(destination)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Clear expired cached messages
    pub async fn cleanup_expired_cache(&self) {
        let mut cache = self.cached_messages.write().await;
        let current_time = now();

        for messages in cache.values_mut() {
            messages.retain(|msg| msg.expires_at > current_time);
        }

        // Remove empty entries
        cache.retain(|_, messages| !messages.is_empty());
    }

    /// Periodic maintenance
    pub async fn maintenance(&self) {
        // Cleanup expired messages
        self.dedup_cache.write().await.cleanup_expired();
        self.cleanup_expired_cache().await;

        // Cleanup expired rate limiters
        self.rate_limiter.write().await.cleanup_expired();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myriadmesh_crypto::identity::NodeIdentity;
    use myriadmesh_dht::DhtConfig;
    use myriadmesh_protocol::MessageType;

    fn create_test_identity() -> NodeIdentity {
        myriadmesh_crypto::init().unwrap();
        NodeIdentity::generate().unwrap()
    }

    fn create_test_message(source: NodeId, dest: NodeId) -> Message {
        Message::new(source, dest, MessageType::Data, vec![1, 2, 3, 4]).unwrap()
    }

    #[tokio::test]
    async fn test_router_creation() {
        let identity = Arc::new(create_test_identity());
        let node_id = NodeId::from_bytes(*identity.node_id.as_bytes());
        let dht = Arc::new(DhtManager::new(identity, DhtConfig::default()));
        let config = RouterConfig::default();

        let router = MessageRouter::new(node_id, dht, config);
        let stats = router.get_stats().await;

        assert_eq!(stats.messages_sent, 0);
        assert_eq!(stats.messages_received, 0);
    }

    #[tokio::test]
    async fn test_route_message() {
        let identity = Arc::new(create_test_identity());
        let node_id = NodeId::from_bytes(*identity.node_id.as_bytes());
        let dht = Arc::new(DhtManager::new(identity, DhtConfig::default()));
        let router = MessageRouter::new(node_id, dht, RouterConfig::default());

        let message = create_test_message(node_id, NodeId::from_bytes([2u8; 32]));

        router.route_message(message).await.unwrap();

        let stats = router.get_stats().await;
        assert_eq!(stats.messages_sent, 1);
    }

    #[tokio::test]
    async fn test_replay_detection() {
        let identity = Arc::new(create_test_identity());
        let node_id = NodeId::from_bytes(*identity.node_id.as_bytes());
        let dht = Arc::new(DhtManager::new(identity, DhtConfig::default()));
        let router = MessageRouter::new(node_id, dht, RouterConfig::default());

        let message = create_test_message(NodeId::from_bytes([1u8; 32]), node_id);

        // First receive should succeed
        let result = router.handle_incoming(message.clone()).await;
        if let Err(e) = &result {
            panic!("First handle_incoming failed with error: {:?}", e);
        }
        assert!(result.is_ok());

        // Second receive should be rejected (replay)
        assert!(matches!(
            router.handle_incoming(message).await,
            Err(RoutingError::ReplayDetected)
        ));
    }

    #[tokio::test]
    async fn test_message_caching() {
        let identity = Arc::new(create_test_identity());
        let node_id = NodeId::from_bytes(*identity.node_id.as_bytes());
        let dht = Arc::new(DhtManager::new(identity, DhtConfig::default()));
        let router = MessageRouter::new(node_id, dht, RouterConfig::default());

        let dest = NodeId::from_bytes([2u8; 32]);
        let message = create_test_message(NodeId::from_bytes([1u8; 32]), dest);

        // Cache message
        router.cache_message(message.clone()).await.unwrap();

        // Check cached count
        assert_eq!(router.get_cached_count(&dest).await, 1);

        // Retrieve cached messages
        let cached = router.retrieve_cached_messages(&dest).await;
        assert_eq!(cached.len(), 1);

        // Should be empty after retrieval
        assert_eq!(router.get_cached_count(&dest).await, 0);
    }
}
