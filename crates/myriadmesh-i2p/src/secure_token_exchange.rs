//! Secure capability token exchange using encrypted channels
//!
//! This module provides utilities for securely transmitting i2p capability
//! tokens between nodes using end-to-end encryption.

use crate::capability_token::I2pCapabilityToken;
use crate::dual_identity::DualIdentity;
use myriadmesh_crypto::channel::{EncryptedChannel, KeyExchangeRequest, KeyExchangeResponse};
use myriadmesh_crypto::keyexchange::KeyExchangeKeypair;
use serde::{Deserialize, Serialize};

/// Encrypted capability token message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedTokenMessage {
    /// Encrypted token data
    pub encrypted_data: Vec<u8>,
}

/// Secure token exchange manager
pub struct SecureTokenExchange {
    /// Dual identity for this node
    identity: DualIdentity,

    /// Key exchange keypair for encrypted channels
    kx_keypair: KeyExchangeKeypair,
}

impl SecureTokenExchange {
    /// Create new secure token exchange manager
    pub fn new(identity: DualIdentity, kx_keypair: KeyExchangeKeypair) -> Self {
        SecureTokenExchange {
            identity,
            kx_keypair,
        }
    }

    /// Create a key exchange request to establish encrypted channel
    pub fn create_key_exchange_request(
        &self,
        remote_clearnet_node_id: myriadmesh_protocol::NodeId,
    ) -> Result<KeyExchangeRequest, String> {
        let mut channel = EncryptedChannel::new(
            *self.identity.get_clearnet_node_id().as_bytes(),
            self.kx_keypair.clone(),
        );

        channel
            .create_key_exchange_request(*remote_clearnet_node_id.as_bytes())
            .map_err(|e| format!("Key exchange request failed: {}", e))
    }

    /// Process key exchange request and send response
    pub fn process_key_exchange_request(
        &self,
        request: &KeyExchangeRequest,
    ) -> Result<KeyExchangeResponse, String> {
        let mut channel = EncryptedChannel::new(
            *self.identity.get_clearnet_node_id().as_bytes(),
            self.kx_keypair.clone(),
        );

        channel
            .process_key_exchange_request(request)
            .map_err(|e| format!("Key exchange processing failed: {}", e))
    }

    /// Encrypt and send a capability token
    pub fn encrypt_token(
        &self,
        token: &I2pCapabilityToken,
        kx_request: &KeyExchangeRequest,
        _kx_response: &KeyExchangeResponse,
    ) -> Result<EncryptedTokenMessage, String> {
        // Create channel and establish it
        let mut channel = EncryptedChannel::new(
            *self.identity.get_clearnet_node_id().as_bytes(),
            self.kx_keypair.clone(),
        );

        // Process the key exchange to establish channel
        let _response = channel
            .process_key_exchange_request(kx_request)
            .map_err(|e| format!("Failed to process key exchange: {}", e))?;

        // Serialize token
        let token_bytes =
            bincode::serialize(token).map_err(|e| format!("Token serialization failed: {}", e))?;

        // Encrypt token
        let encrypted_data = channel
            .encrypt_message(&token_bytes)
            .map_err(|e| format!("Encryption failed: {}", e))?;

        Ok(EncryptedTokenMessage { encrypted_data })
    }

    /// Decrypt a received capability token
    pub fn decrypt_token(
        &self,
        encrypted_msg: &EncryptedTokenMessage,
        _kx_request: &KeyExchangeRequest,
        kx_response: &KeyExchangeResponse,
    ) -> Result<I2pCapabilityToken, String> {
        // Create channel and establish it
        let mut channel = EncryptedChannel::new(
            *self.identity.get_clearnet_node_id().as_bytes(),
            self.kx_keypair.clone(),
        );

        // Initiate key exchange
        let _req = channel
            .create_key_exchange_request(kx_response.from_node_id)
            .map_err(|e| format!("Failed to create key exchange: {}", e))?;

        // Complete key exchange
        channel
            .process_key_exchange_response(kx_response)
            .map_err(|e| format!("Failed to process key exchange response: {}", e))?;

        // Decrypt token
        let decrypted_bytes = channel
            .decrypt_message(&encrypted_msg.encrypted_data)
            .map_err(|e| format!("Decryption failed: {}", e))?;

        // Deserialize token
        bincode::deserialize(&decrypted_bytes)
            .map_err(|e| format!("Token deserialization failed: {}", e))
    }

    /// Grant i2p access and send encrypted token
    ///
    /// This is a convenience method that:
    /// 1. Grants i2p access (creates capability token)
    /// 2. Establishes encrypted channel
    /// 3. Encrypts and returns the token
    pub fn grant_access_with_encryption(
        &mut self,
        recipient_node_id: myriadmesh_protocol::NodeId,
        validity_days: u32,
        kx_request: &KeyExchangeRequest,
    ) -> Result<(I2pCapabilityToken, EncryptedTokenMessage), String> {
        // Grant access (creates token)
        let token = self
            .identity
            .grant_i2p_access(recipient_node_id, validity_days as u64)
            .map_err(|e| format!("Failed to grant access: {}", e))?;

        // Create response for key exchange
        let kx_response = self.process_key_exchange_request(kx_request)?;

        // Encrypt token
        let encrypted = self.encrypt_token(&token, kx_request, &kx_response)?;

        Ok((token, encrypted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::I2pDestination;
    use myriadmesh_crypto::keyexchange::KeyExchangeKeypair;

    #[test]
    fn test_secure_token_exchange() {
        myriadmesh_crypto::init().unwrap();

        // Alice creates identity and token exchange
        let alice_dest = I2pDestination::new("alice.b32.i2p".to_string());
        let alice_identity = DualIdentity::generate(alice_dest).unwrap();
        let alice_kx_kp = KeyExchangeKeypair::generate();
        let alice_exchange = SecureTokenExchange::new(alice_identity.clone(), alice_kx_kp);

        // Bob creates identity and token exchange
        let bob_dest = I2pDestination::new("bob.b32.i2p".to_string());
        let bob_identity = DualIdentity::generate(bob_dest).unwrap();
        let bob_kx_kp = KeyExchangeKeypair::generate();
        let bob_exchange = SecureTokenExchange::new(bob_identity.clone(), bob_kx_kp);

        // Bob requests access from Alice
        let kx_request = bob_exchange
            .create_key_exchange_request(alice_identity.get_clearnet_node_id())
            .unwrap();

        // Alice processes request
        let kx_response = alice_exchange
            .process_key_exchange_request(&kx_request)
            .unwrap();

        // Alice grants Bob access
        let token = alice_identity
            .grant_i2p_access(bob_identity.get_clearnet_node_id(), 30)
            .unwrap();

        // Alice encrypts token for Bob
        let encrypted = alice_exchange
            .encrypt_token(&token, &kx_request, &kx_response)
            .unwrap();

        // Bob decrypts token
        let decrypted = bob_exchange
            .decrypt_token(&encrypted, &kx_request, &kx_response)
            .unwrap();

        // Verify token matches
        assert_eq!(token.for_node, decrypted.for_node);
        assert_eq!(token.i2p_destination, decrypted.i2p_destination);
        assert_eq!(token.i2p_node_id, decrypted.i2p_node_id);
    }

    #[test]
    fn test_end_to_end_encrypted_token_exchange() {
        myriadmesh_crypto::init().unwrap();

        // Setup Alice
        let alice_dest = I2pDestination::new("alice.b32.i2p".to_string());
        let alice_identity = DualIdentity::generate(alice_dest).unwrap();
        let alice_kx_kp = KeyExchangeKeypair::generate();
        let mut alice_exchange = SecureTokenExchange::new(alice_identity.clone(), alice_kx_kp);

        // Setup Bob
        let bob_dest = I2pDestination::new("bob.b32.i2p".to_string());
        let mut bob_identity = DualIdentity::generate(bob_dest).unwrap();
        let bob_kx_kp = KeyExchangeKeypair::generate();
        let bob_exchange = SecureTokenExchange::new(bob_identity.clone(), bob_kx_kp);

        // Bob initiates key exchange
        let kx_request = bob_exchange
            .create_key_exchange_request(alice_identity.get_clearnet_node_id())
            .unwrap();

        // Alice grants access with encryption
        let (_token, encrypted) = alice_exchange
            .grant_access_with_encryption(bob_identity.get_clearnet_node_id(), 30, &kx_request)
            .unwrap();

        // Alice processes request to get response
        let kx_response = alice_exchange
            .process_key_exchange_request(&kx_request)
            .unwrap();

        // Bob decrypts and stores token
        let decrypted_token = bob_exchange
            .decrypt_token(&encrypted, &kx_request, &kx_response)
            .unwrap();

        bob_identity
            .store_capability_token(decrypted_token.clone())
            .unwrap();

        // Verify Bob can now access Alice's i2p info
        let alice_token = bob_identity
            .get_capability_token(&alice_identity.get_clearnet_node_id())
            .unwrap();

        assert_eq!(
            alice_token.i2p_destination,
            *alice_identity.get_i2p_destination()
        );
        assert_eq!(alice_token.i2p_node_id, alice_identity.get_i2p_node_id());
    }
}
