//! DHT operations (FIND_NODE, STORE, FIND_VALUE)

use crate::node_info::NodeInfo;
use myriadmesh_protocol::types::NodeId;
use serde::{Deserialize, Serialize};

/// Query ID for tracking requests/responses
pub type QueryId = [u8; 16];

/// DHT message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtMessage {
    /// Find nodes close to a target
    FindNode(FindNodeRequest),
    FindNodeResponse(FindNodeResponse),

    /// Store a value
    Store(StoreRequest),
    StoreResponse(StoreResponse),

    /// Find a value
    FindValue(FindValueRequest),
    FindValueResponse(FindValueResponse),

    /// Ping to check if node is alive
    Ping(PingRequest),
    Pong(PongResponse),
}

/// FIND_NODE request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindNodeRequest {
    /// Query ID for matching responses
    pub query_id: QueryId,

    /// The node ID we're looking for
    pub target: NodeId,

    /// Requester's node ID
    pub requester: NodeId,
}

/// FIND_NODE response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindNodeResponse {
    /// Query ID from request
    pub query_id: QueryId,

    /// Up to k closest nodes to target
    pub nodes: Vec<NodeInfo>,
}

/// STORE request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreRequest {
    /// Query ID
    pub query_id: QueryId,

    /// Key to store under
    pub key: [u8; 32],

    /// Value to store
    pub value: Vec<u8>,

    /// Time to live in seconds
    pub ttl: u32,

    /// Publisher's node ID
    pub publisher: NodeId,

    /// Signature of the data
    pub signature: Vec<u8>,
}

/// STORE response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreResponse {
    /// Query ID from request
    pub query_id: QueryId,

    /// Whether store was successful
    pub success: bool,

    /// Optional error message
    pub error: Option<String>,
}

/// FIND_VALUE request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindValueRequest {
    /// Query ID
    pub query_id: QueryId,

    /// Key to find
    pub key: [u8; 32],

    /// Requester's node ID
    pub requester: NodeId,
}

/// FIND_VALUE response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FindValueResponse {
    /// Value was found
    Found {
        query_id: QueryId,
        value: Vec<u8>,
        publisher: NodeId,
        signature: Vec<u8>,
    },

    /// Value not found, here are closer nodes
    NotFound {
        query_id: QueryId,
        nodes: Vec<NodeInfo>,
    },
}

/// PING request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingRequest {
    /// Query ID
    pub query_id: QueryId,

    /// Requester's node ID
    pub requester: NodeId,
}

/// PONG response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PongResponse {
    /// Query ID from request
    pub query_id: QueryId,

    /// Responder's node ID
    pub responder: NodeId,
}

/// Iterative lookup state
pub struct IterativeLookup {
    /// Target we're looking for
    pub target: NodeId,

    /// Nodes we've queried
    pub queried: std::collections::HashSet<NodeId>,

    /// Current closest nodes
    pub closest: Vec<NodeInfo>,

    /// Maximum number of results to return
    pub k: usize,

    /// Number of parallel queries (alpha)
    pub alpha: usize,

    /// Whether lookup is complete
    pub complete: bool,
}

impl IterativeLookup {
    /// Create a new iterative lookup
    pub fn new(target: NodeId, initial_nodes: Vec<NodeInfo>, k: usize, alpha: usize) -> Self {
        Self {
            target,
            queried: std::collections::HashSet::new(),
            closest: initial_nodes,
            k,
            alpha,
            complete: false,
        }
    }

    /// Get next nodes to query
    pub fn next_queries(&mut self) -> Vec<NodeInfo> {
        let mut to_query = Vec::new();

        for node in &self.closest {
            if !self.queried.contains(&node.node_id) && to_query.len() < self.alpha {
                to_query.push(node.clone());
                self.queried.insert(node.node_id);
            }
        }

        if to_query.is_empty() {
            self.complete = true;
        }

        to_query
    }

    /// Process response from a query
    pub fn process_response(&mut self, new_nodes: Vec<NodeInfo>) {
        use crate::routing_table::xor_distance;

        // Add new nodes to closest list
        for node in new_nodes {
            if !self.queried.contains(&node.node_id) {
                self.closest.push(node);
            }
        }

        // Sort by distance to target
        self.closest.sort_by_key(|n| xor_distance(&n.node_id, &self.target));

        // Keep only k closest
        self.closest.truncate(self.k);
    }

    /// Get final results
    pub fn results(&self) -> Vec<NodeInfo> {
        self.closest.clone()
    }

    /// Check if lookup is complete
    pub fn is_complete(&self) -> bool {
        self.complete
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

    #[test]
    fn test_find_node_message() {
        let request = FindNodeRequest {
            query_id: [1u8; 16],
            target: NodeId::from_bytes([2u8; 32]),
            requester: NodeId::from_bytes([3u8; 32]),
        };

        let msg = DhtMessage::FindNode(request);
        assert!(matches!(msg, DhtMessage::FindNode(_)));
    }

    #[test]
    fn test_store_request() {
        let request = StoreRequest {
            query_id: [1u8; 16],
            key: [2u8; 32],
            value: b"test data".to_vec(),
            ttl: 3600,
            publisher: NodeId::from_bytes([3u8; 32]),
            signature: vec![4u8; 64],
        };

        assert_eq!(request.value, b"test data");
        assert_eq!(request.ttl, 3600);
    }

    #[test]
    fn test_iterative_lookup() {
        let target = NodeId::from_bytes([10u8; 32]);
        let initial_nodes = vec![
            create_test_node(1),
            create_test_node(2),
            create_test_node(3),
        ];

        let mut lookup = IterativeLookup::new(target, initial_nodes, 5, 3);

        // Get initial queries
        let queries = lookup.next_queries();
        assert_eq!(queries.len(), 3); // alpha = 3

        // Process response
        let new_nodes = vec![
            create_test_node(4),
            create_test_node(5),
        ];
        lookup.process_response(new_nodes);

        // Should have more nodes now
        assert!(lookup.closest.len() > 3);
    }

    #[test]
    fn test_find_value_response_variants() {
        let found = FindValueResponse::Found {
            query_id: [1u8; 16],
            value: b"data".to_vec(),
            publisher: NodeId::from_bytes([2u8; 32]),
            signature: vec![3u8; 64],
        };

        assert!(matches!(found, FindValueResponse::Found { .. }));

        let not_found = FindValueResponse::NotFound {
            query_id: [1u8; 16],
            nodes: vec![],
        };

        assert!(matches!(not_found, FindValueResponse::NotFound { .. }));
    }
}
