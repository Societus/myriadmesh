//! Priority queue system for message routing
//!
//! 5-level priority queue based on Priority values (0-255)

use myriadmesh_protocol::{Frame, Priority};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Number of priority queues
pub const NUM_QUEUES: usize = 5;

/// Maximum messages per queue
const MAX_QUEUE_SIZE: usize = 10_000;

/// Priority queue entry
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    /// The frame to send
    pub frame: Frame,

    /// Priority (for ordering within queue)
    pub priority: Priority,

    /// When this was queued
    pub queued_at: std::time::Instant,

    /// Number of retry attempts
    pub retry_count: u32,
}

/// Multi-level priority queue
pub struct PriorityQueue {
    /// 5 queues: Background, Low, Normal, High, Emergency
    queues: [Arc<Mutex<VecDeque<QueuedMessage>>>; NUM_QUEUES],

    /// Total messages across all queues
    total_size: Arc<Mutex<usize>>,
}

impl PriorityQueue {
    /// Create a new priority queue
    pub fn new() -> Self {
        Self {
            queues: [
                Arc::new(Mutex::new(VecDeque::new())), // Queue 0: Background
                Arc::new(Mutex::new(VecDeque::new())), // Queue 1: Low
                Arc::new(Mutex::new(VecDeque::new())), // Queue 2: Normal
                Arc::new(Mutex::new(VecDeque::new())), // Queue 3: High
                Arc::new(Mutex::new(VecDeque::new())), // Queue 4: Emergency
            ],
            total_size: Arc::new(Mutex::new(0)),
        }
    }

    /// Enqueue a message
    pub async fn enqueue(&self, frame: Frame, priority: Priority) -> Result<(), QueueError> {
        let queue_idx = priority.queue_index();

        // Check total size
        let mut total = self.total_size.lock().await;
        if *total >= MAX_QUEUE_SIZE * NUM_QUEUES {
            return Err(QueueError::QueueFull);
        }

        let mut queue = self.queues[queue_idx].lock().await;

        // Check per-queue size
        if queue.len() >= MAX_QUEUE_SIZE {
            return Err(QueueError::QueueFull);
        }

        queue.push_back(QueuedMessage {
            frame,
            priority,
            queued_at: std::time::Instant::now(),
            retry_count: 0,
        });

        *total += 1;
        Ok(())
    }

    /// Dequeue highest priority message
    /// Emergency > High > Normal > Low > Background
    pub async fn dequeue(&self) -> Option<QueuedMessage> {
        // Try queues from highest to lowest priority
        for queue_idx in (0..NUM_QUEUES).rev() {
            let mut queue = self.queues[queue_idx].lock().await;
            if let Some(msg) = queue.pop_front() {
                let mut total = self.total_size.lock().await;
                *total = total.saturating_sub(1);
                return Some(msg);
            }
        }

        None
    }

    /// Peek at the next message without removing it
    pub async fn peek(&self) -> Option<Priority> {
        for queue_idx in (0..NUM_QUEUES).rev() {
            let queue = self.queues[queue_idx].lock().await;
            if let Some(msg) = queue.front() {
                return Some(msg.priority);
            }
        }
        None
    }

    /// Get total number of queued messages
    pub async fn len(&self) -> usize {
        *self.total_size.lock().await
    }

    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Get size of a specific priority queue
    pub async fn queue_size(&self, queue_idx: usize) -> usize {
        if queue_idx >= NUM_QUEUES {
            return 0;
        }
        self.queues[queue_idx].lock().await.len()
    }

    /// Clear all queues
    pub async fn clear(&self) {
        for queue in &self.queues {
            queue.lock().await.clear();
        }
        *self.total_size.lock().await = 0;
    }

    /// Remove old messages (older than timeout)
    pub async fn cleanup_old_messages(&self, timeout: std::time::Duration) -> usize {
        let mut removed = 0;
        let now = std::time::Instant::now();

        for queue in &self.queues {
            let mut q = queue.lock().await;
            let original_len = q.len();

            // Retain only messages newer than timeout
            q.retain(|msg| now.duration_since(msg.queued_at) < timeout);

            removed += original_len - q.len();
        }

        if removed > 0 {
            let mut total = self.total_size.lock().await;
            *total = total.saturating_sub(removed);
        }

        removed
    }

    /// Get statistics for each queue
    pub async fn stats(&self) -> QueueStats {
        let mut stats = QueueStats {
            queue_sizes: [0; NUM_QUEUES],
            total: 0,
        };

        for (idx, queue) in self.queues.iter().enumerate() {
            let size = queue.lock().await.len();
            stats.queue_sizes[idx] = size;
            stats.total += size;
        }

        stats
    }
}

impl Default for PriorityQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Queue statistics
#[derive(Debug, Clone)]
pub struct QueueStats {
    /// Size of each queue (0=Background, 1=Low, 2=Normal, 3=High, 4=Emergency)
    pub queue_sizes: [usize; NUM_QUEUES],

    /// Total messages
    pub total: usize,
}

/// Queue errors
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue is full")]
    QueueFull,

    #[error("Invalid priority queue index: {0}")]
    InvalidQueue(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use myriadmesh_protocol::frame::{FrameHeader, MAGIC_BYTES, PROTOCOL_VERSION};

    fn create_test_frame(priority: Priority) -> Frame {
        Frame {
            header: FrameHeader {
                magic: MAGIC_BYTES,
                version: PROTOCOL_VERSION,
                flags: 0,
                payload_length: 10,
                checksum: 0,
            },
            payload: vec![0u8; 10],
        }
    }

    #[tokio::test]
    async fn test_enqueue_dequeue() {
        let queue = PriorityQueue::new();

        // Enqueue normal priority
        let frame = create_test_frame(Priority::NORMAL);
        queue.enqueue(frame, Priority::NORMAL).await.unwrap();

        assert_eq!(queue.len().await, 1);

        // Dequeue
        let msg = queue.dequeue().await.unwrap();
        assert_eq!(msg.priority, Priority::NORMAL);
        assert_eq!(queue.len().await, 0);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = PriorityQueue::new();

        // Enqueue in random order
        queue
            .enqueue(create_test_frame(Priority::LOW), Priority::LOW)
            .await
            .unwrap();
        queue
            .enqueue(create_test_frame(Priority::EMERGENCY), Priority::EMERGENCY)
            .await
            .unwrap();
        queue
            .enqueue(create_test_frame(Priority::NORMAL), Priority::NORMAL)
            .await
            .unwrap();

        // Should dequeue in priority order
        assert_eq!(queue.dequeue().await.unwrap().priority, Priority::EMERGENCY);
        assert_eq!(queue.dequeue().await.unwrap().priority, Priority::NORMAL);
        assert_eq!(queue.dequeue().await.unwrap().priority, Priority::LOW);
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let queue = PriorityQueue::new();

        queue
            .enqueue(create_test_frame(Priority::EMERGENCY), Priority::EMERGENCY)
            .await
            .unwrap();
        queue
            .enqueue(create_test_frame(Priority::HIGH), Priority::HIGH)
            .await
            .unwrap();
        queue
            .enqueue(create_test_frame(Priority::HIGH), Priority::HIGH)
            .await
            .unwrap();

        let stats = queue.stats().await;
        assert_eq!(stats.total, 3);
        assert_eq!(stats.queue_sizes[4], 1); // Emergency
        assert_eq!(stats.queue_sizes[3], 2); // High
    }

    #[tokio::test]
    async fn test_cleanup_old_messages() {
        let queue = PriorityQueue::new();

        queue
            .enqueue(create_test_frame(Priority::NORMAL), Priority::NORMAL)
            .await
            .unwrap();

        // Wait a bit
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Cleanup messages older than 50ms
        let removed = queue
            .cleanup_old_messages(std::time::Duration::from_millis(50))
            .await;

        assert_eq!(removed, 1);
        assert_eq!(queue.len().await, 0);
    }

    #[tokio::test]
    async fn test_peek() {
        let queue = PriorityQueue::new();

        assert!(queue.peek().await.is_none());

        queue
            .enqueue(create_test_frame(Priority::HIGH), Priority::HIGH)
            .await
            .unwrap();

        // Peek should return priority without removing
        assert_eq!(queue.peek().await, Some(Priority::HIGH));
        assert_eq!(queue.len().await, 1);
    }

    #[tokio::test]
    async fn test_clear() {
        let queue = PriorityQueue::new();

        for _ in 0..10 {
            queue
                .enqueue(create_test_frame(Priority::NORMAL), Priority::NORMAL)
                .await
                .unwrap();
        }

        assert_eq!(queue.len().await, 10);

        queue.clear().await;
        assert_eq!(queue.len().await, 0);
    }
}
