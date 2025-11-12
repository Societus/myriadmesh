//! DHT Manager - coordinates routing table, storage, and operations

use crate::node_info::{NodeInfo, NodeReputation};
use crate::operations::*;
use crate::routing_table::{RoutingTable, K};
use crate::storage::{DhtStorage, StoredValue, StorageError};
use myriadmesh_protocol::types::NodeId;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Configuration for DHT
#[derive(Debug, Clone)]
pub struct DhtConfig {
    /// K-bucket size
    pub k: usize,

    /// Alpha (parallel queries)
    pub alpha: usize,

    /// Bucket refresh interval
    pub bucket_refresh_interval: Duration,

    /// Value republish interval
    pub republish_interval: Duration,

    /// Node timeout (mark as dead)
    pub node_timeout: Duration,
}

impl Default for DhtConfig {
    fn default() -> Self {
        Self {
            k: K,
            alpha: 3,
            bucket_refresh_interval: Duration::from_secs(3600), // 1 hour
            republish_interval: Duration::from_secs(3600),       // 1 hour
            node_timeout: Duration::from_secs(300),              // 5 minutes
        }
    }
}

/// DHT Manager
pub struct DhtManager {
    /// Configuration
    config: DhtConfig,

    /// Routing table
    routing_table: Arc<RwLock<RoutingTable>>,

    /// DHT storage
    storage: Arc<RwLock<DhtStorage>>,

    /// Our local node ID
    local_node_id: NodeId,
}

impl DhtManager {
    /// Create a new DHT manager
    pub fn new(local_node_id: NodeId, config: DhtConfig) -> Self {
        Self {
            config,
            routing_table: Arc::new(RwLock::new(RoutingTable::new(local_node_id))),
            storage: Arc::new(RwLock::new(DhtStorage::new())),
            local_node_id,
        }
    }

    /// Get local node ID
    pub fn local_node_id(&self) -> NodeId {
        self.local_node_id
    }

    /// Add a node to the routing table
    pub async fn add_node(&self, node: NodeInfo) -> bool {
        let mut table = self.routing_table.write().await;
        table.add_node(node)
    }

    /// Remove a node from the routing table
    pub async fn remove_node(&self, node_id: &NodeId) -> Option<NodeInfo> {
        let mut table = self.routing_table.write().await;
        table.remove_node(node_id)
    }

    /// Get a node from the routing table
    pub async fn get_node(&self, node_id: &NodeId) -> Option<NodeInfo> {
        let table = self.routing_table.read().await;
        table.get_node(node_id).cloned()
    }

    /// Get k closest nodes to a target
    pub async fn get_k_closest(&self, target: &NodeId, k: usize) -> Vec<NodeInfo> {
        let table = self.routing_table.read().await;
        table.get_k_closest(target, k)
    }

    /// Get all nodes
    pub async fn all_nodes(&self) -> Vec<NodeInfo> {
        let table = self.routing_table.read().await;
        table.all_nodes()
    }

    /// Get node count
    pub async fn node_count(&self) -> usize {
        let table = self.routing_table.read().await;
        table.node_count()
    }

    /// Handle FIND_NODE request
    pub async fn handle_find_node(&self, request: FindNodeRequest) -> FindNodeResponse {
        let nodes = self.get_k_closest(&request.target, self.config.k).await;

        FindNodeResponse {
            query_id: request.query_id,
            nodes,
        }
    }

    /// Handle STORE request
    pub async fn handle_store(&self, request: StoreRequest) -> StoreResponse {
        let mut storage = self.storage.write().await;

        // Create stored value
        let value = StoredValue::new(
            request.value,
            request.ttl as i64,
            *request.publisher.as_bytes(),
            request.signature,
        );

        match storage.insert(request.key, value) {
            Ok(()) => StoreResponse {
                query_id: request.query_id,
                success: true,
                error: None,
            },
            Err(e) => StoreResponse {
                query_id: request.query_id,
                success: false,
                error: Some(e.to_string()),
            },
        }
    }

    /// Handle FIND_VALUE request
    pub async fn handle_find_value(&self, request: FindValueRequest) -> FindValueResponse {
        let storage = self.storage.read().await;

        if let Some(stored) = storage.get(&request.key) {
            // Value found
            FindValueResponse::Found {
                query_id: request.query_id,
                value: stored.data.clone(),
                publisher: NodeId::from_bytes(stored.publisher),
                signature: stored.signature.clone(),
            }
        } else {
            // Value not found, return closest nodes
            drop(storage); // Release read lock
            let target = NodeId::from_bytes(request.key);
            let nodes = self.get_k_closest(&target, self.config.k).await;

            FindValueResponse::NotFound {
                query_id: request.query_id,
                nodes,
            }
        }
    }

    /// Handle PING request
    pub async fn handle_ping(&self, request: PingRequest) -> PongResponse {
        PongResponse {
            query_id: request.query_id,
            responder: self.local_node_id,
        }
    }

    /// Store a value in the DHT (client-side)
    pub async fn store_value(
        &self,
        key: [u8; 32],
        value: Vec<u8>,
        ttl: u32,
        signature: Vec<u8>,
    ) -> Result<(), StorageError> {
        // Store locally first
        let stored = StoredValue::new(value, ttl as i64, *self.local_node_id.as_bytes(), signature);

        let mut storage = self.storage.write().await;
        storage.insert(key, stored)?;

        // TODO: In a full implementation, we would also:
        // 1. Find k-closest nodes to the key
        // 2. Send STORE requests to those nodes
        // 3. Wait for majority to succeed

        Ok(())
    }

    /// Find a value in the DHT (client-side)
    pub async fn find_value(&self, key: [u8; 32]) -> Option<Vec<u8>> {
        // Check local storage first
        let storage = self.storage.read().await;
        if let Some(stored) = storage.get(&key) {
            return Some(stored.data.clone());
        }
        drop(storage);

        // TODO: In a full implementation, we would:
        // 1. Do iterative lookup to find k-closest nodes
        // 2. Query them for the value
        // 3. Return first successful response

        None
    }

    /// Perform iterative node lookup
    pub async fn lookup_node(&self, target: NodeId) -> Vec<NodeInfo> {
        let initial = self.get_k_closest(&target, self.config.k).await;
        let mut lookup = IterativeLookup::new(target, initial, self.config.k, self.config.alpha);

        // TODO: In a full implementation, we would:
        // 1. Query nodes in parallel
        // 2. Process responses
        // 3. Continue until no closer nodes found

        lookup.results()
    }

    /// Record successful relay for reputation
    pub async fn record_successful_relay(&self, node_id: NodeId) {
        let mut table = self.routing_table.write().await;
        if let Some(node) = table.get_node(&node_id) {
            let mut updated = node.clone();
            updated.reputation.record_success();
            table.add_node(updated);
        }
    }

    /// Record failed relay for reputation
    pub async fn record_failed_relay(&self, node_id: NodeId) {
        let mut table = self.routing_table.write().await;
        if let Some(node) = table.get_node(&node_id) {
            let mut updated = node.clone();
            updated.reputation.record_failure();
            table.add_node(updated);
        }
    }

    /// Get high reputation nodes for relay selection
    pub async fn get_high_reputation_nodes(&self, min_reputation: f64) -> Vec<NodeInfo> {
        let table = self.routing_table.read().await;
        table
            .all_nodes()
            .into_iter()
            .filter(|n| n.reputation.score >= min_reputation)
            .collect()
    }

    /// Cleanup expired storage values
    pub async fn cleanup_expired(&self) -> usize {
        let mut storage = self.storage.write().await;
        storage.cleanup_expired()
    }

    /// Get storage statistics
    pub async fn storage_stats(&self) -> (usize, usize) {
        let storage = self.storage.read().await;
        (storage.key_count(), storage.size())
    }

    /// Maintenance task - should be run periodically
    pub async fn maintenance(&self) {
        // Cleanup expired values
        self.cleanup_expired().await;

        // TODO: In a full implementation, we would also:
        // 1. Refresh stale buckets
        // 2. Republish stored values
        // 3. Health check nodes
        // 4. Update reputation scores
    }

    /// Get stale buckets that need refreshing
    pub async fn stale_buckets(&self) -> Vec<usize> {
        let table = self.routing_table.read().await;
        table.stale_buckets(self.config.bucket_refresh_interval)
    }

    /// Get random nodes (for bootstrapping or sampling)
    pub async fn get_random_nodes(&self, count: usize) -> Vec<NodeInfo> {
        let table = self.routing_table.read().await;
        table.get_random_nodes(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_info::AdapterInfo;
    use myriadmesh_protocol::types::AdapterType;

    fn create_test_node(id_byte: u8) -> NodeInfo {
        NodeInfo::new(
            NodeId::from_bytes([id_byte; 32]),
            vec![AdapterInfo {
                adapter_type: AdapterType::Ethernet,
                address: format!("192.168.1.{}", id_byte),
                active: true,
            }],
        )
    }

    #[tokio::test]
    async fn test_dht_manager_basic() {
        let local_id = NodeId::from_bytes([0; 32]);
        let config = DhtConfig::default();
        let manager = DhtManager::new(local_id, config);

        assert_eq!(manager.local_node_id(), local_id);
        assert_eq!(manager.node_count().await, 0);
    }

    #[tokio::test]
    async fn test_add_remove_node() {
        let local_id = NodeId::from_bytes([0; 32]);
        let config = DhtConfig::default();
        let manager = DhtManager::new(local_id, config);

        let node = create_test_node(1);
        let node_id = node.node_id;

        assert!(manager.add_node(node).await);
        assert_eq!(manager.node_count().await, 1);

        let removed = manager.remove_node(&node_id).await;
        assert!(removed.is_some());
        assert_eq!(manager.node_count().await, 0);
    }

    #[tokio::test]
    async fn test_k_closest() {
        let local_id = NodeId::from_bytes([0; 32]);
        let config = DhtConfig::default();
        let manager = DhtManager::new(local_id, config);

        // Add some nodes
        for i in 1..10 {
            manager.add_node(create_test_node(i)).await;
        }

        let target = NodeId::from_bytes([5; 32]);
        let closest = manager.get_k_closest(&target, 5).await;

        assert_eq!(closest.len(), 5);
    }

    #[tokio::test]
    async fn test_store_and_find() {
        let local_id = NodeId::from_bytes([0; 32]);
        let config = DhtConfig::default();
        let manager = DhtManager::new(local_id, config);

        let key = [1u8; 32];
        let value = b"test data".to_vec();
        let signature = vec![2u8; 64];

        manager
            .store_value(key, value.clone(), 3600, signature)
            .await
            .unwrap();

        let found = manager.find_value(key).await;
        assert_eq!(found, Some(value));
    }

    #[tokio::test]
    async fn test_reputation_tracking() {
        let local_id = NodeId::from_bytes([0; 32]);
        let config = DhtConfig::default();
        let manager = DhtManager::new(local_id, config);

        let node = create_test_node(1);
        let node_id = node.node_id;
        manager.add_node(node).await;

        // Record successes
        for _ in 0..10 {
            manager.record_successful_relay(node_id).await;
        }

        let updated = manager.get_node(&node_id).await.unwrap();
        assert!(updated.reputation.score > 0.5);
    }

    #[tokio::test]
    async fn test_handle_ping() {
        let local_id = NodeId::from_bytes([0; 32]);
        let config = DhtConfig::default();
        let manager = DhtManager::new(local_id, config);

        let request = PingRequest {
            query_id: [1u8; 16],
            requester: NodeId::from_bytes([2; 32]),
        };

        let response = manager.handle_ping(request).await;
        assert_eq!(response.query_id, [1u8; 16]);
        assert_eq!(response.responder, local_id);
    }
}
