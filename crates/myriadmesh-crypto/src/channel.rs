//! Encrypted channels for end-to-end message encryption
//!
//! This module provides secure, authenticated channels for encrypting
//! messages between nodes using X25519 key exchange and XSalsa20-Poly1305.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Alice initiates channel
//! let alice_identity = NodeIdentity::generate()?;
//! let alice_kx_keypair = KeyExchangeKeypair::generate();
//! let mut alice_channel = EncryptedChannel::new(alice_identity.node_id, alice_kx_keypair);
//!
//! // Alice creates key exchange request
//! let kx_request = alice_channel.create_key_exchange_request(bob_node_id)?;
//!
//! // Bob responds to key exchange
//! let bob_identity = NodeIdentity::generate()?;
//! let bob_kx_keypair = KeyExchangeKeypair::generate();
//! let mut bob_channel = EncryptedChannel::new(bob_identity.node_id, bob_kx_keypair);
//!
//! let kx_response = bob_channel.process_key_exchange_request(&kx_request)?;
//!
//! // Alice processes response
//! alice_channel.process_key_exchange_response(&kx_response)?;
//!
//! // Now both can encrypt/decrypt messages
//! let plaintext = b"Secret message";
//! let encrypted = alice_channel.encrypt_message(plaintext)?;
//! let decrypted = bob_channel.decrypt_message(&encrypted)?;
//! assert_eq!(plaintext, &decrypted[..]);
//! ```

use crate::encryption::{decrypt, encrypt_with_nonce, EncryptedMessage, Nonce, SymmetricKey};
use crate::error::{CryptoError, Result};
use crate::identity::NODE_ID_SIZE;
use crate::keyexchange::{
    client_session_keys, server_session_keys, KeyExchangeKeypair, X25519PublicKey,
};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Key exchange request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchangeRequest {
    /// Initiator's node ID
    /// SECURITY C6: 64-byte NodeID for collision resistance
    #[serde(with = "BigArray")]
    pub from_node_id: [u8; NODE_ID_SIZE],

    /// Responder's node ID
    /// SECURITY C6: 64-byte NodeID for collision resistance
    #[serde(with = "BigArray")]
    pub to_node_id: [u8; NODE_ID_SIZE],

    /// Initiator's public key for key exchange
    pub public_key: X25519PublicKey,

    /// Timestamp of request
    pub timestamp: u64,
}

/// Key exchange response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchangeResponse {
    /// Responder's node ID
    /// SECURITY C6: 64-byte NodeID for collision resistance
    #[serde(with = "BigArray")]
    pub from_node_id: [u8; NODE_ID_SIZE],

    /// Initiator's node ID
    /// SECURITY C6: 64-byte NodeID for collision resistance
    #[serde(with = "BigArray")]
    pub to_node_id: [u8; NODE_ID_SIZE],

    /// Responder's public key for key exchange
    pub public_key: X25519PublicKey,

    /// Timestamp of response
    pub timestamp: u64,
}

/// Channel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    /// No key exchange has occurred
    Uninitialized,

    /// Key exchange initiated, waiting for response
    KeyExchangeSent,

    /// Key exchange received, response sent
    KeyExchangeReceived,

    /// Channel is established and ready for encryption
    Established,
}

/// An encrypted channel for end-to-end message encryption
pub struct EncryptedChannel {
    /// Local node ID
    /// SECURITY C6: 64-byte NodeID for collision resistance
    local_node_id: [u8; NODE_ID_SIZE],

    /// Remote node ID
    /// SECURITY C6: 64-byte NodeID for collision resistance
    remote_node_id: Option<[u8; NODE_ID_SIZE]>,

    /// Local keypair for key exchange
    local_keypair: KeyExchangeKeypair,

    /// Remote public key (received during key exchange)
    remote_public_key: Option<X25519PublicKey>,

    /// Transmit key (for encrypting outgoing messages)
    tx_key: Option<SymmetricKey>,

    /// Receive key (for decrypting incoming messages)
    rx_key: Option<SymmetricKey>,

    /// Channel state
    state: ChannelState,

    /// When the channel was established
    established_at: Option<u64>,

    /// SECURITY FIX C4: Atomic nonce counter for guaranteed uniqueness
    /// Using counter-based nonces prevents reuse even with clock issues or RNG failures
    tx_nonce_counter: AtomicU64,
}

impl EncryptedChannel {
    /// Create a new encrypted channel
    ///
    /// SECURITY C6: NodeID is now 64 bytes for collision resistance
    pub fn new(local_node_id: [u8; NODE_ID_SIZE], local_keypair: KeyExchangeKeypair) -> Self {
        EncryptedChannel {
            local_node_id,
            remote_node_id: None,
            local_keypair,
            remote_public_key: None,
            tx_key: None,
            rx_key: None,
            state: ChannelState::Uninitialized,
            established_at: None,
            tx_nonce_counter: AtomicU64::new(0),
        }
    }

    /// Generate next nonce using atomic counter (C4 security fix)
    ///
    /// This ensures nonces are never reused, even in multi-threaded scenarios
    /// or with clock/RNG failures. XSalsa20 has a 192-bit nonce, so we use
    /// 64-bit counter + 64-bit channel ID + 64-bit timestamp for uniqueness.
    fn next_nonce(&self) -> Nonce {
        // Get next counter value atomically
        let counter = self.tx_nonce_counter.fetch_add(1, Ordering::SeqCst);

        // Build 24-byte nonce from:
        // - 8 bytes: counter (guarantees uniqueness within this channel)
        // - 8 bytes: local_node_id prefix (ensures uniqueness across channels)
        // - 8 bytes: timestamp (adds entropy and prevents reuse on restart)
        let mut nonce_bytes = [0u8; 24];
        nonce_bytes[0..8].copy_from_slice(&counter.to_le_bytes());
        nonce_bytes[8..16].copy_from_slice(&self.local_node_id[0..8]);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        nonce_bytes[16..24].copy_from_slice(&timestamp.to_le_bytes());

        Nonce::from_bytes(nonce_bytes)
    }

    /// Get channel state
    pub fn state(&self) -> ChannelState {
        self.state
    }

    /// Check if channel is established
    pub fn is_established(&self) -> bool {
        self.state == ChannelState::Established
    }

    /// Get remote node ID (if set)
    pub fn remote_node_id(&self) -> Option<[u8; NODE_ID_SIZE]> {
        self.remote_node_id
    }

    /// Create a key exchange request to initiate encrypted channel
    ///
    /// SECURITY C6: NodeID is now 64 bytes for collision resistance
    pub fn create_key_exchange_request(
        &mut self,
        remote_node_id: [u8; NODE_ID_SIZE],
    ) -> Result<KeyExchangeRequest> {
        if self.state != ChannelState::Uninitialized {
            return Err(CryptoError::InvalidState(
                "Channel already initialized".to_string(),
            ));
        }

        self.remote_node_id = Some(remote_node_id);
        self.state = ChannelState::KeyExchangeSent;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(KeyExchangeRequest {
            from_node_id: self.local_node_id,
            to_node_id: remote_node_id,
            public_key: X25519PublicKey::from(&self.local_keypair.public_key),
            timestamp,
        })
    }

    /// Process a key exchange request and generate response
    pub fn process_key_exchange_request(
        &mut self,
        request: &KeyExchangeRequest,
    ) -> Result<KeyExchangeResponse> {
        // Verify request is for us
        if request.to_node_id != self.local_node_id {
            return Err(CryptoError::InvalidState(
                "Key exchange request not for this node".to_string(),
            ));
        }

        if self.state != ChannelState::Uninitialized {
            return Err(CryptoError::InvalidState(
                "Channel already initialized".to_string(),
            ));
        }

        // Store remote info
        self.remote_node_id = Some(request.from_node_id);
        self.remote_public_key = Some(request.public_key);

        // Derive session keys (we are the server/responder)
        let session_keys = server_session_keys(&self.local_keypair, &request.public_key)?;

        self.tx_key = Some(session_keys.tx_key);
        self.rx_key = Some(session_keys.rx_key);
        self.state = ChannelState::KeyExchangeReceived;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.established_at = Some(timestamp);
        self.state = ChannelState::Established;

        Ok(KeyExchangeResponse {
            from_node_id: self.local_node_id,
            to_node_id: request.from_node_id,
            public_key: X25519PublicKey::from(&self.local_keypair.public_key),
            timestamp,
        })
    }

    /// Process a key exchange response to complete channel establishment
    pub fn process_key_exchange_response(&mut self, response: &KeyExchangeResponse) -> Result<()> {
        // Verify response is for us
        if response.to_node_id != self.local_node_id {
            return Err(CryptoError::InvalidState(
                "Key exchange response not for this node".to_string(),
            ));
        }

        if self.state != ChannelState::KeyExchangeSent {
            return Err(CryptoError::InvalidState(
                "Not expecting key exchange response".to_string(),
            ));
        }

        // Verify it's from the expected remote node
        if Some(response.from_node_id) != self.remote_node_id {
            return Err(CryptoError::InvalidState(
                "Key exchange response from unexpected node".to_string(),
            ));
        }

        // Store remote public key
        self.remote_public_key = Some(response.public_key);

        // Derive session keys (we are the client/initiator)
        let session_keys = client_session_keys(&self.local_keypair, &response.public_key)?;

        self.tx_key = Some(session_keys.tx_key);
        self.rx_key = Some(session_keys.rx_key);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.established_at = Some(timestamp);
        self.state = ChannelState::Established;

        Ok(())
    }

    /// Encrypt a message for transmission
    pub fn encrypt_message(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        if self.state != ChannelState::Established {
            return Err(CryptoError::InvalidState(
                "Channel not established".to_string(),
            ));
        }

        let tx_key = self
            .tx_key
            .as_ref()
            .ok_or_else(|| CryptoError::InvalidState("No TX key".to_string()))?;

        // SECURITY FIX C4: Use atomic counter-based nonce instead of random
        // This guarantees no nonce reuse even with RNG failures or clock issues
        let nonce = self.next_nonce();
        let encrypted = encrypt_with_nonce(tx_key, plaintext, &nonce)?;

        // Serialize encrypted message (nonce + ciphertext)
        let mut result = Vec::new();
        result.extend_from_slice(encrypted.nonce.as_bytes());
        result.extend_from_slice(&encrypted.ciphertext);

        Ok(result)
    }

    /// Decrypt a received message
    pub fn decrypt_message(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if self.state != ChannelState::Established {
            return Err(CryptoError::InvalidState(
                "Channel not established".to_string(),
            ));
        }

        let rx_key = self
            .rx_key
            .as_ref()
            .ok_or_else(|| CryptoError::InvalidState("No RX key".to_string()))?;

        // Parse nonce and ciphertext
        if ciphertext.len() < 24 {
            return Err(CryptoError::DecryptionFailed);
        }

        let mut nonce_bytes = [0u8; 24];
        nonce_bytes.copy_from_slice(&ciphertext[0..24]);
        let nonce = crate::encryption::Nonce::from_bytes(nonce_bytes);

        let ct = ciphertext[24..].to_vec();

        let encrypted_msg = EncryptedMessage {
            nonce,
            ciphertext: ct,
        };

        decrypt(rx_key, &encrypted_msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_establishment() {
        crate::init().unwrap();

        // Alice initiates
        let alice_node_id = [1u8; NODE_ID_SIZE];
        let alice_kp = KeyExchangeKeypair::generate();
        let mut alice_channel = EncryptedChannel::new(alice_node_id, alice_kp);

        // Bob responds
        let bob_node_id = [2u8; NODE_ID_SIZE];
        let bob_kp = KeyExchangeKeypair::generate();
        let mut bob_channel = EncryptedChannel::new(bob_node_id, bob_kp);

        // Alice creates request
        let kx_request = alice_channel
            .create_key_exchange_request(bob_node_id)
            .unwrap();

        assert_eq!(alice_channel.state(), ChannelState::KeyExchangeSent);

        // Bob processes request
        let kx_response = bob_channel
            .process_key_exchange_request(&kx_request)
            .unwrap();

        assert_eq!(bob_channel.state(), ChannelState::Established);
        assert!(bob_channel.is_established());

        // Alice processes response
        alice_channel
            .process_key_exchange_response(&kx_response)
            .unwrap();

        assert_eq!(alice_channel.state(), ChannelState::Established);
        assert!(alice_channel.is_established());
    }

    #[test]
    fn test_end_to_end_encryption() {
        crate::init().unwrap();

        // Setup channel
        let alice_node_id = [1u8; NODE_ID_SIZE];
        let alice_kp = KeyExchangeKeypair::generate();
        let mut alice_channel = EncryptedChannel::new(alice_node_id, alice_kp);

        let bob_node_id = [2u8; NODE_ID_SIZE];
        let bob_kp = KeyExchangeKeypair::generate();
        let mut bob_channel = EncryptedChannel::new(bob_node_id, bob_kp);

        let kx_request = alice_channel
            .create_key_exchange_request(bob_node_id)
            .unwrap();
        let kx_response = bob_channel
            .process_key_exchange_request(&kx_request)
            .unwrap();
        alice_channel
            .process_key_exchange_response(&kx_response)
            .unwrap();

        // Alice sends message to Bob
        let plaintext = b"Hello from Alice!";
        let encrypted = alice_channel.encrypt_message(plaintext).unwrap();
        let decrypted = bob_channel.decrypt_message(&encrypted).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());

        // Bob sends message to Alice
        let plaintext2 = b"Hello from Bob!";
        let encrypted2 = bob_channel.encrypt_message(plaintext2).unwrap();
        let decrypted2 = alice_channel.decrypt_message(&encrypted2).unwrap();

        assert_eq!(plaintext2.as_slice(), decrypted2.as_slice());
    }

    #[test]
    fn test_encryption_before_establishment_fails() {
        crate::init().unwrap();

        let node_id = [1u8; NODE_ID_SIZE];
        let kp = KeyExchangeKeypair::generate();
        let channel = EncryptedChannel::new(node_id, kp);

        let result = channel.encrypt_message(b"test");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_recipient_fails() {
        crate::init().unwrap();

        let alice_node_id = [1u8; NODE_ID_SIZE];
        let alice_kp = KeyExchangeKeypair::generate();
        let mut alice_channel = EncryptedChannel::new(alice_node_id, alice_kp);

        let bob_node_id = [2u8; NODE_ID_SIZE];

        // Alice creates request for Bob
        let kx_request = alice_channel
            .create_key_exchange_request(bob_node_id)
            .unwrap();

        // Charlie tries to process it
        let charlie_node_id = [3u8; NODE_ID_SIZE];
        let charlie_kp = KeyExchangeKeypair::generate();
        let mut charlie_channel = EncryptedChannel::new(charlie_node_id, charlie_kp);

        let result = charlie_channel.process_key_exchange_request(&kx_request);
        assert!(result.is_err());
    }

    #[test]
    fn test_large_message() {
        crate::init().unwrap();

        // Setup channel
        let alice_node_id = [1u8; NODE_ID_SIZE];
        let alice_kp = KeyExchangeKeypair::generate();
        let mut alice_channel = EncryptedChannel::new(alice_node_id, alice_kp);

        let bob_node_id = [2u8; NODE_ID_SIZE];
        let bob_kp = KeyExchangeKeypair::generate();
        let mut bob_channel = EncryptedChannel::new(bob_node_id, bob_kp);

        let kx_request = alice_channel
            .create_key_exchange_request(bob_node_id)
            .unwrap();
        let kx_response = bob_channel
            .process_key_exchange_request(&kx_request)
            .unwrap();
        alice_channel
            .process_key_exchange_response(&kx_response)
            .unwrap();

        // Send large message
        let plaintext = vec![42u8; 10000];
        let encrypted = alice_channel.encrypt_message(&plaintext).unwrap();
        let decrypted = bob_channel.decrypt_message(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }
}

#[test]
fn test_nonce_uniqueness_sequential() {
    // SECURITY TEST C4: Verify nonces are never reused
    crate::init().unwrap();

    let alice_node_id = [1u8; NODE_ID_SIZE];
    let alice_kp = KeyExchangeKeypair::generate();
    let mut alice_channel = EncryptedChannel::new(alice_node_id, alice_kp);

    let bob_node_id = [2u8; NODE_ID_SIZE];
    let bob_kp = KeyExchangeKeypair::generate();
    let mut bob_channel = EncryptedChannel::new(bob_node_id, bob_kp);

    // Establish channel
    let kx_request = alice_channel
        .create_key_exchange_request(bob_node_id)
        .unwrap();
    let kx_response = bob_channel
        .process_key_exchange_request(&kx_request)
        .unwrap();
    alice_channel
        .process_key_exchange_response(&kx_response)
        .unwrap();

    // Encrypt multiple messages and collect nonces
    use std::collections::HashSet;
    let mut nonces = HashSet::new();

    for i in 0..1000 {
        let plaintext = format!("Message {}", i);
        let encrypted = alice_channel.encrypt_message(plaintext.as_bytes()).unwrap();

        // Extract nonce (first 24 bytes)
        let nonce_bytes = &encrypted[0..24];
        let nonce_array: [u8; 24] = nonce_bytes.try_into().unwrap();

        // Verify this nonce hasn't been seen before
        assert!(
            nonces.insert(nonce_array),
            "Nonce reuse detected at message {}!",
            i
        );
    }

    // Verify we got 1000 unique nonces
    assert_eq!(nonces.len(), 1000);
}

#[test]
fn test_nonce_uniqueness_multithreaded() {
    // SECURITY TEST C4: Verify nonces are unique even with concurrent access
    use std::sync::Arc;
    use std::thread;

    crate::init().unwrap();

    let alice_node_id = [1u8; NODE_ID_SIZE];
    let alice_kp = KeyExchangeKeypair::generate();
    let mut alice_channel = EncryptedChannel::new(alice_node_id, alice_kp);

    let bob_node_id = [2u8; NODE_ID_SIZE];
    let bob_kp = KeyExchangeKeypair::generate();
    let mut bob_channel = EncryptedChannel::new(bob_node_id, bob_kp);

    // Establish channel
    let kx_request = alice_channel
        .create_key_exchange_request(bob_node_id)
        .unwrap();
    let kx_response = bob_channel
        .process_key_exchange_request(&kx_request)
        .unwrap();
    alice_channel
        .process_key_exchange_response(&kx_response)
        .unwrap();

    // Share channel across threads
    let alice_arc = Arc::new(alice_channel);

    // Spawn multiple threads encrypting simultaneously
    let mut handles = vec![];
    for thread_id in 0..10 {
        let alice_clone = Arc::clone(&alice_arc);
        let handle = thread::spawn(move || {
            let mut thread_nonces = Vec::new();
            for i in 0..100 {
                let plaintext = format!("Thread {} Message {}", thread_id, i);
                let encrypted = alice_clone.encrypt_message(plaintext.as_bytes()).unwrap();

                // Extract nonce
                let nonce_bytes = &encrypted[0..24];
                let nonce_array: [u8; 24] = nonce_bytes.try_into().unwrap();
                thread_nonces.push(nonce_array);
            }
            thread_nonces
        });
        handles.push(handle);
    }

    // Collect all nonces from all threads
    use std::collections::HashSet;
    let mut all_nonces = HashSet::new();

    for handle in handles {
        let thread_nonces = handle.join().unwrap();
        for nonce in thread_nonces {
            // Verify no duplicates
            assert!(
                all_nonces.insert(nonce),
                "Nonce reuse detected in multi-threaded scenario!"
            );
        }
    }

    // Verify we got 1000 unique nonces (10 threads × 100 messages)
    assert_eq!(all_nonces.len(), 1000);
}
