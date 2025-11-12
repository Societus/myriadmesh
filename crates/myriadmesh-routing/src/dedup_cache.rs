//! Message deduplication cache
//!
//! Prevents routing duplicate messages using an LRU cache of message IDs

use lru::LruCache;
use myriadmesh_protocol::MessageId;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Default cache size (100,000 messages)
const DEFAULT_CACHE_SIZE: usize = 100_000;

/// Deduplication cache
pub struct DedupCache {
    /// LRU cache of message IDs
    cache: Arc<Mutex<LruCache<MessageId, std::time::Instant>>>,
}

impl DedupCache {
    /// Create a new dedup cache with default size
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CACHE_SIZE)
    }

    /// Create a new dedup cache with specific capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(capacity))),
        }
    }

    /// Check if we've seen this message before
    /// Returns true if this is a duplicate
    pub async fn is_duplicate(&self, message_id: &MessageId) -> bool {
        let cache = self.cache.lock().await;
        cache.contains(message_id)
    }

    /// Mark a message as seen
    /// Returns true if this was a new message (not a duplicate)
    pub async fn mark_seen(&self, message_id: MessageId) -> bool {
        let mut cache = self.cache.lock().await;
        let was_new = !cache.contains(&message_id);
        cache.put(message_id, std::time::Instant::now());
        was_new
    }

    /// Check and mark in one operation
    /// Returns true if message is a duplicate (was already seen)
    pub async fn check_and_mark(&self, message_id: MessageId) -> bool {
        let mut cache = self.cache.lock().await;
        let is_duplicate = cache.contains(&message_id);
        cache.put(message_id, std::time::Instant::now());
        is_duplicate
    }

    /// Get cache size (number of entries)
    pub async fn len(&self) -> usize {
        self.cache.lock().await.len()
    }

    /// Check if cache is empty
    pub async fn is_empty(&self) -> bool {
        self.cache.lock().await.is_empty()
    }

    /// Clear the cache
    pub async fn clear(&self) {
        self.cache.lock().await.clear();
    }

    /// Remove old entries (older than duration)
    /// This is a manual cleanup; LRU handles most eviction automatically
    pub async fn cleanup_old(&self, max_age: std::time::Duration) -> usize {
        let mut cache = self.cache.lock().await;
        let now = std::time::Instant::now();
        let mut to_remove = Vec::new();

        // Collect old entries
        for (id, timestamp) in cache.iter() {
            if now.duration_since(*timestamp) > max_age {
                to_remove.push(*id);
            }
        }

        // Remove them
        let count = to_remove.len();
        for id in to_remove {
            cache.pop(&id);
        }

        count
    }
}

impl Default for DedupCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_message_id(byte: u8) -> MessageId {
        MessageId::from_bytes([byte; 32])
    }

    #[tokio::test]
    async fn test_mark_seen() {
        let cache = DedupCache::new();
        let id = create_message_id(1);

        assert!(!cache.is_duplicate(&id).await);
        assert!(cache.mark_seen(id).await); // Returns true (new message)
        assert!(cache.is_duplicate(&id).await);
        assert!(!cache.mark_seen(id).await); // Returns false (duplicate)
    }

    #[tokio::test]
    async fn test_check_and_mark() {
        let cache = DedupCache::new();
        let id = create_message_id(1);

        assert!(!cache.check_and_mark(id).await); // Not a duplicate (first time)
        assert!(cache.check_and_mark(id).await); // Is a duplicate (second time)
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let cache = DedupCache::with_capacity(3);

        // Add 3 messages
        for i in 1..=3 {
            cache.mark_seen(create_message_id(i)).await;
        }

        assert_eq!(cache.len().await, 3);

        // Add a 4th message, should evict the first one
        cache.mark_seen(create_message_id(4)).await;

        assert_eq!(cache.len().await, 3);
        assert!(!cache.is_duplicate(&create_message_id(1)).await); // Evicted
        assert!(cache.is_duplicate(&create_message_id(2)).await); // Still there
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = DedupCache::new();

        for i in 1..=10 {
            cache.mark_seen(create_message_id(i)).await;
        }

        assert_eq!(cache.len().await, 10);

        cache.clear().await;
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_cleanup_old() {
        let cache = DedupCache::new();

        // Mark a message as seen
        cache.mark_seen(create_message_id(1)).await;

        // Wait a bit
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Cleanup messages older than 50ms
        let removed = cache
            .cleanup_old(std::time::Duration::from_millis(50))
            .await;

        assert_eq!(removed, 1);
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let cache = Arc::new(DedupCache::new());
        let mut handles = vec![];

        // Spawn multiple tasks adding messages
        for i in 0..10 {
            let cache = Arc::clone(&cache);
            let handle = tokio::spawn(async move {
                for j in 0..100 {
                    let id = create_message_id((i * 100 + j) as u8);
                    cache.mark_seen(id).await;
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // Should have messages from all tasks
        assert!(cache.len().await > 0);
    }
}
