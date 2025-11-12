//! K-bucket implementation for Kademlia DHT

use crate::error::Result;
use crate::node_info::NodeInfo;
use crate::K;
use myriadmesh_protocol::NodeId;
use std::collections::VecDeque;

/// A k-bucket for storing nodes at a specific distance
#[derive(Debug, Clone)]
pub struct KBucket {
    /// Bucket index (0-255)
    pub index: usize,

    /// Nodes in this bucket (up to k nodes)
    nodes: VecDeque<NodeInfo>,

    /// Replacement cache for when bucket is full
    replacement_cache: VecDeque<NodeInfo>,

    /// Last time this bucket was updated
    pub last_updated: u64,
}

impl KBucket {
    /// Create a new k-bucket
    pub fn new(index: usize) -> Self {
        KBucket {
            index,
            nodes: VecDeque::with_capacity(K),
            replacement_cache: VecDeque::with_capacity(K),
            last_updated: 0,
        }
    }

    /// Get number of nodes in bucket
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if bucket is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Check if bucket is full
    pub fn is_full(&self) -> bool {
        self.nodes.len() >= K
    }

    /// Get all nodes in bucket
    pub fn nodes(&self) -> &VecDeque<NodeInfo> {
        &self.nodes
    }

    /// Get mutable reference to nodes
    pub fn nodes_mut(&mut self) -> &mut VecDeque<NodeInfo> {
        &mut self.nodes
    }

    /// Find node by ID
    pub fn find_node(&self, node_id: &NodeId) -> Option<&NodeInfo> {
        self.nodes.iter().find(|n| &n.node_id == node_id)
    }

    /// Find node mutably
    pub fn find_node_mut(&mut self, node_id: &NodeId) -> Option<&mut NodeInfo> {
        self.nodes.iter_mut().find(|n| &n.node_id == node_id)
    }

    /// Add or update a node in the bucket
    ///
    /// Returns true if node was added/updated, false if bucket is full
    pub fn add_or_update(&mut self, node: NodeInfo, current_time: u64) -> Result<bool> {
        let node_id = node.node_id;

        // If node already exists, move to back (most recently seen)
        if let Some(pos) = self.nodes.iter().position(|n| n.node_id == node_id) {
            self.nodes.remove(pos);
            self.nodes.push_back(node);
            self.last_updated = current_time;
            return Ok(true);
        }

        // If bucket not full, add node
        if !self.is_full() {
            self.nodes.push_back(node);
            self.last_updated = current_time;
            return Ok(true);
        }

        // Bucket is full - check if we should evict the head
        if let Some(head) = self.nodes.front() {
            // If head node is bad (many failures), replace it
            if head.should_evict(5, 3600) {
                self.nodes.pop_front();
                self.nodes.push_back(node);
                self.last_updated = current_time;
                return Ok(true);
            }
        }

        // Bucket full and head is good - add to replacement cache
        self.add_to_replacement_cache(node);
        Ok(false)
    }

    /// Add node to replacement cache
    fn add_to_replacement_cache(&mut self, node: NodeInfo) {
        let node_id = node.node_id;

        // Remove if already in cache
        if let Some(pos) = self
            .replacement_cache
            .iter()
            .position(|n| n.node_id == node_id)
        {
            self.replacement_cache.remove(pos);
        }

        // Add to back
        self.replacement_cache.push_back(node);

        // Limit cache size
        if self.replacement_cache.len() > K {
            self.replacement_cache.pop_front();
        }
    }

    /// Remove a node from the bucket
    pub fn remove(&mut self, node_id: &NodeId) -> Option<NodeInfo> {
        if let Some(pos) = self.nodes.iter().position(|n| &n.node_id == node_id) {
            let removed = self.nodes.remove(pos);

            // Try to fill from replacement cache
            if let Some(replacement) = self.replacement_cache.pop_front() {
                self.nodes.push_back(replacement);
            }

            removed
        } else {
            None
        }
    }

    /// Remove stale nodes
    pub fn prune_stale(&mut self, max_age_secs: u64) -> usize {
        let mut removed = 0;

        self.nodes.retain(|node| {
            let should_keep = !node.is_stale(max_age_secs);
            if !should_keep {
                removed += 1;
            }
            should_keep
        });

        // Fill from replacement cache
        while !self.is_full() && !self.replacement_cache.is_empty() {
            if let Some(replacement) = self.replacement_cache.pop_front() {
                self.nodes.push_back(replacement);
            }
        }

        removed
    }

    /// Get replacement cache
    pub fn replacement_cache(&self) -> &VecDeque<NodeInfo> {
        &self.replacement_cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node(id: u8) -> NodeInfo {
        NodeInfo::new(NodeId::from_bytes([id; 32]))
    }

    #[test]
    fn test_empty_bucket() {
        let bucket = KBucket::new(0);
        assert!(bucket.is_empty());
        assert!(!bucket.is_full());
        assert_eq!(bucket.len(), 0);
    }

    #[test]
    fn test_add_node() {
        let mut bucket = KBucket::new(0);
        let node = create_test_node(1);

        let added = bucket.add_or_update(node, 0).unwrap();
        assert!(added);
        assert_eq!(bucket.len(), 1);
        assert!(!bucket.is_empty());
    }

    #[test]
    fn test_update_existing_node() {
        let mut bucket = KBucket::new(0);
        let node1 = create_test_node(1);
        let node2 = create_test_node(1); // Same ID

        bucket.add_or_update(node1, 0).unwrap();
        bucket.add_or_update(node2, 1).unwrap();

        // Should still have only 1 node
        assert_eq!(bucket.len(), 1);
        assert_eq!(bucket.last_updated, 1);
    }

    #[test]
    fn test_full_bucket() {
        let mut bucket = KBucket::new(0);

        // Fill bucket
        for i in 0..K {
            let node = create_test_node(i as u8);
            bucket.add_or_update(node, 0).unwrap();
        }

        assert!(bucket.is_full());
        assert_eq!(bucket.len(), K);

        // Try to add another node
        let extra_node = create_test_node(99);
        let added = bucket.add_or_update(extra_node, 0).unwrap();
        assert!(!added); // Should not be added

        // Should be in replacement cache
        assert_eq!(bucket.replacement_cache().len(), 1);
    }

    #[test]
    fn test_remove_node() {
        let mut bucket = KBucket::new(0);
        let node = create_test_node(1);
        let node_id = node.node_id;

        bucket.add_or_update(node, 0).unwrap();
        assert_eq!(bucket.len(), 1);

        let removed = bucket.remove(&node_id);
        assert!(removed.is_some());
        assert_eq!(bucket.len(), 0);
    }

    #[test]
    fn test_find_node() {
        let mut bucket = KBucket::new(0);
        let node = create_test_node(1);
        let node_id = node.node_id;

        bucket.add_or_update(node, 0).unwrap();

        let found = bucket.find_node(&node_id);
        assert!(found.is_some());

        let not_found = bucket.find_node(&NodeId::from_bytes([99; 32]));
        assert!(not_found.is_none());
    }

    #[test]
    fn test_prune_stale() {
        let mut bucket = KBucket::new(0);

        // Add old node
        let mut old_node = create_test_node(1);
        old_node.last_seen = 0; // Very old
        bucket.add_or_update(old_node, 0).unwrap();

        // Add fresh node
        let fresh_node = create_test_node(2);
        bucket.add_or_update(fresh_node, 10000).unwrap();

        // Prune stale nodes (max age 1 hour)
        let pruned = bucket.prune_stale(3600);

        // Old node should be pruned
        assert_eq!(pruned, 1);
        assert_eq!(bucket.len(), 1);
    }

    #[test]
    fn test_replacement_cache_fills_bucket() {
        let mut bucket = KBucket::new(0);

        // Fill bucket
        for i in 0..K {
            let node = create_test_node(i as u8);
            bucket.add_or_update(node, 0).unwrap();
        }

        // Add to replacement cache
        let extra_node = create_test_node(99);
        bucket.add_or_update(extra_node, 0).unwrap();

        assert_eq!(bucket.replacement_cache().len(), 1);

        // Remove a node
        let first_node_id = bucket.nodes().front().unwrap().node_id;
        bucket.remove(&first_node_id);

        // Replacement cache should have filled the spot
        assert_eq!(bucket.len(), K);
        assert_eq!(bucket.replacement_cache().len(), 0);
    }
}
