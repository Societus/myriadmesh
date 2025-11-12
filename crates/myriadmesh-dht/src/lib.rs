//! MyriadMesh DHT Module
//!
//! Kademlia-based Distributed Hash Table implementation for MyriadMesh
//!
//! This crate provides:
//! - Kademlia routing table with k-buckets
//! - Node reputation tracking
//! - DHT storage with TTL and size limits
//! - DHT operations (FIND_NODE, STORE, FIND_VALUE, PING)
//! - DHT manager coordinating all components

pub mod manager;
pub mod node_info;
pub mod operations;
pub mod routing_table;
pub mod storage;

// Re-export main types
pub use manager::{DhtConfig, DhtManager};
pub use node_info::{AdapterInfo, NodeInfo, NodeReputation};
pub use operations::{
    DhtMessage, FindNodeRequest, FindNodeResponse, FindValueRequest, FindValueResponse,
    IterativeLookup, PingRequest, PongResponse, QueryId, StoreRequest, StoreResponse,
};
pub use routing_table::{KBucket, RoutingTable, K, NUM_BUCKETS};
pub use storage::{DhtStorage, StorageError, StoredValue, MAX_STORAGE_BYTES};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exports() {
        // Just verify that types are exported correctly
        let _ = K;
        let _ = NUM_BUCKETS;
        let _ = MAX_STORAGE_BYTES;
    }
}
