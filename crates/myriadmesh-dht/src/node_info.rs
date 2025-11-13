//! Node information for DHT routing table
//!
//! SECURITY C2: Implements Proof-of-Work for Sybil resistance

use blake2::{Blake2b512, Digest};
use myriadmesh_protocol::types::{AdapterType, NODE_ID_SIZE};
use myriadmesh_protocol::NodeId as ProtocolNodeId;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::reputation::NodeReputation;

/// SECURITY C2: Required PoW difficulty (leading zero bits)
/// 16 bits = ~65k hash attempts average, good balance of cost vs usability
pub const REQUIRED_POW_DIFFICULTY: u32 = 16;

/// Get current timestamp
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// SECURITY C2: Count leading zero bits in a byte array
fn count_leading_zero_bits(data: &[u8]) -> u32 {
    let mut count = 0u32;
    for &byte in data {
        if byte == 0 {
            count += 8;
        } else {
            // Count leading zeros in this byte and stop
            count += byte.leading_zeros();
            break;
        }
    }
    count
}

/// Information about a network adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    /// Adapter type
    pub adapter_type: AdapterType,

    /// Address for this adapter (protocol-specific)
    pub address: String,

    /// Whether this adapter is currently active
    pub active: bool,
}

/// Node capabilities (safe for public sharing in DHT)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeCapabilities {
    /// Can relay messages
    pub can_relay: bool,

    /// Can store DHT data
    pub can_store: bool,

    /// Supports store-and-forward
    pub store_and_forward: bool,

    /// Has i2p capability (Mode 2: Selective Disclosure)
    /// TRUE means node can be reached via i2p, but destination is NOT public
    /// Use capability tokens for private i2p discovery
    pub i2p_capable: bool,

    /// Has Tor capability (similar privacy model to i2p)
    pub tor_capable: bool,

    /// Maximum message size this node can handle
    pub max_message_size: usize,

    /// Available storage (bytes) - 0 means not advertising
    pub available_storage: u64,
}

impl Default for NodeCapabilities {
    fn default() -> Self {
        NodeCapabilities {
            can_relay: true,
            can_store: true,
            store_and_forward: false,
            i2p_capable: false,
            tor_capable: false,
            max_message_size: 1024 * 1024,        // 1MB
            available_storage: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// Information about a node in the DHT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node identifier (32 bytes)
    pub node_id: ProtocolNodeId,

    /// SECURITY C2: Proof-of-Work nonce for Sybil resistance
    /// Must satisfy: hash(node_id || pow_nonce) has REQUIRED_POW_DIFFICULTY leading zero bits
    pub pow_nonce: u64,

    /// Available network adapters
    pub adapters: Vec<AdapterInfo>,

    /// Last successful communication (Unix timestamp)
    pub last_seen: u64,

    /// Round-trip time in milliseconds
    pub rtt_ms: f64,

    /// Consecutive failures
    pub failures: u32,

    /// Reputation tracking
    pub reputation: NodeReputation,

    /// Node capabilities
    pub capabilities: NodeCapabilities,

    /// First seen timestamp
    pub first_seen: u64,

    /// Total successful communications
    pub total_successes: u64,
}

impl NodeInfo {
    /// Create new node info (requires valid PoW nonce)
    pub fn new(node_id: ProtocolNodeId) -> Self {
        let now = now();
        NodeInfo {
            node_id,
            pow_nonce: 0, // SECURITY C2: Must be set with valid PoW before DHT admission
            adapters: Vec::new(),
            last_seen: now,
            rtt_ms: 0.0,
            failures: 0,
            reputation: NodeReputation::new(),
            capabilities: NodeCapabilities::default(),
            first_seen: now,
            total_successes: 0,
        }
    }

    /// Create with adapters
    pub fn with_adapters(node_id: ProtocolNodeId, adapters: Vec<AdapterInfo>) -> Self {
        let mut info = Self::new(node_id);
        info.adapters = adapters;
        info
    }

    /// SECURITY C2: Compute Proof-of-Work for this NodeId
    ///
    /// Finds a nonce such that hash(node_id || nonce) has required leading zero bits.
    /// This is computationally expensive (~65k attempts average for 16-bit difficulty).
    pub fn compute_pow(&mut self) -> u64 {
        let mut nonce = 0u64;
        loop {
            if Self::verify_pow_internal(&self.node_id, nonce, REQUIRED_POW_DIFFICULTY) {
                self.pow_nonce = nonce;
                return nonce;
            }
            nonce += 1;
        }
    }

    /// SECURITY C2: Verify Proof-of-Work for a NodeId + nonce
    ///
    /// Returns true if hash(node_id || nonce) has at least `difficulty` leading zero bits.
    pub fn verify_pow(&self) -> bool {
        Self::verify_pow_internal(&self.node_id, self.pow_nonce, REQUIRED_POW_DIFFICULTY)
    }

    /// Internal PoW verification
    fn verify_pow_internal(node_id: &ProtocolNodeId, nonce: u64, difficulty: u32) -> bool {
        // Compute hash(node_id || nonce)
        let mut hasher = Blake2b512::new();
        hasher.update(node_id.as_bytes());
        hasher.update(nonce.to_le_bytes());
        let hash = hasher.finalize();

        // Count leading zero bits
        let leading_zeros = count_leading_zero_bits(&hash);
        leading_zeros >= difficulty
    }

    /// Record successful communication
    pub fn record_success(&mut self, rtt_ms: f64) {
        self.last_seen = now();
        self.rtt_ms = rtt_ms;
        self.failures = 0;
        self.total_successes += 1;
        self.reputation.record_success();
    }

    /// Record failed communication
    pub fn record_failure(&mut self) {
        self.failures += 1;
        self.reputation.record_failure();
    }

    /// Check if node is likely stale
    pub fn is_stale(&self, max_age_secs: u64) -> bool {
        let age = now().saturating_sub(self.last_seen);
        age > max_age_secs
    }

    /// Check if node should be evicted
    pub fn should_evict(&self, max_failures: u32, max_age_secs: u64) -> bool {
        self.failures >= max_failures || self.is_stale(max_age_secs)
    }

    /// Get best adapter for communication
    pub fn get_best_adapter(&self) -> Option<&AdapterInfo> {
        self.adapters.iter().find(|a| a.active)
    }

    /// Calculate XOR distance to another node
    ///
    /// SECURITY C6: Returns 64-byte XOR distance for enhanced collision resistance
    pub fn distance_to(&self, other: &ProtocolNodeId) -> [u8; NODE_ID_SIZE] {
        self.node_id.distance(other)
    }

    /// Convert to public node info (safe for DHT sharing)
    /// SECURITY: This removes adapter addresses to prevent de-anonymization
    pub fn to_public(&self) -> PublicNodeInfo {
        PublicNodeInfo {
            node_id: self.node_id,
            capabilities: self.capabilities.clone(),
            reputation: self.reputation.clone(),
            last_seen: self.last_seen,
            rtt_ms: self.rtt_ms,
        }
    }
}

/// Public node information (safe for DHT distribution)
///
/// SECURITY: This structure is shared publicly in DHT queries.
/// It MUST NOT contain any adapter addresses that could de-anonymize users.
///
/// For i2p/Tor: Use capability flags (i2p_capable, tor_capable) to indicate
/// support, but never include the actual destination/onion address here.
/// Private discovery uses capability tokens exchanged out-of-band.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicNodeInfo {
    /// Node identifier (32 bytes)
    pub node_id: ProtocolNodeId,

    /// Node capabilities (includes privacy-preserving flags)
    pub capabilities: NodeCapabilities,

    /// Reputation tracking
    pub reputation: NodeReputation,

    /// Last successful communication (Unix timestamp)
    pub last_seen: u64,

    /// Round-trip time in milliseconds
    pub rtt_ms: f64,
}

impl PublicNodeInfo {
    /// Create new public node info
    pub fn new(node_id: ProtocolNodeId, capabilities: NodeCapabilities) -> Self {
        PublicNodeInfo {
            node_id,
            capabilities,
            reputation: NodeReputation::new(),
            last_seen: now(),
            rtt_ms: 0.0,
        }
    }

    /// Calculate XOR distance to another node
    ///
    /// SECURITY C6: Returns 64-byte XOR distance for enhanced collision resistance
    pub fn distance_to(&self, other: &ProtocolNodeId) -> [u8; NODE_ID_SIZE] {
        self.node_id.distance(other)
    }

    /// Check if node is likely stale
    pub fn is_stale(&self, max_age_secs: u64) -> bool {
        let age = now().saturating_sub(self.last_seen);
        age > max_age_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node() -> NodeInfo {
        NodeInfo::new(ProtocolNodeId::from_bytes([1u8; NODE_ID_SIZE]))
    }

    #[test]
    fn test_new_node() {
        let node = create_test_node();
        assert_eq!(node.failures, 0);
        assert_eq!(node.total_successes, 0);
        // SECURITY C7: New nodes start with low reputation, must earn trust
        assert!(!node.reputation.is_trustworthy());
    }

    #[test]
    fn test_record_success() {
        let mut node = create_test_node();

        node.record_success(10.5);
        assert_eq!(node.failures, 0);
        assert_eq!(node.total_successes, 1);
        assert_eq!(node.rtt_ms, 10.5);
    }

    #[test]
    fn test_record_failure() {
        let mut node = create_test_node();

        node.record_failure();
        assert_eq!(node.failures, 1);
    }

    #[test]
    fn test_should_evict() {
        let mut node = create_test_node();

        // Fresh node should not be evicted
        assert!(!node.should_evict(3, 3600));

        // Too many failures
        node.failures = 5;
        assert!(node.should_evict(3, 3600));
    }

    #[test]
    fn test_is_stale() {
        let mut node = create_test_node();

        // Fresh node is not stale
        assert!(!node.is_stale(3600));

        // Old node is stale
        node.last_seen = now() - 7200; // 2 hours ago
        assert!(node.is_stale(3600)); // Max age 1 hour
    }

    #[test]
    fn test_with_adapters() {
        let node_id = ProtocolNodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let adapters = vec![AdapterInfo {
            adapter_type: AdapterType::Ethernet,
            address: "192.168.1.1:4001".to_string(),
            active: true,
        }];

        let node = NodeInfo::with_adapters(node_id, adapters.clone());
        assert_eq!(node.adapters.len(), 1);
        assert!(node.get_best_adapter().is_some());
    }

    #[test]
    fn test_to_public_removes_adapter_addresses() {
        let node_id = ProtocolNodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let adapters = vec![
            AdapterInfo {
                adapter_type: AdapterType::Ethernet,
                address: "192.168.1.1:4001".to_string(),
                active: true,
            },
            AdapterInfo {
                adapter_type: AdapterType::I2P,
                address: "ukeu3k5o...b32.i2p".to_string(),
                active: true,
            },
        ];

        let mut node = NodeInfo::with_adapters(node_id, adapters);
        node.capabilities.i2p_capable = true;

        // Convert to public
        let public = node.to_public();

        // Public version should not have adapter addresses
        // but should preserve capability flags
        assert_eq!(public.node_id, node.node_id);
        assert!(public.capabilities.i2p_capable);
        assert_eq!(public.reputation.score(), node.reputation.score());
    }

    #[test]
    fn test_public_node_info_creation() {
        let node_id = ProtocolNodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let caps = NodeCapabilities {
            i2p_capable: true,
            tor_capable: false,
            ..Default::default()
        };

        let public = PublicNodeInfo::new(node_id, caps.clone());

        assert_eq!(public.node_id, node_id);
        assert_eq!(public.capabilities, caps);
        // SECURITY C7: New nodes start with low reputation, must earn trust
        assert!(!public.reputation.is_trustworthy());
    }

    #[test]
    fn test_public_node_info_is_stale() {
        let node_id = ProtocolNodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let mut public = PublicNodeInfo::new(node_id, NodeCapabilities::default());

        // Fresh node is not stale
        assert!(!public.is_stale(3600));

        // Old node is stale
        public.last_seen = now() - 7200; // 2 hours ago
        assert!(public.is_stale(3600)); // Max age 1 hour
    }

    #[test]
    fn test_node_capabilities_default() {
        let caps = NodeCapabilities::default();
        assert!(caps.can_relay);
        assert!(caps.can_store);
        assert!(!caps.store_and_forward);
        assert!(!caps.i2p_capable);
        assert!(!caps.tor_capable);
    }

    // SECURITY C2: Proof-of-Work tests

    #[test]
    fn test_count_leading_zero_bits() {
        // All zeros
        assert_eq!(count_leading_zero_bits(&[0u8; 8]), 64);

        // First byte non-zero
        assert_eq!(count_leading_zero_bits(&[0b00010000, 0, 0, 0]), 3);

        // Multiple zero bytes then non-zero
        assert_eq!(count_leading_zero_bits(&[0, 0, 0b00000001, 0]), 7 + 8 + 8);

        // No leading zeros
        assert_eq!(count_leading_zero_bits(&[0b10000000, 0, 0, 0]), 0);
    }

    #[test]
    fn test_pow_compute_and_verify() {
        // SECURITY C2: PoW computation and verification
        let mut node = NodeInfo::new(ProtocolNodeId::from_bytes([42u8; NODE_ID_SIZE]));

        // Initially has no valid PoW
        assert!(!node.verify_pow());

        // Compute PoW (this will take ~65k attempts on average for 16-bit difficulty)
        let nonce = node.compute_pow();

        // Now PoW should be valid
        assert!(node.verify_pow());
        assert_eq!(node.pow_nonce, nonce);
    }

    #[test]
    fn test_pow_reject_invalid_nonce() {
        // SECURITY C2: Verify that invalid nonces are rejected
        let mut node = NodeInfo::new(ProtocolNodeId::from_bytes([123u8; NODE_ID_SIZE]));

        // Set an arbitrary invalid nonce
        node.pow_nonce = 12345;

        // Should fail verification (extremely unlikely to be valid)
        assert!(!node.verify_pow());
    }

    #[test]
    fn test_pow_different_nodes_need_different_nonces() {
        // SECURITY C2: Different NodeIDs need different PoW solutions
        let node_id_1 = ProtocolNodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let node_id_2 = ProtocolNodeId::from_bytes([2u8; NODE_ID_SIZE]);

        let mut node1 = NodeInfo::new(node_id_1);
        let mut node2 = NodeInfo::new(node_id_2);

        node1.compute_pow();
        node2.compute_pow();

        // Different NodeIDs should have different nonces
        // (extremely unlikely to be the same)
        assert_ne!(node1.pow_nonce, node2.pow_nonce);

        // Each should verify correctly
        assert!(node1.verify_pow());
        assert!(node2.verify_pow());

        // Swapping nonces should fail verification
        let temp = node1.pow_nonce;
        node1.pow_nonce = node2.pow_nonce;
        node2.pow_nonce = temp;

        assert!(!node1.verify_pow());
        assert!(!node2.verify_pow());
    }

    #[test]
    fn test_pow_low_difficulty() {
        // Test with very low difficulty for speed
        let node_id = ProtocolNodeId::from_bytes([99u8; NODE_ID_SIZE]);

        // Test with difficulty 4 (should be fast: ~16 attempts)
        let mut nonce = 0u64;
        loop {
            if NodeInfo::verify_pow_internal(&node_id, nonce, 4) {
                break;
            }
            nonce += 1;
            assert!(nonce < 1000, "Took too many attempts for difficulty 4");
        }

        // Verify the nonce works
        assert!(NodeInfo::verify_pow_internal(&node_id, nonce, 4));
    }
}
