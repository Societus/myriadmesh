//! Kademlia routing table

use crate::error::Result;
use crate::kbucket::KBucket;
use crate::node_info::NodeInfo;
use myriadmesh_protocol::NodeId;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Kademlia routing table
#[derive(Debug, Clone)]
pub struct RoutingTable {
    /// Our local node ID
    local_node_id: NodeId,

    /// 256 k-buckets (one per bit of node ID distance)
    buckets: Vec<KBucket>,

    /// Total nodes in routing table
    node_count: usize,
}

impl RoutingTable {
    /// Create a new routing table
    pub fn new(local_node_id: NodeId) -> Self {
        let mut buckets = Vec::with_capacity(256);
        for i in 0..256 {
            buckets.push(KBucket::new(i));
        }

        RoutingTable {
            local_node_id,
            buckets,
            node_count: 0,
        }
    }

    /// Get our local node ID
    pub fn local_node_id(&self) -> &NodeId {
        &self.local_node_id
    }

    /// Get total number of nodes in routing table
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Calculate bucket index for a node ID
    ///
    /// Bucket index is based on the position of the first differing bit:
    /// - Bucket 0: MSB of first byte differs (most distant)
    /// - Bucket 255: LSB of last byte differs (closest)
    fn bucket_index(&self, node_id: &NodeId) -> usize {
        let distance = self.local_node_id.distance(node_id);

        // Find the first non-zero byte
        for (byte_idx, &byte) in distance.iter().enumerate() {
            if byte != 0 {
                // Find the position of the most significant bit within the byte
                let msb_pos = byte.leading_zeros() as usize;
                // Calculate bucket index
                return byte_idx * 8 + msb_pos;
            }
        }

        // All bits are the same (shouldn't happen for different nodes)
        255
    }

    /// Add or update a node in the routing table
    ///
    /// SECURITY C2: Verifies Proof-of-Work before admitting nodes to prevent Sybil attacks
    pub fn add_or_update(&mut self, node: NodeInfo) -> Result<()> {
        // Don't add ourselves
        if node.node_id == self.local_node_id {
            return Ok(());
        }

        // SECURITY C2: Verify Proof-of-Work to prevent Sybil attacks
        if !node.verify_pow() {
            return Err(crate::error::DhtError::InvalidProofOfWork(format!(
                "Node {} has invalid PoW nonce {}",
                hex::encode(node.node_id.as_bytes()),
                node.pow_nonce
            )));
        }

        let bucket_idx = self.bucket_index(&node.node_id);
        let bucket = &mut self.buckets[bucket_idx];

        let was_present = bucket.find_node(&node.node_id).is_some();
        let added = bucket.add_or_update(node, now())?;

        // Update node count
        if added && !was_present {
            self.node_count += 1;
        }

        Ok(())
    }

    /// Find a node by ID
    pub fn find_node(&self, node_id: &NodeId) -> Option<&NodeInfo> {
        if node_id == &self.local_node_id {
            return None; // Don't return ourselves
        }

        let bucket_idx = self.bucket_index(node_id);
        self.buckets[bucket_idx].find_node(node_id)
    }

    /// Find a node mutably
    pub fn find_node_mut(&mut self, node_id: &NodeId) -> Option<&mut NodeInfo> {
        if node_id == &self.local_node_id {
            return None;
        }

        let bucket_idx = self.bucket_index(node_id);
        self.buckets[bucket_idx].find_node_mut(node_id)
    }

    /// Remove a node from the routing table
    pub fn remove(&mut self, node_id: &NodeId) -> Option<NodeInfo> {
        let bucket_idx = self.bucket_index(node_id);
        if let Some(node) = self.buckets[bucket_idx].remove(node_id) {
            self.node_count -= 1;
            Some(node)
        } else {
            None
        }
    }

    /// Get k closest nodes to a target
    pub fn get_k_closest(&self, target: &NodeId, k: usize) -> Vec<NodeInfo> {
        let mut all_nodes: Vec<NodeInfo> = Vec::new();

        // Collect all nodes
        for bucket in &self.buckets {
            all_nodes.extend(bucket.nodes().iter().cloned());
        }

        // Sort by distance to target
        all_nodes.sort_by_key(|node| {
            let dist = target.distance(&node.node_id);
            dist.to_vec() // Convert to Vec for comparison
        });

        // Take first k
        all_nodes.into_iter().take(k).collect()
    }

    /// Get random nodes from routing table
    pub fn get_random_nodes(&self, count: usize) -> Vec<NodeInfo> {
        use rand::seq::SliceRandom;

        let mut all_nodes: Vec<NodeInfo> = Vec::new();

        // Collect all nodes
        for bucket in &self.buckets {
            all_nodes.extend(bucket.nodes().iter().cloned());
        }

        // Shuffle and take
        let mut rng = rand::thread_rng();
        all_nodes.shuffle(&mut rng);

        all_nodes.into_iter().take(count).collect()
    }

    /// Get nodes with good reputation for relay
    pub fn get_good_reputation_nodes(&self, min_reputation: f64) -> Vec<NodeInfo> {
        let mut nodes = Vec::new();

        for bucket in &self.buckets {
            for node in bucket.nodes().iter() {
                if node.reputation.score() >= min_reputation {
                    nodes.push(node.clone());
                }
            }
        }

        nodes
    }

    /// Prune stale nodes from all buckets
    pub fn prune_stale(&mut self, max_age_secs: u64) -> usize {
        let mut total_pruned = 0;

        for bucket in &mut self.buckets {
            let pruned = bucket.prune_stale(max_age_secs);
            total_pruned += pruned;
            self.node_count -= pruned;
        }

        total_pruned
    }

    /// Get buckets that need refreshing
    pub fn get_stale_buckets(&self, max_age_secs: u64) -> Vec<usize> {
        let current_time = now();
        let mut stale = Vec::new();

        for bucket in &self.buckets {
            if !bucket.is_empty() {
                let age = current_time.saturating_sub(bucket.last_updated);
                if age > max_age_secs {
                    stale.push(bucket.index);
                }
            }
        }

        stale
    }

    /// Get all nodes in routing table
    pub fn get_all_nodes(&self) -> Vec<NodeInfo> {
        let mut all_nodes = Vec::new();

        for bucket in &self.buckets {
            all_nodes.extend(bucket.nodes().iter().cloned());
        }

        all_nodes
    }

    /// Get bucket by index
    pub fn get_bucket(&self, index: usize) -> Option<&KBucket> {
        self.buckets.get(index)
    }

    /// Get mutable bucket by index
    pub fn get_bucket_mut(&mut self, index: usize) -> Option<&mut KBucket> {
        self.buckets.get_mut(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myriadmesh_protocol::types::NODE_ID_SIZE;

    fn create_test_node(id: u8) -> NodeInfo {
        let mut node = NodeInfo::new(NodeId::from_bytes([id; NODE_ID_SIZE]));
        // SECURITY C2: Compute valid PoW for test nodes
        node.compute_pow();
        node
    }

    #[test]
    fn test_new_routing_table() {
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let table = RoutingTable::new(local_id);

        assert_eq!(table.node_count(), 0);
        assert_eq!(table.local_node_id(), &local_id);
    }

    #[test]
    fn test_add_node() {
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let mut table = RoutingTable::new(local_id);

        let node = create_test_node(1);
        table.add_or_update(node.clone()).unwrap();

        assert_eq!(table.node_count(), 1);
        assert!(table.find_node(&node.node_id).is_some());
    }

    #[test]
    fn test_dont_add_self() {
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let mut table = RoutingTable::new(local_id);

        let self_node = NodeInfo::new(local_id);
        table.add_or_update(self_node).unwrap();

        assert_eq!(table.node_count(), 0);
    }

    #[test]
    fn test_remove_node() {
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let mut table = RoutingTable::new(local_id);

        let node = create_test_node(1);
        let node_id = node.node_id;

        table.add_or_update(node).unwrap();
        assert_eq!(table.node_count(), 1);

        let removed = table.remove(&node_id);
        assert!(removed.is_some());
        assert_eq!(table.node_count(), 0);
    }

    #[test]
    fn test_get_k_closest() {
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let mut table = RoutingTable::new(local_id);

        // Add several nodes
        for i in 1..=10 {
            let node = create_test_node(i);
            table.add_or_update(node).unwrap();
        }

        let target = NodeId::from_bytes([5; NODE_ID_SIZE]);
        let closest = table.get_k_closest(&target, 3);

        assert_eq!(closest.len(), 3);
    }

    #[test]
    fn test_get_random_nodes() {
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let mut table = RoutingTable::new(local_id);

        // Add several nodes
        for i in 1..=10 {
            let node = create_test_node(i);
            table.add_or_update(node).unwrap();
        }

        let random = table.get_random_nodes(3);
        assert_eq!(random.len(), 3);
    }

    #[test]
    fn test_bucket_index() {
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let table = RoutingTable::new(local_id);

        // Node with first bit different
        let mut node_id_bytes = [0u8; NODE_ID_SIZE];
        node_id_bytes[0] = 0b1000_0000;
        let node_id = NodeId::from_bytes(node_id_bytes);

        let bucket_idx = table.bucket_index(&node_id);
        assert_eq!(bucket_idx, 0); // First bit different
    }

    #[test]
    fn test_prune_stale() {
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let mut table = RoutingTable::new(local_id);

        // Add old node
        let mut old_node = create_test_node(1);
        old_node.last_seen = 0; // Very old
        table.add_or_update(old_node).unwrap();

        // Add fresh node
        let fresh_node = create_test_node(2);
        table.add_or_update(fresh_node).unwrap();

        assert_eq!(table.node_count(), 2);

        // Prune stale
        let pruned = table.prune_stale(3600);
        assert_eq!(pruned, 1);
        assert_eq!(table.node_count(), 1);
    }

    // SECURITY C2: Proof-of-Work enforcement tests

    #[test]
    fn test_reject_node_without_valid_pow() {
        // SECURITY C2: Verify routing table rejects nodes without valid PoW
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let mut table = RoutingTable::new(local_id);

        // Create node with invalid PoW
        let mut invalid_node = NodeInfo::new(NodeId::from_bytes([42; NODE_ID_SIZE]));
        invalid_node.pow_nonce = 12345; // Arbitrary invalid nonce

        // Should be rejected
        let result = table.add_or_update(invalid_node);
        assert!(result.is_err());
        assert_eq!(table.node_count(), 0);
    }

    #[test]
    fn test_accept_node_with_valid_pow() {
        // SECURITY C2: Verify routing table accepts nodes with valid PoW
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let mut table = RoutingTable::new(local_id);

        // Create node and compute valid PoW
        let valid_node = create_test_node(99);

        // Should be accepted
        let result = table.add_or_update(valid_node);
        assert!(result.is_ok());
        assert_eq!(table.node_count(), 1);
    }

    #[test]
    fn test_pow_prevents_sybil_flooding() {
        // SECURITY C2: PoW makes it expensive to flood DHT with many identities
        let local_id = NodeId::from_bytes([0; NODE_ID_SIZE]);
        let mut table = RoutingTable::new(local_id);

        // Try to add 10 nodes with invalid PoW (should all fail)
        for i in 1..=10 {
            let mut invalid_node = NodeInfo::new(NodeId::from_bytes([i; NODE_ID_SIZE]));
            invalid_node.pow_nonce = i as u64 * 1000; // Invalid nonces

            let result = table.add_or_update(invalid_node);
            assert!(
                result.is_err(),
                "Node {} with invalid PoW should be rejected",
                i
            );
        }

        // No nodes should have been added
        assert_eq!(table.node_count(), 0);

        // Now add legitimate nodes with valid PoW
        for i in 1..=3 {
            let valid_node = create_test_node(i);
            table.add_or_update(valid_node).unwrap();
        }

        // Only legitimate nodes added
        assert_eq!(table.node_count(), 3);
    }
}
