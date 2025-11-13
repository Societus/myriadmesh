//! MyriadMesh Core Library
//!
//! This is the main library that ties together all MyriadMesh components:
//! - Cryptography (identity, signing, verification)
//! - Protocol (messages, frames, node IDs)
//! - DHT (distributed hash table with privacy modes)
//! - Routing (priority queuing, rate limiting, message routing)
//! - Network (multi-transport abstraction, adapters)
//! - i2p (capability tokens, privacy layers, onion routing)

pub use myriadmesh_crypto as crypto;
pub use myriadmesh_dht as dht;
pub use myriadmesh_i2p as i2p;
pub use myriadmesh_network as network;
pub use myriadmesh_protocol as protocol;
pub use myriadmesh_routing as routing;

pub use crypto::CryptoError;
pub use protocol::ProtocolError;

/// Initialize the MyriadMesh library
pub fn init() -> Result<(), CryptoError> {
    crypto::init()
}

#[cfg(test)]
mod tests {
    use super::*;
    use myriadmesh_protocol::types::NODE_ID_SIZE;

    #[test]
    fn test_init() {
        assert!(init().is_ok());
    }

    #[test]
    fn test_module_exports() {
        // Verify all Phase 2 modules are accessible
        let _ = crypto::init();

        // Protocol types
        // SECURITY C6: NodeID is now 64 bytes for collision resistance
        let node_id = protocol::NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        assert_eq!(node_id.as_bytes().len(), NODE_ID_SIZE);

        // DHT types are accessible
        let capabilities = dht::NodeCapabilities::default();
        let reputation = dht::NodeReputation::new();
        let public_info = dht::PublicNodeInfo {
            node_id,
            capabilities,
            reputation,
            last_seen: 0,
            rtt_ms: 0.0,
        };
        assert_eq!(public_info.node_id, node_id);

        // Routing types are accessible (just verify module exists)
        // The routing module exports MessageRouter and related types

        // Network types are accessible
        let addr = network::Address::Ethernet("127.0.0.1:4001".to_string());
        assert!(matches!(addr, network::Address::Ethernet(_)));

        // i2p types are accessible
        let dest = i2p::I2pDestination::new("test.b32.i2p".to_string());
        assert_eq!(dest.as_str(), "test.b32.i2p");
    }
}
