//! DHT storage for key-value pairs

use crate::error::{DhtError, Result};
use crate::{MAX_DHT_KEYS, MAX_DHT_STORAGE_BYTES, MAX_VALUE_SIZE};
use serde::{Deserialize, Serialize};
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

    /// Publisher node ID
    pub publisher: Option<[u8; 32]>,
}

impl StorageEntry {
    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        now() >= self.expires_at
    }

    /// Get remaining TTL in seconds
    pub fn ttl_remaining(&self) -> u64 {
        let current = now();
        if current >= self.expires_at {
            0
        } else {
            self.expires_at - current
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
    pub fn store(
        &mut self,
        key: [u8; 32],
        value: Vec<u8>,
        ttl_secs: u64,
        publisher: Option<[u8; 32]>,
    ) -> Result<()> {
        // Check value size
        if value.len() > MAX_VALUE_SIZE {
            return Err(DhtError::ValueTooLarge {
                size: value.len(),
                max: MAX_VALUE_SIZE,
            });
        }

        // If key exists, remove old value first for accurate size tracking
        if let Some(old_entry) = self.entries.remove(&key) {
            self.current_size -= old_entry.value.len();
        }

        // Check capacity
        if !self.has_capacity(value.len()) {
            // Try to make space by removing expired entries
            self.cleanup_expired();

            if !self.has_capacity(value.len()) {
                return Err(DhtError::StorageFull {
                    max: self.max_size,
                });
            }
        }

        // Create entry
        let entry = StorageEntry {
            key,
            value: value.clone(),
            stored_at: now(),
            expires_at: now() + ttl_secs,
            publisher,
        };

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

        storage.store(key, value.clone(), 3600, None).unwrap();

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

        let result = storage.store(key, value, 3600, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_storage_full() {
        let mut storage = DhtStorage::with_limits(100, 5);
        let key = [1u8; 32];
        let value = vec![0u8; 101]; // Too large for capacity

        let result = storage.store(key, value, 3600, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_existing() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value1 = b"first value".to_vec();
        let value2 = b"second value".to_vec();

        storage.store(key, value1, 3600, None).unwrap();
        assert_eq!(storage.key_count(), 1);

        storage.store(key, value2.clone(), 3600, None).unwrap();
        assert_eq!(storage.key_count(), 1); // Still only 1 entry

        let retrieved = storage.get(&key).unwrap();
        assert_eq!(retrieved.value, value2);
    }

    #[test]
    fn test_remove() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value = b"test".to_vec();

        storage.store(key, value, 3600, None).unwrap();
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

        // Store with 0 TTL (immediately expired)
        storage.store(key, value, 0, None).unwrap();

        // Should not be retrievable
        assert!(storage.get(&key).is_none());
    }

    #[test]
    fn test_cleanup_expired() {
        let mut storage = DhtStorage::new();

        // Add expired entry
        let key1 = [1u8; 32];
        storage.store(key1, b"expired".to_vec(), 0, None).unwrap();

        // Add valid entry
        let key2 = [2u8; 32];
        storage.store(key2, b"valid".to_vec(), 3600, None).unwrap();

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

        storage.store(key, value, 3600, None).unwrap();
        assert_eq!(storage.key_count(), 1);

        storage.clear();
        assert_eq!(storage.key_count(), 0);
        assert_eq!(storage.size(), 0);
    }

    #[test]
    fn test_ttl_remaining() {
        let entry = StorageEntry {
            key: [0u8; 32],
            value: vec![],
            stored_at: now(),
            expires_at: now() + 3600,
            publisher: None,
        };

        let ttl = entry.ttl_remaining();
        assert!(ttl > 0 && ttl <= 3600);
    }
}
