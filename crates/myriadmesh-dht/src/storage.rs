//! DHT storage layer for key-value pairs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum DHT storage per node (100MB)
pub const MAX_STORAGE_BYTES: usize = 100 * 1024 * 1024;

/// Maximum number of keys stored
pub const MAX_KEYS: usize = 10_000;

/// Maximum size per value (1MB)
pub const MAX_VALUE_SIZE: usize = 1024 * 1024;

/// A stored value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredValue {
    /// The actual data
    pub data: Vec<u8>,

    /// When this value expires
    pub expires_at: DateTime<Utc>,

    /// When this value was stored
    pub stored_at: DateTime<Utc>,

    /// Publisher's node ID (for verification)
    pub publisher: [u8; 32],

    /// Signature of the data (for verification)
    pub signature: Vec<u8>,
}

impl StoredValue {
    /// Create a new stored value
    pub fn new(
        data: Vec<u8>,
        ttl_seconds: i64,
        publisher: [u8; 32],
        signature: Vec<u8>,
    ) -> Self {
        let now = Utc::now();
        Self {
            data,
            expires_at: now + chrono::Duration::seconds(ttl_seconds),
            stored_at: now,
            publisher,
            signature,
        }
    }

    /// Check if this value has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Get remaining TTL in seconds
    pub fn remaining_ttl(&self) -> i64 {
        (self.expires_at - Utc::now()).num_seconds().max(0)
    }

    /// Get size in bytes
    pub fn size(&self) -> usize {
        self.data.len() + self.signature.len() + 64 // approximate overhead
    }
}

/// DHT storage
pub struct DhtStorage {
    /// Stored values (key -> value)
    store: HashMap<[u8; 32], StoredValue>,

    /// Current storage size in bytes
    total_size: usize,
}

impl DhtStorage {
    /// Create a new DHT storage
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
            total_size: 0,
        }
    }

    /// Store a value
    pub fn insert(
        &mut self,
        key: [u8; 32],
        value: StoredValue,
    ) -> Result<(), StorageError> {
        // Check value size
        if value.data.len() > MAX_VALUE_SIZE {
            return Err(StorageError::ValueTooLarge {
                size: value.data.len(),
                max: MAX_VALUE_SIZE,
            });
        }

        // Check if we have space
        let value_size = value.size();
        if self.total_size + value_size > MAX_STORAGE_BYTES {
            // Try to make space by removing expired values
            self.cleanup_expired();

            // Check again
            if self.total_size + value_size > MAX_STORAGE_BYTES {
                return Err(StorageError::StorageFull {
                    current: self.total_size,
                    max: MAX_STORAGE_BYTES,
                });
            }
        }

        // Check key limit
        if !self.store.contains_key(&key) && self.store.len() >= MAX_KEYS {
            return Err(StorageError::TooManyKeys {
                current: self.store.len(),
                max: MAX_KEYS,
            });
        }

        // Remove old value if exists
        if let Some(old) = self.store.remove(&key) {
            self.total_size -= old.size();
        }

        // Insert new value
        self.total_size += value_size;
        self.store.insert(key, value);

        Ok(())
    }

    /// Retrieve a value
    pub fn get(&self, key: &[u8; 32]) -> Option<&StoredValue> {
        self.store.get(key).filter(|v| !v.is_expired())
    }

    /// Remove a value
    pub fn remove(&mut self, key: &[u8; 32]) -> Option<StoredValue> {
        if let Some(value) = self.store.remove(key) {
            self.total_size -= value.size();
            Some(value)
        } else {
            None
        }
    }

    /// Remove all expired values
    pub fn cleanup_expired(&mut self) -> usize {
        let mut removed_count = 0;
        let mut to_remove = Vec::new();

        for (key, value) in &self.store {
            if value.is_expired() {
                to_remove.push(*key);
            }
        }

        for key in to_remove {
            if let Some(value) = self.store.remove(&key) {
                self.total_size -= value.size();
                removed_count += 1;
            }
        }

        removed_count
    }

    /// Get all keys
    pub fn keys(&self) -> Vec<[u8; 32]> {
        self.store.keys().copied().collect()
    }

    /// Get values expiring soon (within the next hour)
    pub fn expiring_soon(&self) -> Vec<([u8; 32], &StoredValue)> {
        let threshold = Utc::now() + chrono::Duration::hours(1);
        self.store
            .iter()
            .filter(|(_, v)| !v.is_expired() && v.expires_at < threshold)
            .map(|(k, v)| (*k, v))
            .collect()
    }

    /// Get current storage size
    pub fn size(&self) -> usize {
        self.total_size
    }

    /// Get number of stored keys
    pub fn key_count(&self) -> usize {
        self.store.len()
    }

    /// Check if storage is responsible for a key (within our k-bucket range)
    /// This is a simplified check; actual responsibility depends on k-closest nodes
    pub fn is_responsible_for_key(&self, _key: &[u8; 32]) -> bool {
        // For now, accept all keys
        // In a full implementation, this would check if we're one of the k-closest nodes
        true
    }
}

impl Default for DhtStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage errors
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Value too large: {size} bytes (max {max})")]
    ValueTooLarge { size: usize, max: usize },

    #[error("Storage full: {current} bytes (max {max})")]
    StorageFull { current: usize, max: usize },

    #[error("Too many keys: {current} (max {max})")]
    TooManyKeys { current: usize, max: usize },

    #[error("Key not found")]
    KeyNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_value(size: usize, ttl_seconds: i64) -> StoredValue {
        StoredValue::new(
            vec![0u8; size],
            ttl_seconds,
            [1u8; 32],
            vec![2u8; 64],
        )
    }

    #[test]
    fn test_stored_value_expiry() {
        let value = create_test_value(100, 10); // 10 seconds TTL
        assert!(!value.is_expired());
        assert!(value.remaining_ttl() > 0);
    }

    #[test]
    fn test_storage_insert_get() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value = create_test_value(100, 3600);

        storage.insert(key, value).unwrap();
        assert!(storage.get(&key).is_some());
        assert_eq!(storage.key_count(), 1);
    }

    #[test]
    fn test_storage_size_limit() {
        let mut storage = DhtStorage::new();

        // Create a large value
        let large_value = create_test_value(MAX_VALUE_SIZE + 1, 3600);
        let result = storage.insert([1u8; 32], large_value);

        assert!(matches!(result, Err(StorageError::ValueTooLarge { .. })));
    }

    #[test]
    fn test_storage_cleanup_expired() {
        let mut storage = DhtStorage::new();

        // Add some values with short TTL
        for i in 0..5 {
            let key = [i; 32];
            let value = create_test_value(100, -1); // Already expired
            storage.insert(key, value).ok(); // Ignore errors
        }

        // Add some values with long TTL
        for i in 5..10 {
            let key = [i; 32];
            let value = create_test_value(100, 3600);
            storage.insert(key, value).unwrap();
        }

        // Cleanup should remove expired values
        let removed = storage.cleanup_expired();
        assert!(removed > 0);
        assert!(storage.key_count() <= 5);
    }

    #[test]
    fn test_storage_update_value() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];

        // Insert initial value
        let value1 = create_test_value(100, 3600);
        storage.insert(key, value1).unwrap();

        let initial_size = storage.size();

        // Update with larger value
        let value2 = create_test_value(200, 3600);
        storage.insert(key, value2).unwrap();

        // Should still have 1 key
        assert_eq!(storage.key_count(), 1);

        // Size should have increased
        assert!(storage.size() > initial_size);
    }

    #[test]
    fn test_storage_remove() {
        let mut storage = DhtStorage::new();
        let key = [1u8; 32];
        let value = create_test_value(100, 3600);

        storage.insert(key, value).unwrap();
        let initial_size = storage.size();

        let removed = storage.remove(&key);
        assert!(removed.is_some());
        assert_eq!(storage.key_count(), 0);
        assert!(storage.size() < initial_size);
    }

    #[test]
    fn test_storage_expiring_soon() {
        let mut storage = DhtStorage::new();

        // Add value expiring in 30 minutes
        let key1 = [1u8; 32];
        let value1 = create_test_value(100, 30 * 60);
        storage.insert(key1, value1).unwrap();

        // Add value expiring in 2 hours
        let key2 = [2u8; 32];
        let value2 = create_test_value(100, 2 * 3600);
        storage.insert(key2, value2).unwrap();

        let expiring = storage.expiring_soon();
        assert_eq!(expiring.len(), 1); // Only first value expires within 1 hour
    }
}
