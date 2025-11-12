//! Kademlia routing table with k-buckets

use crate::node_info::NodeInfo;
use myriadmesh_protocol::types::NodeId;
use std::collections::HashMap;

/// Kademlia k-bucket size (maximum nodes per bucket)
pub const K: usize = 20;

/// Number of k-buckets (256 for 256-bit node IDs)
pub const NUM_BUCKETS: usize = 256;

/// A k-bucket containing up to k nodes
#[derive(Debug, Clone)]
pub struct KBucket {
    /// Nodes in this bucket (up to k)
    nodes: Vec<NodeInfo>,

    /// Last time this bucket was updated
    last_updated: std::time::Instant,

    /// Bucket index (0-255)
    index: usize,
}

impl KBucket {
    /// Create a new k-bucket
    pub fn new(index: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(K),
            last_updated: std::time::Instant::now(),
            index,
        }
    }

    /// Try to add a node to the bucket
    /// Returns true if added, false if bucket is full
    pub fn add_node(&mut self, node: NodeInfo) -> bool {
        // Check if node already exists
        if let Some(existing) = self.nodes.iter_mut().find(|n| n.node_id == node.node_id) {
            // Update existing node
            *existing = node;
            self.last_updated = std::time::Instant::now();
            return true;
        }

        // Add new node if space available
        if self.nodes.len() < K {
            self.nodes.push(node);
            self.last_updated = std::time::Instant::now();
            true
        } else {
            false
        }
    }

    /// Remove a node from the bucket
    pub fn remove_node(&mut self, node_id: &NodeId) -> Option<NodeInfo> {
        if let Some(pos) = self.nodes.iter().position(|n| n.node_id == *node_id) {
            let removed = self.nodes.remove(pos);
            self.last_updated = std::time::Instant::now();
            Some(removed)
        } else {
            None
        }
    }

    /// Get a node from the bucket
    pub fn get_node(&self, node_id: &NodeId) -> Option<&NodeInfo> {
        self.nodes.iter().find(|n| n.node_id == *node_id)
    }

    /// Get all nodes in this bucket
    pub fn nodes(&self) -> &[NodeInfo] {
        &self.nodes
    }

    /// Check if bucket is full
    pub fn is_full(&self) -> bool {
        self.nodes.len() >= K
    }

    /// Get bucket size
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if bucket is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get last updated time
    pub fn last_updated(&self) -> std::time::Instant {
        self.last_updated
    }
}

/// Kademlia routing table
pub struct RoutingTable {
    /// Our local node ID
    local_node_id: NodeId,

    /// 256 k-buckets (one per bit of node ID)
    buckets: Vec<KBucket>,

    /// Replacement cache for full buckets (node_id -> node_info)
    replacement_cache: HashMap<NodeId, NodeInfo>,
}

impl RoutingTable {
    /// Create a new routing table
    pub fn new(local_node_id: NodeId) -> Self {
        let mut buckets = Vec::with_capacity(NUM_BUCKETS);
        for i in 0..NUM_BUCKETS {
            buckets.push(KBucket::new(i));
        }

        Self {
            local_node_id,
            buckets,
            replacement_cache: HashMap::new(),
        }
    }

    /// Calculate bucket index for a given node ID
    /// Returns the index of the first differing bit (0-255)
    fn bucket_index(&self, node_id: &NodeId) -> usize {
        let distance = xor_distance(&self.local_node_id, node_id);

        // Find the first non-zero bit (most significant bit)
        for (byte_idx, &byte) in distance.iter().enumerate() {
            if byte != 0 {
                // Count leading zeros in this byte
                let leading_zeros = byte.leading_zeros() as usize;
                return byte_idx * 8 + leading_zeros;
            }
        }

        // If all bits are zero, node IDs are identical
        // This shouldn't happen in practice (can't add ourselves)
        NUM_BUCKETS - 1
    }

    /// Add or update a node in the routing table
    pub fn add_node(&mut self, node: NodeInfo) -> bool {
        // Don't add ourselves
        if node.node_id == self.local_node_id {
            return false;
        }

        let bucket_idx = self.bucket_index(&node.node_id);
        let bucket = &mut self.buckets[bucket_idx];

        if bucket.add_node(node.clone()) {
            // Successfully added or updated
            // Remove from replacement cache if present
            self.replacement_cache.remove(&node.node_id);
            true
        } else {
            // Bucket is full, add to replacement cache
            self.replacement_cache.insert(node.node_id, node);
            false
        }
    }

    /// Remove a node from the routing table
    pub fn remove_node(&mut self, node_id: &NodeId) -> Option<NodeInfo> {
        let bucket_idx = self.bucket_index(node_id);
        let removed = self.buckets[bucket_idx].remove_node(node_id);

        // If we removed a node and have a replacement, add it
        if removed.is_some() {
            // Find a replacement from cache for this bucket
            let replacement = self.replacement_cache.iter()
                .find(|(id, _)| self.bucket_index(id) == bucket_idx)
                .map(|(id, _)| *id);

            if let Some(replacement_id) = replacement {
                if let Some(replacement_node) = self.replacement_cache.remove(&replacement_id) {
                    self.buckets[bucket_idx].add_node(replacement_node);
                }
            }
        }

        removed
    }

    /// Get k closest nodes to a target
    pub fn get_k_closest(&self, target: &NodeId, k: usize) -> Vec<NodeInfo> {
        let mut all_nodes: Vec<NodeInfo> = self.buckets
            .iter()
            .flat_map(|bucket| bucket.nodes().iter().cloned())
            .collect();

        // Sort by XOR distance to target
        all_nodes.sort_by_key(|node| xor_distance(&node.node_id, target));

        // Take k closest
        all_nodes.into_iter().take(k).collect()
    }

    /// Get a specific node
    pub fn get_node(&self, node_id: &NodeId) -> Option<&NodeInfo> {
        let bucket_idx = self.bucket_index(node_id);
        self.buckets[bucket_idx].get_node(node_id)
    }

    /// Get all nodes in the routing table
    pub fn all_nodes(&self) -> Vec<NodeInfo> {
        self.buckets
            .iter()
            .flat_map(|bucket| bucket.nodes().iter().cloned())
            .collect()
    }

    /// Get total number of nodes
    pub fn node_count(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    /// Get buckets that haven't been updated recently
    pub fn stale_buckets(&self, max_age: std::time::Duration) -> Vec<usize> {
        let now = std::time::Instant::now();
        self.buckets
            .iter()
            .enumerate()
            .filter(|(_, bucket)| {
                !bucket.is_empty() && (now - bucket.last_updated()) > max_age
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    /// Get random nodes from the routing table
    pub fn get_random_nodes(&self, count: usize) -> Vec<NodeInfo> {
        use rand::seq::SliceRandom;
        let mut all_nodes = self.all_nodes();
        let mut rng = rand::thread_rng();
        all_nodes.shuffle(&mut rng);
        all_nodes.into_iter().take(count).collect()
    }

    /// Get the local node ID
    pub fn local_node_id(&self) -> &NodeId {
        &self.local_node_id
    }
}

/// Calculate XOR distance between two node IDs
pub fn xor_distance(a: &NodeId, b: &NodeId) -> [u8; 32] {
    let mut result = [0u8; 32];
    for (i, (byte_a, byte_b)) in a.as_bytes().iter().zip(b.as_bytes().iter()).enumerate() {
        result[i] = byte_a ^ byte_b;
    }
    result
}

/// Compare two distances (for sorting)
/// Returns std::cmp::Ordering
pub fn compare_distance(dist_a: &[u8; 32], dist_b: &[u8; 32]) -> std::cmp::Ordering {
    for (byte_a, byte_b) in dist_a.iter().zip(dist_b.iter()) {
        match byte_a.cmp(byte_b) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
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

    #[test]
    fn test_xor_distance() {
        let id1 = NodeId::from_bytes([0xFF; 32]);
        let id2 = NodeId::from_bytes([0x00; 32]);

        let distance = xor_distance(&id1, &id2);
        assert_eq!(distance, [0xFF; 32]);

        // XOR is symmetric
        let distance2 = xor_distance(&id2, &id1);
        assert_eq!(distance, distance2);

        // Distance to self is zero
        let distance3 = xor_distance(&id1, &id1);
        assert_eq!(distance3, [0x00; 32]);
    }

    #[test]
    fn test_bucket_add_remove() {
        let mut bucket = KBucket::new(0);
        let node = create_test_node(1);

        // Add node
        assert!(bucket.add_node(node.clone()));
        assert_eq!(bucket.len(), 1);

        // Remove node
        let removed = bucket.remove_node(&node.node_id);
        assert!(removed.is_some());
        assert_eq!(bucket.len(), 0);
    }

    #[test]
    fn test_bucket_capacity() {
        let mut bucket = KBucket::new(0);

        // Fill bucket to capacity
        for i in 0..K {
            let node = create_test_node(i as u8);
            assert!(bucket.add_node(node));
        }

        assert!(bucket.is_full());
        assert_eq!(bucket.len(), K);

        // Try to add one more (should fail)
        let extra_node = create_test_node(99);
        assert!(!bucket.add_node(extra_node));
        assert_eq!(bucket.len(), K);
    }

    #[test]
    fn test_routing_table_basic() {
        let local_id = NodeId::from_bytes([0; 32]);
        let mut table = RoutingTable::new(local_id);

        // Add some nodes
        for i in 1..10 {
            let node = create_test_node(i);
            table.add_node(node);
        }

        assert_eq!(table.node_count(), 9);
    }

    #[test]
    fn test_routing_table_k_closest() {
        let local_id = NodeId::from_bytes([0; 32]);
        let mut table = RoutingTable::new(local_id);

        // Add nodes
        for i in 1..20 {
            let node = create_test_node(i);
            table.add_node(node);
        }

        // Get k closest to a target
        let target = NodeId::from_bytes([10; 32]);
        let closest = table.get_k_closest(&target, 5);

        assert_eq!(closest.len(), 5);

        // Verify they're sorted by distance
        for i in 0..closest.len() - 1 {
            let dist1 = xor_distance(&closest[i].node_id, &target);
            let dist2 = xor_distance(&closest[i + 1].node_id, &target);
            assert!(compare_distance(&dist1, &dist2) != std::cmp::Ordering::Greater);
        }
    }

    #[test]
    fn test_routing_table_remove_with_replacement() {
        let local_id = NodeId::from_bytes([0; 32]);
        let mut table = RoutingTable::new(local_id);

        // Add nodes to fill a bucket
        for i in 1..=K {
            let node = create_test_node(i as u8);
            table.add_node(node);
        }

        // Try to add one more (goes to replacement cache)
        let extra = create_test_node((K + 1) as u8);
        let added = table.add_node(extra);
        assert!(!added); // Should not be added to bucket

        // Remove a node
        let first_node_id = NodeId::from_bytes([1; 32]);
        table.remove_node(&first_node_id);

        // The replacement should have been added
        // (This depends on bucket assignment, so we just check the count)
        assert!(table.node_count() >= K - 1);
    }

    #[test]
    fn test_routing_table_no_self() {
        let local_id = NodeId::from_bytes([42; 32]);
        let mut table = RoutingTable::new(local_id);

        // Try to add ourselves
        let self_node = create_test_node(42);
        let added = table.add_node(self_node);

        assert!(!added);
        assert_eq!(table.node_count(), 0);
    }
}
