//! Dual Identity Management (Mode 2: Selective Disclosure)
//!
//! Manages separate clearnet and i2p identities to prevent de-anonymization.
//! Clearnet NodeID is used for public DHT, i2p NodeID is used only over i2p.

use crate::capability_token::{I2pCapabilityToken, I2pDestination, TokenStorage};
use myriadmesh_crypto::identity::NodeIdentity;
use myriadmesh_protocol::NodeId;
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::sign::ed25519;

/// Dual identity configuration for Mode 2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualIdentity {
    /// Public clearnet identity (advertised in DHT)
    pub clearnet_node_id: NodeId,

    /// Clearnet identity (for signing, stored privately)
    #[serde(skip)]
    clearnet_identity: Option<NodeIdentity>,

    /// Private i2p identity (NEVER linked publicly)
    pub i2p_node_id: NodeId,

    /// i2p identity (for signing, stored privately)
    #[serde(skip)]
    i2p_identity: Option<NodeIdentity>,

    /// i2p destination address
    pub i2p_destination: I2pDestination,

    /// Local storage for received capability tokens
    #[serde(skip)]
    token_storage: TokenStorage,
}

impl DualIdentity {
    /// Create new dual identity with separate identities
    pub fn new(
        clearnet_identity: NodeIdentity,
        i2p_identity: NodeIdentity,
        i2p_destination: I2pDestination,
    ) -> Self {
        // Convert from crypto::NodeId to protocol::NodeId
        let clearnet_node_id = NodeId::from_bytes(*clearnet_identity.node_id.as_bytes());
        let i2p_node_id = NodeId::from_bytes(*i2p_identity.node_id.as_bytes());

        DualIdentity {
            clearnet_node_id,
            clearnet_identity: Some(clearnet_identity),
            i2p_node_id,
            i2p_identity: Some(i2p_identity),
            i2p_destination,
            token_storage: TokenStorage::new(),
        }
    }

    /// Generate new dual identity with random identities
    pub fn generate(i2p_destination: I2pDestination) -> Result<Self, String> {
        let clearnet_identity = NodeIdentity::generate()
            .map_err(|e| format!("Failed to generate clearnet identity: {}", e))?;
        let i2p_identity = NodeIdentity::generate()
            .map_err(|e| format!("Failed to generate i2p identity: {}", e))?;
        Ok(Self::new(clearnet_identity, i2p_identity, i2p_destination))
    }

    /// Get clearnet NodeID (public)
    pub fn get_clearnet_node_id(&self) -> NodeId {
        self.clearnet_node_id
    }

    /// Get i2p NodeID (private, only shared via capability tokens)
    pub fn get_i2p_node_id(&self) -> NodeId {
        self.i2p_node_id
    }

    /// Get i2p destination
    pub fn get_i2p_destination(&self) -> &I2pDestination {
        &self.i2p_destination
    }

    /// Get clearnet public key (for signature verification)
    pub fn get_clearnet_public_key(&self) -> Option<&ed25519::PublicKey> {
        self.clearnet_identity.as_ref().map(|id| &id.public_key)
    }

    /// Get i2p public key (for signature verification)
    pub fn get_i2p_public_key(&self) -> Option<&ed25519::PublicKey> {
        self.i2p_identity.as_ref().map(|id| &id.public_key)
    }

    /// Grant i2p access to another node by issuing a capability token
    ///
    /// This creates a signed token that authorizes the recipient to reach
    /// this node via i2p. The token reveals this node's i2p destination
    /// and i2p-specific NodeID.
    ///
    /// SECURITY: Token should be transmitted via encrypted channel!
    pub fn grant_i2p_access(&self, contact_node_id: NodeId, validity_days: u64) -> Result<I2pCapabilityToken, String> {
        let clearnet_identity = self
            .clearnet_identity
            .as_ref()
            .ok_or("Clearnet identity not available")?;

        let mut token = I2pCapabilityToken::new(
            contact_node_id,
            self.i2p_destination.clone(),
            self.i2p_node_id,
            self.clearnet_node_id,
            validity_days,
        );

        // Sign token with clearnet identity
        token.sign(clearnet_identity)?;

        Ok(token)
    }

    /// Store a received capability token
    ///
    /// Allows this node to reach the token issuer via i2p.
    pub fn store_capability_token(&mut self, token: I2pCapabilityToken) -> Result<(), String> {
        // Verify token is for us
        if token.for_node != self.clearnet_node_id {
            return Err("Token not intended for this node".to_string());
        }

        // Check not expired
        if token.is_expired() {
            return Err("Token is expired".to_string());
        }

        self.token_storage.store_token(token);
        Ok(())
    }

    /// Get capability token for reaching a specific node via i2p
    pub fn get_capability_token(&self, node_id: &NodeId) -> Option<&I2pCapabilityToken> {
        self.token_storage.get_token(node_id)
    }

    /// Get all capability tokens for a node
    pub fn get_all_capability_tokens(&self, node_id: &NodeId) -> Vec<&I2pCapabilityToken> {
        self.token_storage.get_all_tokens(node_id)
    }

    /// Cleanup expired capability tokens
    pub fn cleanup_expired_tokens(&mut self) -> usize {
        self.token_storage.cleanup_expired()
    }

    /// Get number of stored capability tokens
    pub fn token_count(&self) -> usize {
        self.token_storage.token_count()
    }

    /// Verify that clearnet and i2p NodeIDs are different
    ///
    /// SECURITY: This is critical for Mode 2!
    /// If they're the same, there's identity linkage.
    pub fn verify_separate_identities(&self) -> bool {
        self.clearnet_node_id != self.i2p_node_id
    }

    /// Generate QR code data for capability token (for in-person exchange)
    pub fn generate_qr_token(&self, contact_node_id: NodeId, validity_days: u64) -> Result<Vec<u8>, String> {
        let token = self.grant_i2p_access(contact_node_id, validity_days)?;
        token.to_bytes()
    }

    /// Parse capability token from QR code data
    pub fn parse_qr_token(data: &[u8]) -> Result<I2pCapabilityToken, String> {
        I2pCapabilityToken::from_bytes(data)
    }

    /// Save identity to persistent storage (serialization)
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| format!("Serialization failed: {}", e))
    }

    /// Load identity from persistent storage
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        bincode::deserialize(data).map_err(|e| format!("Deserialization failed: {}", e))
    }

    /// Set identities after deserialization (identities are not serialized)
    pub fn set_identities(&mut self, clearnet_identity: NodeIdentity, i2p_identity: NodeIdentity) {
        self.clearnet_identity = Some(clearnet_identity);
        self.i2p_identity = Some(i2p_identity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_identity() -> DualIdentity {
        myriadmesh_crypto::init().unwrap();
        let dest = I2pDestination::new("test.b32.i2p".to_string());
        DualIdentity::generate(dest).unwrap()
    }

    #[test]
    fn test_dual_identity_creation() {
        let identity = create_test_identity();

        // Verify separate identities
        assert!(identity.verify_separate_identities());
        assert_ne!(identity.get_clearnet_node_id(), identity.get_i2p_node_id());
    }

    #[test]
    fn test_grant_i2p_access() {
        let identity = create_test_identity();
        let contact_node_id = NodeId::from_bytes([5u8; 32]);

        // Grant access
        let token = identity.grant_i2p_access(contact_node_id, 30).unwrap();

        assert_eq!(token.for_node, contact_node_id);
        assert_eq!(token.i2p_node_id, identity.get_i2p_node_id());
        assert_eq!(token.issuer_node_id, identity.get_clearnet_node_id());
        assert!(!token.signature.is_empty());
    }

    #[test]
    fn test_store_and_retrieve_token() {
        let alice = create_test_identity();
        let mut bob = create_test_identity();

        // Alice grants Bob access
        let token = alice.grant_i2p_access(bob.get_clearnet_node_id(), 30).unwrap();

        // Bob stores the token
        bob.store_capability_token(token.clone()).unwrap();
        assert_eq!(bob.token_count(), 1);

        // Bob retrieves token
        let retrieved = bob.get_capability_token(&alice.get_clearnet_node_id());
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().i2p_destination, *alice.get_i2p_destination());
    }

    #[test]
    fn test_token_rejection_wrong_recipient() {
        let alice = create_test_identity();
        let mut bob = create_test_identity();
        let charlie_node_id = NodeId::from_bytes([6u8; 32]);

        // Alice grants access to Charlie
        let token = alice.grant_i2p_access(charlie_node_id, 30).unwrap();

        // Bob tries to store token meant for Charlie
        let result = bob.store_capability_token(token);
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_expired_tokens() {
        let alice = create_test_identity();
        let mut bob = create_test_identity();

        // Create expired token
        let mut token = alice.grant_i2p_access(bob.get_clearnet_node_id(), 30).unwrap();
        token.expires_at = 0; // Set to past

        // Store expired token
        bob.token_storage.store_token(token);
        assert_eq!(bob.token_count(), 1);

        // Cleanup should remove it
        let removed = bob.cleanup_expired_tokens();
        assert_eq!(removed, 1);
        assert_eq!(bob.token_count(), 0);
    }

    #[test]
    fn test_qr_code_generation() {
        let identity = create_test_identity();
        let contact_node_id = NodeId::from_bytes([7u8; 32]);

        // Generate QR code data
        let qr_data = identity.generate_qr_token(contact_node_id, 30).unwrap();
        assert!(!qr_data.is_empty());

        // Parse QR code data
        let token = DualIdentity::parse_qr_token(&qr_data).unwrap();
        assert_eq!(token.for_node, contact_node_id);
        assert_eq!(token.issuer_node_id, identity.get_clearnet_node_id());
    }

    #[test]
    fn test_identity_serialization() {
        let identity = create_test_identity();

        // Serialize
        let bytes = identity.to_bytes().unwrap();
        assert!(!bytes.is_empty());

        // Deserialize
        let mut deserialized = DualIdentity::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.clearnet_node_id, identity.clearnet_node_id);
        assert_eq!(deserialized.i2p_node_id, identity.i2p_node_id);
        assert_eq!(deserialized.i2p_destination, identity.i2p_destination);

        // Note: Identities are not serialized for security
        // They must be set separately
        assert!(deserialized.clearnet_identity.is_none());
        assert!(deserialized.i2p_identity.is_none());

        // Set identities
        let clearnet_id = NodeIdentity::generate().unwrap();
        let i2p_id = NodeIdentity::generate().unwrap();
        deserialized.set_identities(clearnet_id, i2p_id);

        assert!(deserialized.clearnet_identity.is_some());
        assert!(deserialized.i2p_identity.is_some());
    }

    #[test]
    fn test_separate_identities_required() {
        let identity = create_test_identity();

        // Should always have separate identities
        assert!(identity.verify_separate_identities());

        // Different public keys should produce different NodeIDs
        let clearnet_bytes = identity.clearnet_node_id.as_bytes();
        let i2p_bytes = identity.i2p_node_id.as_bytes();
        assert_ne!(clearnet_bytes, i2p_bytes);
    }
}
