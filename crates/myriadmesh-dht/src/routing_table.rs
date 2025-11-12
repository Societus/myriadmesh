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
    /// Bucket index is the position of the most significant differing bit
    fn bucket_index(&self, node_id: &NodeId) -> usize {
        let distance = self.local_node_id.distance(node_id);

        // Find the first non-zero byte
        for (byte_idx, &byte) in distance.iter().enumerate() {
            if byte != 0 {
                // Find the position of the most significant bit
                let bit_pos = 7 - byte.leading_zeros() as usize;
                return byte_idx * 8 + bit_pos;
            }
        }

        // All bits are the same (shouldn't happen for different nodes)
        0
    }

    /// Add or update a node in the routing table
    pub fn add_or_update(&mut self, node: NodeInfo) -> Result<()> {
        // Don't add ourselves
        if node.node_id == self.local_node_id {
            return Ok(());
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

    fn create_test_node(id: u8) -> NodeInfo {
        NodeInfo::new(NodeId::from_bytes([id; 32]))
    }

    #[test]
    fn test_new_routing_table() {
        let local_id = NodeId::from_bytes([0; 32]);
        let table = RoutingTable::new(local_id);

        assert_eq!(table.node_count(), 0);
        assert_eq!(table.local_node_id(), &local_id);
    }

    #[test]
    fn test_add_node() {
        let local_id = NodeId::from_bytes([0; 32]);
        let mut table = RoutingTable::new(local_id);

        let node = create_test_node(1);
        table.add_or_update(node.clone()).unwrap();

        assert_eq!(table.node_count(), 1);
        assert!(table.find_node(&node.node_id).is_some());
    }

    #[test]
    fn test_dont_add_self() {
        let local_id = NodeId::from_bytes([0; 32]);
        let mut table = RoutingTable::new(local_id);

        let self_node = NodeInfo::new(local_id);
        table.add_or_update(self_node).unwrap();

        assert_eq!(table.node_count(), 0);
    }

    #[test]
    fn test_remove_node() {
        let local_id = NodeId::from_bytes([0; 32]);
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
        let local_id = NodeId::from_bytes([0; 32]);
        let mut table = RoutingTable::new(local_id);

        // Add several nodes
        for i in 1..=10 {
            let node = create_test_node(i);
            table.add_or_update(node).unwrap();
        }

        let target = NodeId::from_bytes([5; 32]);
        let closest = table.get_k_closest(&target, 3);

        assert_eq!(closest.len(), 3);
    }

    #[test]
    fn test_get_random_nodes() {
        let local_id = NodeId::from_bytes([0; 32]);
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
        let local_id = NodeId::from_bytes([0; 32]);
        let table = RoutingTable::new(local_id);

        // Node with first bit different
        let mut node_id_bytes = [0u8; 32];
        node_id_bytes[0] = 0b1000_0000;
        let node_id = NodeId::from_bytes(node_id_bytes);

        let bucket_idx = table.bucket_index(&node_id);
        assert_eq!(bucket_idx, 0); // First bit different
    }

    #[test]
    fn test_prune_stale() {
        let local_id = NodeId::from_bytes([0; 32]);
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
}
