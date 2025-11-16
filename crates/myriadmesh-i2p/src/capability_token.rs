//! i2p Capability Token System (Mode 2: Selective Disclosure)
//!
//! Implements privacy-preserving i2p destination sharing via signed capability tokens.
//! Tokens are exchanged privately (NOT in public DHT) to authorize i2p communication.

use myriadmesh_crypto::identity::NodeIdentity;
use myriadmesh_protocol::NodeId;
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::sign::ed25519;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current Unix timestamp with graceful fallback on system time errors
fn now() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(e) => {
            eprintln!(
                "WARNING: System time error in I2P capability token: {}. Using fallback timestamp.",
                e
            );
            1500000000
        }
    }
}

/// i2p destination address (base32 format)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct I2pDestination(String);

impl I2pDestination {
    /// Create new i2p destination
    pub fn new(destination: String) -> Self {
        I2pDestination(destination)
    }

    /// Get destination string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to bytes for signing
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }
}

impl std::fmt::Display for I2pDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// i2p Capability Token
///
/// Allows authorized access to a node's i2p destination.
/// Token is signed by the clearnet NodeID to prove authenticity.
///
/// SECURITY: Tokens are NEVER stored in public DHT.
/// They are exchanged via:
/// - Direct encrypted messages
/// - QR codes (in-person)
/// - Out-of-band secure channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I2pCapabilityToken {
    /// Who can use this token (recipient's clearnet NodeID)
    pub for_node: NodeId,

    /// i2p destination to reach
    pub i2p_destination: I2pDestination,

    /// i2p-specific NodeID (different from clearnet NodeID)
    /// This prevents linking clearnet and i2p identities
    pub i2p_node_id: NodeId,

    /// When this token was issued
    pub issued_at: u64,

    /// When this token expires (Unix timestamp)
    pub expires_at: u64,

    /// Signature by issuer's clearnet keypair (proves authorization)
    pub signature: Vec<u8>,

    /// Issuer's clearnet NodeID (for signature verification)
    pub issuer_node_id: NodeId,
}

impl I2pCapabilityToken {
    /// Create new capability token (unsigned)
    pub fn new(
        for_node: NodeId,
        i2p_destination: I2pDestination,
        i2p_node_id: NodeId,
        issuer_node_id: NodeId,
        validity_days: u64,
    ) -> Self {
        let now = now();
        let expires_at = now + (validity_days * 24 * 60 * 60);

        I2pCapabilityToken {
            for_node,
            i2p_destination,
            i2p_node_id,
            issued_at: now,
            expires_at,
            signature: Vec::new(),
            issuer_node_id,
        }
    }

    /// Sign this token with clearnet identity
    pub fn sign(&mut self, identity: &NodeIdentity) -> Result<(), String> {
        let message = self.signing_message();
        let signature = ed25519::sign_detached(&message, &identity.secret_key);
        self.signature = signature.to_bytes().to_vec();
        Ok(())
    }

    /// Verify token signature
    pub fn verify(&self, issuer_public_key: &ed25519::PublicKey) -> Result<bool, String> {
        if self.signature.is_empty() {
            return Ok(false);
        }

        if self.signature.len() != 64 {
            return Ok(false);
        }

        let message = self.signing_message();

        // Reconstruct signature
        let signature = ed25519::Signature::from_bytes(&self.signature)
            .map_err(|_| "Invalid signature format".to_string())?;

        // Verify signature
        Ok(ed25519::verify_detached(
            &signature,
            &message,
            issuer_public_key,
        ))
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        now() >= self.expires_at
    }

    /// Check if token is valid
    pub fn is_valid(
        &self,
        recipient_node_id: &NodeId,
        issuer_public_key: &ed25519::PublicKey,
    ) -> Result<bool, String> {
        // SECURITY FIX C1: Verify the public key matches the claimed issuer
        // Derive NodeId from the provided public key (using crypto module)
        let derived_node_id_crypto = NodeIdentity::derive_node_id(issuer_public_key);

        // Convert to protocol NodeId for comparison
        let derived_node_id = NodeId::from_bytes(*derived_node_id_crypto.as_bytes());

        // Check that it matches the issuer_node_id in the token
        if derived_node_id != self.issuer_node_id {
            return Ok(false); // Public key doesn't match claimed issuer!
        }

        // Check expiration
        if self.is_expired() {
            return Ok(false);
        }

        // Check recipient
        if self.for_node != *recipient_node_id {
            return Ok(false);
        }

        // Verify signature (now we know the key belongs to the claimed issuer)
        self.verify(issuer_public_key)
    }

    /// Get remaining validity time in seconds
    pub fn ttl_remaining(&self) -> u64 {
        let current = now();
        self.expires_at.saturating_sub(current)
    }

    /// Get message to sign
    fn signing_message(&self) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(self.for_node.as_bytes());
        message.extend_from_slice(&self.i2p_destination.to_bytes());
        message.extend_from_slice(self.i2p_node_id.as_bytes());
        message.extend_from_slice(&self.issued_at.to_le_bytes());
        message.extend_from_slice(&self.expires_at.to_le_bytes());
        message.extend_from_slice(self.issuer_node_id.as_bytes());
        message
    }

    /// Serialize token for transmission
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| format!("Serialization failed: {}", e))
    }

    /// Deserialize token from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|e| format!("Deserialization failed: {}", e))
    }
}

/// Token storage for managing received capability tokens
///
/// SECURITY: This is stored LOCALLY, never in public DHT
#[derive(Debug, Default, Clone)]
pub struct TokenStorage {
    /// Tokens indexed by issuer NodeID
    tokens: std::collections::HashMap<NodeId, Vec<I2pCapabilityToken>>,
}

impl TokenStorage {
    /// Create new token storage
    pub fn new() -> Self {
        TokenStorage {
            tokens: std::collections::HashMap::new(),
        }
    }

    /// Store a capability token
    pub fn store_token(&mut self, token: I2pCapabilityToken) {
        self.tokens
            .entry(token.issuer_node_id)
            .or_default()
            .push(token);
    }

    /// Get token for a specific node
    pub fn get_token(&self, issuer_node_id: &NodeId) -> Option<&I2pCapabilityToken> {
        self.tokens
            .get(issuer_node_id)?
            .iter()
            .find(|t| !t.is_expired())
    }

    /// Get all valid tokens for a node
    pub fn get_all_tokens(&self, issuer_node_id: &NodeId) -> Vec<&I2pCapabilityToken> {
        self.tokens
            .get(issuer_node_id)
            .map(|tokens| tokens.iter().filter(|t| !t.is_expired()).collect())
            .unwrap_or_default()
    }

    /// Remove expired tokens
    pub fn cleanup_expired(&mut self) -> usize {
        let mut removed = 0;

        for tokens in self.tokens.values_mut() {
            let original_len = tokens.len();
            tokens.retain(|t| !t.is_expired());
            removed += original_len - tokens.len();
        }

        // Remove empty entries
        self.tokens.retain(|_, v| !v.is_empty());

        removed
    }

    /// Get total number of tokens
    pub fn token_count(&self) -> usize {
        self.tokens.values().map(|v| v.len()).sum()
    }

    /// Clear all tokens
    pub fn clear(&mut self) {
        self.tokens.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myriadmesh_protocol::types::NODE_ID_SIZE;

    #[test]
    fn test_i2p_destination() {
        let dest = I2pDestination::new("ukeu3k5oykyyhmjj...b32.i2p".to_string());
        assert_eq!(dest.as_str(), "ukeu3k5oykyyhmjj...b32.i2p");
        assert!(!dest.to_bytes().is_empty());
    }

    #[test]
    fn test_capability_token_creation() {
        let for_node = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let i2p_node_id = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let issuer_node_id = NodeId::from_bytes([3u8; NODE_ID_SIZE]);
        let dest = I2pDestination::new("test.b32.i2p".to_string());

        let token = I2pCapabilityToken::new(
            for_node,
            dest.clone(),
            i2p_node_id,
            issuer_node_id,
            30, // 30 days
        );

        assert_eq!(token.for_node, for_node);
        assert_eq!(token.i2p_destination, dest);
        assert_eq!(token.i2p_node_id, i2p_node_id);
        assert!(token.ttl_remaining() > 0);
        assert!(!token.is_expired());
    }

    #[test]
    fn test_token_signing_and_verification() {
        myriadmesh_crypto::init().unwrap();
        let identity = NodeIdentity::generate().unwrap();
        let for_node = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let i2p_node_id = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let issuer_node_id = NodeId::from_bytes(*identity.node_id.as_bytes());
        let dest = I2pDestination::new("test.b32.i2p".to_string());

        let mut token = I2pCapabilityToken::new(for_node, dest, i2p_node_id, issuer_node_id, 30);

        // Sign token
        token.sign(&identity).unwrap();
        assert!(!token.signature.is_empty());

        // Verify signature
        let verified = token.verify(&identity.public_key).unwrap();
        assert!(verified);

        // Verify full token validation
        let valid = token.is_valid(&for_node, &identity.public_key).unwrap();
        assert!(valid);
    }

    #[test]
    fn test_token_expiration() {
        let for_node = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let i2p_node_id = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let issuer_node_id = NodeId::from_bytes([3u8; NODE_ID_SIZE]);
        let dest = I2pDestination::new("test.b32.i2p".to_string());

        let mut token = I2pCapabilityToken::new(for_node, dest, i2p_node_id, issuer_node_id, 30);

        // Token should not be expired initially
        assert!(!token.is_expired());

        // Manually set expiration to past
        token.expires_at = now() - 3600;
        assert!(token.is_expired());
        assert_eq!(token.ttl_remaining(), 0);
    }

    #[test]
    fn test_token_serialization() {
        let for_node = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let i2p_node_id = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let issuer_node_id = NodeId::from_bytes([3u8; NODE_ID_SIZE]);
        let dest = I2pDestination::new("test.b32.i2p".to_string());

        let token = I2pCapabilityToken::new(for_node, dest, i2p_node_id, issuer_node_id, 30);

        // Serialize and deserialize
        let bytes = token.to_bytes().unwrap();
        let deserialized = I2pCapabilityToken::from_bytes(&bytes).unwrap();

        assert_eq!(token.for_node, deserialized.for_node);
        assert_eq!(token.i2p_destination, deserialized.i2p_destination);
        assert_eq!(token.i2p_node_id, deserialized.i2p_node_id);
    }

    #[test]
    fn test_token_forgery_prevention() {
        // SECURITY TEST C1: Verify that tokens cannot be forged with wrong key
        myriadmesh_crypto::init().unwrap();

        // Create legitimate issuer identity
        let legitimate_issuer = NodeIdentity::generate().unwrap();
        let legitimate_issuer_node_id = NodeId::from_bytes(*legitimate_issuer.node_id.as_bytes());

        // Create attacker identity
        let attacker = NodeIdentity::generate().unwrap();

        let recipient_node_id = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let i2p_node_id = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let dest = I2pDestination::new("victim.b32.i2p".to_string());

        // Attacker creates token claiming to be from legitimate_issuer
        let mut forged_token = I2pCapabilityToken::new(
            recipient_node_id,
            dest,
            i2p_node_id,
            legitimate_issuer_node_id, // Claiming to be from legitimate issuer!
            30,
        );

        // Attacker signs with their own key
        forged_token.sign(&attacker).unwrap();

        // Try to validate with attacker's public key
        // This should FAIL because attacker's public key doesn't derive to legitimate_issuer_node_id
        let valid_with_attacker_key = forged_token
            .is_valid(&recipient_node_id, &attacker.public_key)
            .unwrap();
        assert!(!valid_with_attacker_key, "Forged token should be rejected!");

        // Try to validate with legitimate issuer's public key
        // This should also FAIL because the signature was made with attacker's key
        let valid_with_legitimate_key = forged_token
            .is_valid(&recipient_node_id, &legitimate_issuer.public_key)
            .unwrap();
        assert!(
            !valid_with_legitimate_key,
            "Forged token should be rejected!"
        );

        // Now create a LEGITIMATE token
        let mut legitimate_token = I2pCapabilityToken::new(
            recipient_node_id,
            I2pDestination::new("real.b32.i2p".to_string()),
            i2p_node_id,
            legitimate_issuer_node_id,
            30,
        );

        // Sign with correct key
        legitimate_token.sign(&legitimate_issuer).unwrap();

        // This should succeed
        let valid = legitimate_token
            .is_valid(&recipient_node_id, &legitimate_issuer.public_key)
            .unwrap();
        assert!(valid, "Legitimate token should be accepted!");
    }

    #[test]
    fn test_token_storage() {
        let mut storage = TokenStorage::new();

        let issuer_node_id = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let for_node = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let i2p_node_id = NodeId::from_bytes([3u8; NODE_ID_SIZE]);
        let dest = I2pDestination::new("test.b32.i2p".to_string());

        let token = I2pCapabilityToken::new(for_node, dest, i2p_node_id, issuer_node_id, 30);

        // Store token
        storage.store_token(token.clone());
        assert_eq!(storage.token_count(), 1);

        // Retrieve token
        let retrieved = storage.get_token(&issuer_node_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().for_node, for_node);

        // Clear storage
        storage.clear();
        assert_eq!(storage.token_count(), 0);
    }

    #[test]
    fn test_token_storage_cleanup() {
        let mut storage = TokenStorage::new();

        let issuer_node_id = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let for_node = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let i2p_node_id = NodeId::from_bytes([3u8; NODE_ID_SIZE]);
        let dest = I2pDestination::new("test.b32.i2p".to_string());

        // Create expired token
        let mut token = I2pCapabilityToken::new(for_node, dest, i2p_node_id, issuer_node_id, 30);
        token.expires_at = now() - 3600; // Expired

        storage.store_token(token);
        assert_eq!(storage.token_count(), 1);

        // Cleanup should remove expired token
        let removed = storage.cleanup_expired();
        assert_eq!(removed, 1);
        assert_eq!(storage.token_count(), 0);
    }
}
