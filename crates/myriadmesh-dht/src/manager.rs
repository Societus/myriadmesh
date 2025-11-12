//! DHT Manager - Main coordinator for DHT operations

use crate::error::{DhtError, Result};
use crate::node_info::NodeInfo;
use crate::operations::{
    FindNodeRequest, FindNodeResponse, FindValueRequest, FindValueResponse, QueryId, StoreAck,
    StoreRequest,
};
use crate::reputation::ReputationManager;
use crate::routing_table::RoutingTable;
use crate::storage::DhtStorage;
use crate::{ALPHA, K};
use myriadmesh_crypto::identity::NodeIdentity;
use myriadmesh_crypto::signing::sign_message;
use myriadmesh_protocol::NodeId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Get current timestamp
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// DHT Manager configuration
#[derive(Debug, Clone)]
pub struct DhtConfig {
    /// K parameter (bucket size)
    pub k: usize,

    /// Alpha parameter (parallel queries)
    pub alpha: usize,

    /// Bucket refresh interval (seconds)
    pub bucket_refresh_interval: u64,

    /// Value republish interval (seconds)
    pub republish_interval: u64,

    /// Query timeout (milliseconds)
    pub query_timeout_ms: u64,

    /// Maximum concurrent queries
    pub max_concurrent_queries: usize,
}

impl Default for DhtConfig {
    fn default() -> Self {
        DhtConfig {
            k: K,
            alpha: ALPHA,
            bucket_refresh_interval: 3600, // 1 hour
            republish_interval: 3600,      // 1 hour
            query_timeout_ms: 5000,        // 5 seconds
            max_concurrent_queries: 10,
        }
    }
}

/// Pending query state
#[derive(Debug)]
struct PendingQuery {
    query_id: QueryId,
    started_at: u64,
    timeout_ms: u64,
}

/// DHT Manager - Coordinates all DHT operations
pub struct DhtManager {
    /// Local node identity
    identity: Arc<NodeIdentity>,

    /// Routing table
    routing_table: Arc<RwLock<RoutingTable>>,

    /// DHT storage
    storage: Arc<RwLock<DhtStorage>>,

    /// Reputation manager
    reputation: Arc<RwLock<ReputationManager>>,

    /// Configuration
    config: DhtConfig,

    /// Pending queries (query_id -> response receiver)
    pending_queries: Arc<RwLock<HashMap<QueryId, tokio::sync::oneshot::Sender<Vec<u8>>>>>,
}

impl DhtManager {
    /// Create a new DHT manager
    pub fn new(identity: Arc<NodeIdentity>, config: DhtConfig) -> Self {
        // Convert crypto::NodeId to protocol::NodeId
        let local_node_id = NodeId::from_bytes(*identity.node_id.as_bytes());

        DhtManager {
            identity,
            routing_table: Arc::new(RwLock::new(RoutingTable::new(local_node_id))),
            storage: Arc::new(RwLock::new(DhtStorage::new())),
            reputation: Arc::new(RwLock::new(ReputationManager::new())),
            config,
            pending_queries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get local node ID
    pub async fn local_node_id(&self) -> NodeId {
        self.routing_table.read().await.local_node_id().clone()
    }

    /// Add or update a node in the routing table
    pub async fn add_or_update_node(&self, node: NodeInfo) -> Result<()> {
        self.routing_table.write().await.add_or_update(node)
    }

    /// Find a node by ID in routing table
    pub async fn find_node_local(&self, node_id: &NodeId) -> Option<NodeInfo> {
        self.routing_table.read().await.find_node(node_id).cloned()
    }

    /// Iterative FIND_NODE lookup
    ///
    /// Returns k closest nodes to the target
    pub async fn lookup_node(&self, target: NodeId) -> Result<Vec<NodeInfo>> {
        let mut closest = self
            .routing_table
            .read()
            .await
            .get_k_closest(&target, self.config.k);

        if closest.is_empty() {
            return Err(DhtError::NoKnownNodes);
        }

        let mut queried = HashSet::new();
        let alpha = self.config.alpha;

        // Start with alpha parallel queries
        for node in closest.iter().take(alpha) {
            queried.insert(node.node_id);

            // In a real implementation, we would send FIND_NODE requests here
            // For now, we mark this as a placeholder for network integration
            // The actual query sending will be implemented in the MessageRouter integration
        }

        // Iterative lookup process
        let mut iteration = 0;
        const MAX_ITERATIONS: usize = 20;

        while iteration < MAX_ITERATIONS {
            iteration += 1;

            // Find unqueried nodes from closest set
            let to_query: Vec<NodeInfo> = closest
                .iter()
                .filter(|n| !queried.contains(&n.node_id))
                .take(alpha)
                .cloned()
                .collect();

            if to_query.is_empty() {
                break;
            }

            // Query nodes (placeholder - will be integrated with MessageRouter)
            for node in &to_query {
                queried.insert(node.node_id);
                // TODO: Send actual FIND_NODE request via network
            }

            // Sort by distance and keep k closest
            closest.sort_by_key(|n| {
                let dist = target.distance(&n.node_id);
                dist.to_vec()
            });
            closest.truncate(self.config.k);
        }

        Ok(closest)
    }

    /// Iterative FIND_VALUE lookup
    ///
    /// Returns the value if found, or k closest nodes if not
    pub async fn find_value(&self, key: [u8; 32]) -> Result<Option<Vec<u8>>> {
        // First check local storage
        if let Some(entry) = self.storage.read().await.get(&key) {
            if entry.expires_at > now() {
                return Ok(Some(entry.value.clone()));
            }
        }

        // Convert key to NodeId for distance calculations
        let target = NodeId::from_bytes(key);

        // Start iterative lookup
        let mut closest = self
            .routing_table
            .read()
            .await
            .get_k_closest(&target, self.config.k);

        if closest.is_empty() {
            return Err(DhtError::NoKnownNodes);
        }

        let mut queried = HashSet::new();
        let alpha = self.config.alpha;

        // Iterative lookup (placeholder for network integration)
        let mut iteration = 0;
        const MAX_ITERATIONS: usize = 20;

        while iteration < MAX_ITERATIONS {
            iteration += 1;

            let to_query: Vec<NodeInfo> = closest
                .iter()
                .filter(|n| !queried.contains(&n.node_id))
                .take(alpha)
                .cloned()
                .collect();

            if to_query.is_empty() {
                break;
            }

            for node in &to_query {
                queried.insert(node.node_id);
                // TODO: Send FIND_VALUE request via network
                // If value found, return it immediately
            }

            closest.sort_by_key(|n| {
                let dist = target.distance(&n.node_id);
                dist.to_vec()
            });
            closest.truncate(self.config.k);
        }

        // Value not found
        Ok(None)
    }

    /// Store a value in the DHT
    ///
    /// Stores the value at k closest nodes to the key
    pub async fn store(&self, key: [u8; 32], value: Vec<u8>, ttl: u32) -> Result<()> {
        // Validate value size
        if value.len() > crate::MAX_VALUE_SIZE {
            return Err(DhtError::ValueTooLarge {
                size: value.len(),
                max: crate::MAX_VALUE_SIZE,
            });
        }

        // Sign the data
        let data_to_sign = [&key[..], &value].concat();
        let _signature = sign_message(&self.identity, &data_to_sign)
            .map_err(|e| DhtError::Other(format!("Signing failed: {}", e)))?;

        // Store locally first
        let local_node_id = self.local_node_id().await;
        self.storage
            .write()
            .await
            .store(key, value.clone(), ttl as u64, Some(*local_node_id.as_bytes()))?;

        // Find k closest nodes for replication
        let target = NodeId::from_bytes(key);
        let closest = match self.lookup_node(target).await {
            Ok(nodes) => nodes,
            Err(DhtError::NoKnownNodes) => {
                // No known nodes, but we stored locally so that's fine
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        if closest.is_empty() {
            // No nodes to replicate to, but local storage succeeded
            return Ok(());
        }

        // Send STORE requests to k closest nodes (placeholder)
        let mut success_count = 0;
        for _node in &closest {
            // TODO: Send STORE request via network
            // For now, count as success since we stored locally
            success_count += 1;
        }

        if success_count >= (self.config.k / 2) {
            Ok(())
        } else {
            Err(DhtError::StoreFailed)
        }
    }

    /// Handle incoming FIND_NODE request
    pub async fn handle_find_node(&self, request: FindNodeRequest) -> FindNodeResponse {
        // Get k closest nodes to target
        let closest = self
            .routing_table
            .read()
            .await
            .get_k_closest(&request.target, self.config.k);

        // Convert to PublicNodeInfo (privacy protection)
        let public_nodes = closest
            .into_iter()
            .map(|node| node.to_public())
            .collect();

        FindNodeResponse {
            query_id: request.query_id,
            nodes: public_nodes,
        }
    }

    /// Handle incoming FIND_VALUE request
    pub async fn handle_find_value(&self, request: FindValueRequest) -> FindValueResponse {
        // Check if we have the value
        if let Some(entry) = self.storage.read().await.get(&request.key) {
            if entry.expires_at > now() {
                // Value found
                let data_to_sign = [&request.key[..], &entry.value].concat();
                let signature = sign_message(&self.identity, &data_to_sign)
                    .unwrap_or_else(|_| myriadmesh_crypto::signing::Signature::from_bytes([0u8; 64]));

                return FindValueResponse::Found {
                    query_id: request.query_id,
                    key: request.key,
                    value: entry.value.clone(),
                    signature: signature.as_bytes().to_vec(),
                };
            }
        }

        // Value not found, return closer nodes
        let target = NodeId::from_bytes(request.key);
        let closest = self
            .routing_table
            .read()
            .await
            .get_k_closest(&target, self.config.k);

        let public_nodes = closest
            .into_iter()
            .map(|node| node.to_public())
            .collect();

        FindValueResponse::NotFound {
            query_id: request.query_id,
            nodes: public_nodes,
        }
    }

    /// Handle incoming STORE request
    pub async fn handle_store(&self, request: StoreRequest) -> StoreAck {
        // Verify we're responsible for this key (within k closest)
        let target = NodeId::from_bytes(request.key);
        let local_node_id = self.local_node_id().await;
        let closest = self
            .routing_table
            .read()
            .await
            .get_k_closest(&target, self.config.k);

        let is_responsible = closest.iter().any(|n| n.node_id == local_node_id)
            || closest.len() < self.config.k;

        if !is_responsible {
            return StoreAck {
                query_id: request.query_id,
                success: false,
                error: Some("Not responsible for this key".to_string()),
            };
        }

        // Verify signature (placeholder - actual verification needs publisher's public key)
        // TODO: Implement proper signature verification

        // Store the value
        match self
            .storage
            .write()
            .await
            .store(
                request.key,
                request.value,
                request.ttl as u64,
                Some(*request.publisher.as_bytes()),
            ) {
            Ok(()) => StoreAck {
                query_id: request.query_id,
                success: true,
                error: None,
            },
            Err(e) => StoreAck {
                query_id: request.query_id,
                success: false,
                error: Some(e.to_string()),
            },
        }
    }

    /// Record a successful relay for reputation
    pub async fn record_successful_relay(&self, node_id: NodeId) {
        if let Some(node) = self.routing_table.write().await.find_node_mut(&node_id) {
            node.reputation.record_success();
        }
    }

    /// Record a failed relay for reputation
    pub async fn record_failed_relay(&self, node_id: NodeId) {
        if let Some(node) = self.routing_table.write().await.find_node_mut(&node_id) {
            node.reputation.record_failure();
        }
    }

    /// Get nodes with good reputation for relay selection
    pub async fn get_relay_candidates(&self, min_reputation: f64) -> Vec<NodeInfo> {
        self.routing_table
            .read()
            .await
            .get_good_reputation_nodes(min_reputation)
    }

    /// Get routing table statistics
    pub async fn get_stats(&self) -> DhtStats {
        let table = self.routing_table.read().await;
        let storage = self.storage.read().await;

        DhtStats {
            node_count: table.node_count(),
            stored_values: storage.key_count(),
            storage_bytes: storage.size(),
        }
    }

    /// Start maintenance loop
    pub async fn start_maintenance_loop(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                // Sleep first
                sleep(Duration::from_secs(60)).await;

                // Refresh stale buckets
                if let Err(e) = self.refresh_stale_buckets().await {
                    eprintln!("DHT maintenance: refresh_stale_buckets failed: {}", e);
                }

                // Clean up expired values
                self.cleanup_expired_values().await;

                // Prune stale nodes
                self.prune_stale_nodes().await;

                // Republish stored values (every republish_interval)
                // TODO: Implement republish logic
            }
        });
    }

    /// Refresh stale buckets
    async fn refresh_stale_buckets(&self) -> Result<()> {
        let stale_buckets = self
            .routing_table
            .read()
            .await
            .get_stale_buckets(self.config.bucket_refresh_interval);

        for _bucket_idx in stale_buckets {
            // TODO: Generate random node ID in bucket range and perform lookup
            // This will refresh the bucket with new nodes
        }

        Ok(())
    }

    /// Clean up expired values
    async fn cleanup_expired_values(&self) {
        self.storage.write().await.cleanup_expired();
    }

    /// Prune stale nodes from routing table
    async fn prune_stale_nodes(&self) {
        const MAX_NODE_AGE: u64 = 24 * 3600; // 24 hours
        self.routing_table.write().await.prune_stale(MAX_NODE_AGE);
    }
}

/// DHT statistics
#[derive(Debug, Clone, Copy)]
pub struct DhtStats {
    pub node_count: usize,
    pub stored_values: usize,
    pub storage_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_identity() -> NodeIdentity {
        myriadmesh_crypto::init().unwrap();
        NodeIdentity::generate().unwrap()
    }

    #[tokio::test]
    async fn test_dht_manager_creation() {
        let identity = Arc::new(create_test_identity());
        let config = DhtConfig::default();
        let dht = DhtManager::new(identity, config);

        let stats = dht.get_stats().await;
        assert_eq!(stats.node_count, 0);
        assert_eq!(stats.stored_values, 0);
    }

    #[tokio::test]
    async fn test_add_node() {
        let identity = Arc::new(create_test_identity());
        let dht = DhtManager::new(identity, DhtConfig::default());

        let node = NodeInfo::new(NodeId::from_bytes([1u8; 32]));
        dht.add_or_update_node(node.clone()).await.unwrap();

        let stats = dht.get_stats().await;
        assert_eq!(stats.node_count, 1);

        let found = dht.find_node_local(&node.node_id).await;
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_local_storage() {
        let identity = Arc::new(create_test_identity());
        let dht = DhtManager::new(identity, DhtConfig::default());

        let key = [1u8; 32];
        let value = b"test value".to_vec();

        // Store value
        dht.store(key, value.clone(), 3600).await.unwrap();

        // Retrieve value
        let result = dht.find_value(key).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), value);
    }

    #[tokio::test]
    async fn test_reputation_tracking() {
        let identity = Arc::new(create_test_identity());
        let dht = DhtManager::new(identity, DhtConfig::default());

        let node_id = NodeId::from_bytes([1u8; 32]);
        let node = NodeInfo::new(node_id);
        dht.add_or_update_node(node).await.unwrap();

        // Record successful relay
        dht.record_successful_relay(node_id).await;

        let found = dht.find_node_local(&node_id).await.unwrap();
        assert_eq!(found.reputation.successful_relays, 1);
    }

    #[tokio::test]
    async fn test_handle_find_node() {
        let identity = Arc::new(create_test_identity());
        let dht = DhtManager::new(identity, DhtConfig::default());

        // Add some nodes
        for i in 1..=10 {
            let node = NodeInfo::new(NodeId::from_bytes([i; 32]));
            dht.add_or_update_node(node).await.unwrap();
        }

        // Send FIND_NODE request
        let target = NodeId::from_bytes([5u8; 32]);
        let request = FindNodeRequest::new(target, NodeId::from_bytes([99u8; 32]));

        let response = dht.handle_find_node(request).await;
        assert!(!response.nodes.is_empty());
        assert!(response.nodes.len() <= K);
    }

    #[tokio::test]
    async fn test_handle_store() {
        let identity = Arc::new(create_test_identity());
        let dht = DhtManager::new(identity, DhtConfig::default());

        let key = [1u8; 32];
        let value = b"test value".to_vec();
        let publisher = NodeId::from_bytes([99u8; 32]);

        let request = StoreRequest {
            query_id: [1u8; 16],
            key,
            value: value.clone(),
            ttl: 3600,
            publisher,
            signature: vec![0u8; 64],
        };

        let ack = dht.handle_store(request).await;
        assert!(ack.success);

        // Verify it was stored
        let result = dht.find_value(key).await.unwrap();
        assert!(result.is_some());
    }
}
