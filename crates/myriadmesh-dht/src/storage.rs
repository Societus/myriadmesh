//! DHT storage for key-value pairs

use crate::error::{DhtError, Result};
use crate::{MAX_DHT_KEYS, MAX_DHT_STORAGE_BYTES, MAX_VALUE_SIZE};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// A stored value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageEntry {
    /// The key (32 bytes)
    pub key: [u8; 32],

    /// The value
    pub value: Vec<u8>,

    /// When this entry was stored
    pub stored_at: u64,

    /// When this entry expires (Unix timestamp)
    pub expires_at: u64,

    /// Publisher node ID (REQUIRED for signature verification)
    /// SECURITY H7: Required for value poisoning prevention
    pub publisher: [u8; 32],

    /// Ed25519 signature over (key || value || expires_at)
    /// SECURITY H7: Signature from publisher to prevent DHT poisoning
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

impl StorageEntry {
    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        now() >= self.expires_at
    }

    /// Get remaining TTL in seconds
    pub fn ttl_remaining(&self) -> u64 {
        let current = now();
        self.expires_at.saturating_sub(current)
    }

    /// SECURITY H7: Verify signature on stored value
    /// Prevents DHT value poisoning by ensuring publisher authenticity
    pub fn verify_signature(&self) -> Result<()> {
        use sodiumoxide::crypto::sign::ed25519;

        // Build message to verify: key || value || expires_at
        let mut message = Vec::new();
        message.extend_from_slice(&self.key);
        message.extend_from_slice(&self.value);
        message.extend_from_slice(&self.expires_at.to_le_bytes());

        // Extract signature (ed25519::Signature uses from_bytes which returns a Result)
        let signature = ed25519::Signature::from_bytes(&self.signature)
            .map_err(|_| DhtError::InvalidSignature)?;

        // Derive public key from publisher node ID
        // In MyriadMesh, NodeID = BLAKE2b(public_key)
        // For verification, we need to store the actual public key or have it provided
        // For now, we'll assume publisher is the public key itself (32 bytes)
        // NOTE: This may need adjustment based on actual NodeID derivation
        let public_key = ed25519::PublicKey::from_slice(&self.publisher)
            .ok_or(DhtError::InvalidPublicKey)?;

        // Verify signature
        if ed25519::verify_detached(&signature, &message, &public_key) {
            Ok(())
        } else {
            Err(DhtError::InvalidSignature)
        }
    }
}

/// DHT storage layer
#[derive(Debug)]
pub struct DhtStorage {
    /// Stored entries by key
    entries: HashMap<[u8; 32], StorageEntry>,

    /// Current storage size in bytes
    current_size: usize,

    /// Maximum storage size
    max_size: usize,

    /// Maximum number of keys
    max_keys: usize,
}

impl DhtStorage {
    /// Create new DHT storage
    pub fn new() -> Self {
        DhtStorage {
            entries: HashMap::new(),
            current_size: 0,
            max_size: MAX_DHT_STORAGE_BYTES,
            max_keys: MAX_DHT_KEYS,
        }
    }

    /// Create with custom limits
    pub fn with_limits(max_size: usize, max_keys: usize) -> Self {
        DhtStorage {
            entries: HashMap::new(),
            current_size: 0,
            max_size,
            max_keys,
        }
    }

    /// Get current storage size in bytes
    pub fn size(&self) -> usize {
        self.current_size
    }

    /// Get number of stored keys
    pub fn key_count(&self) -> usize {
        self.entries.len()
    }

    /// Check if storage has capacity for a value
    fn has_capacity(&self, value_size: usize) -> bool {
        self.key_count() < self.max_keys && (self.current_size + value_size) <= self.max_size
    }

    /// Store a value
    /// SECURITY H7: Requires valid signature from publisher
    pub fn store(
        &mut self,
        key: [u8; 32],
        value: Vec<u8>,
        ttl_secs: u64,
        publisher: [u8; 32],
        signature: [u8; 64],
    ) -> Result<()> {
        // Check value size
        if value.len() > MAX_VALUE_SIZE {
            return Err(DhtError::ValueTooLarge {
                size: value.len(),
                max: MAX_VALUE_SIZE,
            });
        }

        let expires_at = now() + ttl_secs;

        // Create entry for verification
        let entry = StorageEntry {
            key,
            value: value.clone(),
            stored_at: now(),
            expires_at,
            publisher,
            signature,
        };

        // SECURITY H7: Verify signature before storing
        entry.verify_signature()?;

        // If key exists, remove old value first for accurate size tracking
        if let Some(old_entry) = self.entries.remove(&key) {
            self.current_size -= old_entry.value.len();
        }

        // Check capacity
        if !self.has_capacity(value.len()) {
            // Try to make space by removing expired entries
            self.cleanup_expired();

            if !self.has_capacity(value.len()) {
                return Err(DhtError::StorageFull { max: self.max_size });
            }
        }

        // Store
        self.entries.insert(key, entry);
        self.current_size += value.len();

        Ok(())
    }

    /// Retrieve a value
    pub fn get(&self, key: &[u8; 32]) -> Option<&StorageEntry> {
        self.entries.get(key).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                Some(entry)
            }
        })
    }

    /// Remove a value
    pub fn remove(&mut self, key: &[u8; 32]) -> Option<StorageEntry> {
        if let Some(entry) = self.entries.remove(key) {
            self.current_size -= entry.value.len();
            Some(entry)
        } else {
            None
        }
    }

    /// Cleanup expired entries
    pub fn cleanup_expired(&mut self) -> usize {
        let mut removed = 0;
        let current_time = now();

        self.entries.retain(|_, entry| {
            if entry.expires_at <= current_time {
                self.current_size -= entry.value.len();
                removed += 1;
                false
            } else {
                true
            }
        });

        removed
    }

    /// Get all entries (for republishing)
    pub fn get_all_entries(&self) -> Vec<&StorageEntry> {
        self.entries
            .values()
            .filter(|entry| !entry.is_expired())
            .collect()
    }

    /// Get entries that need republishing
    pub fn get_expiring_entries(&self, within_secs: u64) -> Vec<&StorageEntry> {
        let threshold = now() + within_secs;

        self.entries
            .values()
            .filter(|entry| !entry.is_expired() && entry.expires_at <= threshold)
            .collect()
    }

    /// Clear all storage
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_size = 0;
    }
}

impl Default for DhtStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sodiumoxide::crypto::sign::ed25519;

    /// Helper to sign a DHT value for testing
    /// Returns signature bytes
    fn sign_value(key: &[u8; 32], value: &[u8], expires_at: u64, sk: &ed25519::SecretKey) -> [u8; 64] {
        // Build message to sign: key || value || expires_at
        let mut message = Vec::new();
        message.extend_from_slice(key);
        message.extend_from_slice(value);
        message.extend_from_slice(&expires_at.to_le_bytes());

        // Sign
        let signature = ed25519::sign_detached(&message, sk);
        signature.to_bytes()
    }

    /// Helper to create a keypair and sign a value
    fn create_signed_value(key: [u8; 32], value: Vec<u8>, ttl_secs: u64) -> ([u8; 32], [u8; 64]) {
        sodiumoxide::init().unwrap();
        let (pk, sk) = ed25519::gen_keypair();
        let expires_at = now() + ttl_secs;
        let signature = sign_value(&key, &value, expires_at, &sk);
        let mut pk_bytes = [0u8; 32];
        pk_bytes.copy_from_slice(&pk[..]);
        (pk_bytes, signature)
    }

    #[test]
    fn test_new_storage() {
        let storage = DhtStorage::new();
        assert_eq!(storage.size(), 0);
        assert_eq!(storage.key_count(), 0);
    }

    #[test]
    fn test_store_and_retrieve() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value = b"test value".to_vec();
        let (publisher, signature) = create_signed_value(key, value.clone(), 3600);

        storage.store(key, value.clone(), 3600, publisher, signature).unwrap();

        assert_eq!(storage.key_count(), 1);
        assert!(storage.size() > 0);

        let retrieved = storage.get(&key).unwrap();
        assert_eq!(retrieved.value, value);
    }

    #[test]
    fn test_store_too_large() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value = vec![0u8; MAX_VALUE_SIZE + 1];
        let (publisher, signature) = create_signed_value(key, value.clone(), 3600);

        let result = storage.store(key, value, 3600, publisher, signature);
        assert!(result.is_err());
    }

    #[test]
    fn test_storage_full() {
        let mut storage = DhtStorage::with_limits(100, 5);
        let key = [1u8; 32];
        let value = vec![0u8; 101]; // Too large for capacity
        let (publisher, signature) = create_signed_value(key, value.clone(), 3600);

        let result = storage.store(key, value, 3600, publisher, signature);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_existing() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value1 = b"first value".to_vec();
        let value2 = b"second value".to_vec();

        let (publisher1, signature1) = create_signed_value(key, value1.clone(), 3600);
        storage.store(key, value1, 3600, publisher1, signature1).unwrap();
        assert_eq!(storage.key_count(), 1);

        let (publisher2, signature2) = create_signed_value(key, value2.clone(), 3600);
        storage.store(key, value2.clone(), 3600, publisher2, signature2).unwrap();
        assert_eq!(storage.key_count(), 1); // Still only 1 entry

        let retrieved = storage.get(&key).unwrap();
        assert_eq!(retrieved.value, value2);
    }

    #[test]
    fn test_remove() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value = b"test".to_vec();
        let (publisher, signature) = create_signed_value(key, value.clone(), 3600);

        storage.store(key, value, 3600, publisher, signature).unwrap();
        assert_eq!(storage.key_count(), 1);

        let removed = storage.remove(&key);
        assert!(removed.is_some());
        assert_eq!(storage.key_count(), 0);
        assert_eq!(storage.size(), 0);
    }

    #[test]
    fn test_expired_entry() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value = b"test".to_vec();
        let (publisher, signature) = create_signed_value(key, value.clone(), 0);

        // Store with 0 TTL (immediately expired)
        storage.store(key, value, 0, publisher, signature).unwrap();

        // Should not be retrievable
        assert!(storage.get(&key).is_none());
    }

    #[test]
    fn test_cleanup_expired() {
        let mut storage = DhtStorage::new();

        // Add expired entry
        let key1 = [1u8; 32];
        let value1 = b"expired".to_vec();
        let (publisher1, signature1) = create_signed_value(key1, value1.clone(), 0);
        storage.store(key1, value1, 0, publisher1, signature1).unwrap();

        // Add valid entry
        let key2 = [2u8; 32];
        let value2 = b"valid".to_vec();
        let (publisher2, signature2) = create_signed_value(key2, value2.clone(), 3600);
        storage.store(key2, value2, 3600, publisher2, signature2).unwrap();

        assert_eq!(storage.key_count(), 2);

        let removed = storage.cleanup_expired();
        assert_eq!(removed, 1);
        assert_eq!(storage.key_count(), 1);
    }

    #[test]
    fn test_clear() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value = b"test".to_vec();
        let (publisher, signature) = create_signed_value(key, value.clone(), 3600);

        storage.store(key, value, 3600, publisher, signature).unwrap();
        assert_eq!(storage.key_count(), 1);

        storage.clear();
        assert_eq!(storage.key_count(), 0);
        assert_eq!(storage.size(), 0);
    }

    #[test]
    fn test_ttl_remaining() {
        sodiumoxide::init().unwrap();
        let (pk, _sk) = ed25519::gen_keypair();
        let mut pk_bytes = [0u8; 32];
        pk_bytes.copy_from_slice(&pk[..]);

        let entry = StorageEntry {
            key: [0u8; 32],
            value: vec![],
            stored_at: now(),
            expires_at: now() + 3600,
            publisher: pk_bytes,
            signature: [0u8; 64],
        };

        let ttl = entry.ttl_remaining();
        assert!(ttl > 0 && ttl <= 3600);
    }

    #[test]
    fn test_invalid_signature_rejected() {
        // SECURITY TEST H7: Verify invalid signatures are rejected
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value = b"test value".to_vec();

        sodiumoxide::init().unwrap();
        let (pk, _sk) = ed25519::gen_keypair();
        let mut pk_bytes = [0u8; 32];
        pk_bytes.copy_from_slice(&pk[..]);
        let invalid_signature = [0u8; 64]; // Invalid signature

        let result = storage.store(key, value, 3600, pk_bytes, invalid_signature);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DhtError::InvalidSignature));
    }

    #[test]
    fn test_tampered_value_rejected() {
        // SECURITY TEST H7: Verify tampered values are rejected
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let original_value = b"original value".to_vec();
        let tampered_value = b"tampered value".to_vec();

        // Sign the original value
        let (publisher, signature) = create_signed_value(key, original_value.clone(), 3600);

        // Try to store tampered value with original signature
        let result = storage.store(key, tampered_value, 3600, publisher, signature);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DhtError::InvalidSignature));
    }

    #[test]
    fn test_valid_signature_accepted() {
        // SECURITY TEST H7: Verify valid signatures are accepted
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value = b"test value".to_vec();
        let (publisher, signature) = create_signed_value(key, value.clone(), 3600);

        let result = storage.store(key, value, 3600, publisher, signature);
        assert!(result.is_ok());
        assert_eq!(storage.key_count(), 1);
    }

    #[test]
    fn test_wrong_key_signature_rejected() {
        // SECURITY TEST H7: Verify signature for different key is rejected
        let mut storage = DhtStorage::new();
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let value = b"test value".to_vec();

        // Sign with key1
        let (publisher, signature) = create_signed_value(key1, value.clone(), 3600);

        // Try to store with key2 but signature for key1
        let result = storage.store(key2, value, 3600, publisher, signature);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DhtError::InvalidSignature));
    }
}
