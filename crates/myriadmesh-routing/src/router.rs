//! Message router - coordinates all routing components

use crate::dedup_cache::DedupCache;
use crate::priority_queue::PriorityQueue;
use crate::rate_limiter::{RateLimiter, RateLimiterConfig};
use myriadmesh_crypto::identity::NodeIdentity;
use myriadmesh_dht::{DhtManager, NodeInfo};
use myriadmesh_protocol::{ContentTag, Frame, Message, MessageId, NodeId, RoutingFlags};
use myriadmesh_protocol::routing::Priority;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Router configuration
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Maximum TTL for messages
    pub max_ttl: u8,

    /// Store-and-forward timeout (seconds)
    pub store_forward_timeout: u64,

    /// Rate limiter configuration
    pub rate_limiter: RateLimiterConfig,

    /// Enable content tag filtering
    pub enable_content_filtering: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            max_ttl: 32,
            store_forward_timeout: 3600, // 1 hour
            rate_limiter: RateLimiterConfig::default(),
            enable_content_filtering: true,
        }
    }
}

/// Message router
pub struct MessageRouter {
    /// Configuration
    config: RouterConfig,

    /// Our identity
    identity: Arc<NodeIdentity>,

    /// DHT manager
    dht: Arc<DhtManager>,

    /// Priority queue for outgoing messages
    outgoing_queue: Arc<PriorityQueue>,

    /// Deduplication cache
    dedup_cache: Arc<DedupCache>,

    /// Rate limiter
    rate_limiter: Arc<RateLimiter>,

    /// Store-and-forward cache (destination -> messages)
    store_forward: Arc<RwLock<HashMap<NodeId, Vec<StoredMessage>>>>,

    /// Content tag filters (tags to block)
    blocked_tags: Arc<RwLock<Vec<ContentTag>>>,
}

/// Stored message for store-and-forward
#[derive(Debug, Clone)]
struct StoredMessage {
    message: Message,
    stored_at: std::time::Instant,
}

impl MessageRouter {
    /// Create a new message router
    pub fn new(identity: Arc<NodeIdentity>, dht: Arc<DhtManager>, config: RouterConfig) -> Self {
        Self {
            config: config.clone(),
            identity,
            dht,
            outgoing_queue: Arc::new(PriorityQueue::new()),
            dedup_cache: Arc::new(DedupCache::new()),
            rate_limiter: Arc::new(RateLimiter::new(config.rate_limiter)),
            store_forward: Arc::new(RwLock::new(HashMap::new())),
            blocked_tags: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Route an incoming message
    pub async fn route_message(&self, message: Message) -> Result<RouteDecision, RoutingError> {
        // Check for duplicates
        if self.dedup_cache.check_and_mark(message.id).await {
            return Ok(RouteDecision::Duplicate);
        }

        // Check TTL
        if message.ttl == 0 {
            return Ok(RouteDecision::Dropped(DropReason::TtlExpired));
        }

        // Check rate limit
        if !self.rate_limiter.check_rate_limit(&message.source).await {
            return Ok(RouteDecision::Dropped(DropReason::RateLimited));
        }

        // Determine routing decision
        let our_node_id = self.dht.local_node_id();

        if message.destination == our_node_id {
            // Message is for us
            Ok(RouteDecision::Deliver)
        } else {
            // Message needs to be forwarded
            self.route_forward(message).await
        }
    }

    /// Route a message for forwarding
    async fn route_forward(&self, mut message: Message) -> Result<RouteDecision, RoutingError> {
        // Check if we should relay this message
        if !self.should_relay(&message).await {
            return Ok(RouteDecision::Dropped(DropReason::Filtered));
        }

        // Decrement TTL
        message.ttl = message.ttl.saturating_sub(1);
        if message.ttl == 0 {
            return Ok(RouteDecision::Dropped(DropReason::TtlExpired));
        }

        // Look up next hop in DHT
        if let Some(next_hop) = self.find_next_hop(&message.destination).await {
            Ok(RouteDecision::Forward {
                next_hop,
                message,
            })
        } else {
            // Node not reachable - store and forward
            self.store_for_later(message.clone()).await;
            Ok(RouteDecision::Stored)
        }
    }

    /// Check if we should relay this message
    async fn should_relay(&self, message: &Message) -> bool {
        // Always relay SENSITIVE messages (user-designated)
        if message.routing_flags.contains(RoutingFlags::SENSITIVE) {
            return true;
        }

        // If message is not relay-filterable, relay it
        if !message.routing_flags.contains(RoutingFlags::RELAY_FILTERABLE) {
            return true;
        }

        // Check content tags against our blocked list
        if self.config.enable_content_filtering {
            let blocked = self.blocked_tags.read().await;
            for tag in &message.content_tags {
                if blocked.contains(tag) {
                    return false;
                }
            }
        }

        true
    }

    /// Find next hop for a destination
    async fn find_next_hop(&self, destination: &NodeId) -> Option<NodeId> {
        // Try to find node in DHT
        if let Some(node) = self.dht.get_node(destination).await {
            // Check if node is reachable
            if node.is_alive(300) {
                // 5 minutes
                return Some(node.node_id);
            }
        }

        // Try k-closest nodes
        let closest = self.dht.get_k_closest(destination, 1).await;
        closest.first().map(|n| n.node_id)
    }

    /// Store message for later delivery
    async fn store_for_later(&self, message: Message) {
        let mut store = self.store_forward.write().await;
        let entry = store.entry(message.destination).or_insert_with(Vec::new);

        entry.push(StoredMessage {
            message,
            stored_at: std::time::Instant::now(),
        });
    }

    /// Check if we have messages for a node and deliver them
    pub async fn deliver_stored_messages(&self, node_id: NodeId) -> Vec<Message> {
        let mut store = self.store_forward.write().await;
        if let Some(messages) = store.remove(&node_id) {
            messages.into_iter().map(|sm| sm.message).collect()
        } else {
            Vec::new()
        }
    }

    /// Send a message (enqueue for sending)
    pub async fn send_message(
        &self,
        destination: NodeId,
        payload: Vec<u8>,
        priority: Priority,
        routing_flags: RoutingFlags,
        content_tags: Vec<ContentTag>,
    ) -> Result<MessageId, RoutingError> {
        // Create message
        let message = Message {
            id: MessageId::generate(),
            source: self.dht.local_node_id(),
            destination,
            message_type: myriadmesh_protocol::MessageType::Data,
            priority,
            ttl: self.config.max_ttl,
            timestamp: chrono::Utc::now().timestamp() as u64,
            sequence: 0, // TODO: Implement sequence tracking
            routing_flags,
            content_tags,
            payload,
        };

        // Sign message
        // TODO: Sign with our identity

        // Create frame
        let frame = Frame::from_message(&message)?;

        // Enqueue
        self.outgoing_queue.enqueue(frame, priority).await?;

        Ok(message.id)
    }

    /// Get next message to send (from priority queue)
    pub async fn get_next_outgoing(&self) -> Option<Frame> {
        self.outgoing_queue.dequeue().await.map(|qm| qm.frame)
    }

    /// Add a blocked content tag
    pub async fn block_content_tag(&self, tag: ContentTag) {
        let mut blocked = self.blocked_tags.write().await;
        if !blocked.contains(&tag) {
            blocked.push(tag);
        }
    }

    /// Remove a blocked content tag
    pub async fn unblock_content_tag(&self, tag: &ContentTag) {
        let mut blocked = self.blocked_tags.write().await;
        blocked.retain(|t| t != tag);
    }

    /// Get queue statistics
    pub async fn queue_stats(&self) -> crate::priority_queue::QueueStats {
        self.outgoing_queue.stats().await
    }

    /// Cleanup old stored messages
    pub async fn cleanup_stored_messages(&self) -> usize {
        let mut store = self.store_forward.write().await;
        let timeout = std::time::Duration::from_secs(self.config.store_forward_timeout);
        let now = std::time::Instant::now();
        let mut removed = 0;

        for (_, messages) in store.iter_mut() {
            let original_len = messages.len();
            messages.retain(|sm| now.duration_since(sm.stored_at) < timeout);
            removed += original_len - messages.len();
        }

        // Remove empty entries
        store.retain(|_, messages| !messages.is_empty());

        removed
    }
}

/// Routing decision
#[derive(Debug, Clone)]
pub enum RouteDecision {
    /// Message is for us - deliver to application
    Deliver,

    /// Forward to next hop
    Forward { next_hop: NodeId, message: Message },

    /// Store for later (destination unreachable)
    Stored,

    /// Drop message
    Dropped(DropReason),

    /// Duplicate message
    Duplicate,
}

/// Reason for dropping a message
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// TTL expired
    TtlExpired,

    /// Rate limited
    RateLimited,

    /// Filtered by content tags
    Filtered,

    /// Invalid message
    Invalid,
}

/// Routing errors
#[derive(Debug, thiserror::Error)]
pub enum RoutingError {
    #[error("Protocol error: {0}")]
    Protocol(#[from] myriadmesh_protocol::ProtocolError),

    #[error("Queue error: {0}")]
    Queue(#[from] crate::priority_queue::QueueError),

    #[error("Message validation failed: {0}")]
    ValidationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use myriadmesh_dht::DhtConfig;

    async fn create_test_router() -> MessageRouter {
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        // Convert crypto NodeId to protocol NodeId
        let protocol_node_id = NodeId::from_bytes(*identity.node_id.as_bytes());
        let dht = Arc::new(DhtManager::new(
            protocol_node_id,
            DhtConfig::default(),
        ));
        MessageRouter::new(identity, dht, RouterConfig::default())
    }

    #[tokio::test]
    async fn test_router_creation() {
        let router = create_test_router().await;
        assert!(router.outgoing_queue.is_empty().await);
    }

    #[tokio::test]
    async fn test_content_tag_blocking() {
        let router = create_test_router().await;

        let tag = ContentTag::new("nsfw").unwrap();
        router.block_content_tag(tag.clone()).await;

        let message = Message {
            id: MessageId::generate(),
            source: NodeId::from_bytes([1; 32]),
            destination: NodeId::from_bytes([2; 32]),
            message_type: myriadmesh_protocol::MessageType::Data,
            priority: Priority::NORMAL,
            ttl: 10,
            timestamp: 0,
            sequence: 0,
            routing_flags: RoutingFlags::RELAY_FILTERABLE,
            content_tags: vec![tag],
            payload: vec![],
        };

        assert!(!router.should_relay(&message).await);
    }

    #[tokio::test]
    async fn test_sensitive_always_relayed() {
        let router = create_test_router().await;

        // Block a tag
        let tag = ContentTag::new("blocked").unwrap();
        router.block_content_tag(tag.clone()).await;

        // But message with SENSITIVE flag should still be relayed
        let message = Message {
            id: MessageId::generate(),
            source: NodeId::from_bytes([1; 32]),
            destination: NodeId::from_bytes([2; 32]),
            message_type: myriadmesh_protocol::MessageType::Data,
            priority: Priority::NORMAL,
            ttl: 10,
            timestamp: 0,
            sequence: 0,
            routing_flags: RoutingFlags::SENSITIVE,
            content_tags: vec![tag],
            payload: vec![],
        };

        assert!(router.should_relay(&message).await);
    }
}
