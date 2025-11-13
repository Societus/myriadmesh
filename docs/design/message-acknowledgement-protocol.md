# Message Acknowledgement Protocol

**Status:** Design Phase
**Version:** 1.0
**Last Updated:** 2025-01-13

---

## Overview

The Message Acknowledgement Protocol enables nodes to confirm message receipt and relay across multiple network adapters using a round-robin approach. This serves dual purposes:
1. **Reliable delivery confirmation** - Sender knows message was received
2. **Adapter health verification** - Tests multiple adapters without separate heartbeats

---

## Core Principles

1. **Round-Robin Acknowledgement**: Use different adapter than original message when possible
2. **Implicit Proof-of-Life**: Acknowledgements suppress redundant heartbeats
3. **Spectrum Efficiency**: Support short-hash acks for constrained adapters
4. **Multi-Hop Verification**: Relay confirmations verify intermediate nodes

---

## Message Flow

### Simple Acknowledgement (Single Adapter Available)

```
Node A ──[Message via Ethernet]──► Node B
Node B ──[Ack via Ethernet]──────► Node A

Result: Ethernet confirmed functional
```

### Round-Robin Acknowledgement (Multiple Adapters)

```
Scenario: Both nodes have Ethernet + Bluetooth

Node A ──[Message via Ethernet]──► Node B
Node B ──[Ack via Bluetooth]─────► Node A

Result:
  ✓ Ethernet confirmed (message delivery)
  ✓ Bluetooth confirmed (ack delivery)
  ✓ Both directions tested
  ✓ No separate heartbeat needed for 120+ seconds
```

### Relay Confirmation (Multi-Hop)

```
Node A ──[Msg]──► Node B ──[Msg]──► Node C

Relay confirmations:
Node B ──[Relayed]──────► Node A  (confirms B received and forwarded)
Node C ──[Delivered]────► Node B  (confirms C received final)
Node C ──[Delivered]────► Node A  (optional: end-to-end confirmation)
```

---

## Message Types

### 1. Direct Acknowledgement

```rust
pub struct MessageAck {
    /// Message identifier (full or truncated hash)
    pub message_id: MessageId,

    /// Acknowledgement type
    pub ack_type: AckType,

    /// Timestamp
    pub timestamp: u64,

    /// Signature (optional for bandwidth-constrained adapters)
    pub signature: Option<Vec<u8>>,
}

pub enum MessageId {
    /// Full message ID (32 bytes BLAKE3 hash)
    Full([u8; 32]),

    /// Truncated hash for spectrum-constrained adapters
    Short([u8; 8]),
}

pub enum AckType {
    /// Message received but not yet processed
    Received = 0,

    /// Message relayed to next hop
    Relayed = 1,

    /// Message delivered to final recipient (account)
    Delivered = 2,

    /// Message stored for later delivery
    Stored = 3,

    /// Message delivery failed
    Failed = 4,
}
```

### 2. Batch Acknowledgement

For efficiency when multiple messages are received:

```rust
pub struct BatchAck {
    /// List of message hashes
    pub message_ids: Vec<[u8; 8]>,  // Truncated hashes

    /// All have same ack type
    pub ack_type: AckType,

    /// Timestamp
    pub timestamp: u64,

    /// Single signature covers all
    pub signature: Vec<u8>,
}
```

---

## Adapter Selection Algorithm

### For Sending Acknowledgement

```rust
impl AckSender {
    async fn select_ack_adapter(
        &self,
        recipient: &NodeId,
        message_received_on: &str,  // Adapter ID that received message
    ) -> Result<String> {
        // Get all ready adapters to this peer
        let available = self.adapter_manager
            .get_ready_adapters_to_peer(recipient)
            .await?;

        // Prefer adapters that are:
        //  1. Different from receiving adapter (round-robin)
        //  2. Haven't been used recently (health check)
        //  3. Low-latency (for quick confirmation)

        // Filter out the receiving adapter
        let mut candidates: Vec<_> = available.into_iter()
            .filter(|a| a.id() != message_received_on)
            .collect();

        if candidates.is_empty() {
            // No alternative - use same adapter
            return Ok(message_received_on.to_string());
        }

        // Score adapters
        let mut scores: Vec<(String, f64)> = candidates.iter()
            .map(|adapter| {
                let mut score = 0.0;

                // Prefer adapters not used recently
                if let Some(last_used) = self.get_last_used(recipient, adapter.id()) {
                    let age = last_used.elapsed().as_secs() as f64;
                    score += age / 3600.0;  // Hours since last use
                }

                // Prefer low-latency adapters for quick ack
                let latency_score = 1000.0 / (adapter.latency_ms() + 1.0);
                score += latency_score * 0.1;

                (adapter.id().to_string(), score)
            })
            .collect();

        // Sort by score (highest first)
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        Ok(scores[0].0.clone())
    }
}
```

---

## Short Hash for Constrained Adapters

### Problem

- **LoRaWAN SF12**: 11-51 bytes payload
- **Iridium SBD**: 340 bytes (expensive per byte)
- **Full Message ID**: 32 bytes

### Solution: Truncated Hash

```rust
pub fn truncate_message_id(full_hash: &[u8; 32]) -> [u8; 8] {
    let mut short = [0u8; 8];
    short.copy_from_slice(&full_hash[0..8]);
    short
}

pub fn verify_short_hash(full_hash: &[u8; 32], short_hash: &[u8; 8]) -> bool {
    &full_hash[0..8] == short_hash
}
```

### Collision Analysis

**Probability of collision:**
- 8-byte hash space: 2^64 = 18,446,744,073,709,551,616 possible values
- For 1 million messages: P(collision) ≈ 1 / 18 trillion (negligible)

**Mitigation:**
- Short hashes only valid within time window (e.g., 5 minutes)
- After timeout, full hash required
- Collisions handled by timestamp disambiguation

---

## Integration with Heartbeat Suppression

### MessageTracker

```rust
pub struct MessageTracker {
    /// Last confirmed activity per peer
    last_activity: HashMap<NodeId, LastActivity>,

    /// Recent message IDs (for deduplication)
    recent_messages: LruCache<[u8; 32], Instant>,
}

pub struct LastActivity {
    /// When last activity occurred
    timestamp: Instant,

    /// What type of activity
    activity_type: ActivityType,

    /// Which adapter was used
    adapter_id: String,
}

pub enum ActivityType {
    MessageSent,
    MessageReceived,
    AckSent,
    AckReceived,
    Relayed,
}

impl MessageTracker {
    pub fn record_activity(
        &mut self,
        peer: &NodeId,
        activity_type: ActivityType,
        adapter_id: String,
    ) {
        let activity = LastActivity {
            timestamp: Instant::now(),
            activity_type,
            adapter_id,
        };

        self.last_activity.insert(peer.clone(), activity);

        // Notify heartbeat service to suppress next heartbeat
        self.notify_heartbeat_suppression(peer);
    }

    pub fn should_suppress_heartbeat(&self, peer: &NodeId, threshold: Duration) -> bool {
        if let Some(activity) = self.last_activity.get(peer) {
            activity.timestamp.elapsed() < threshold
        } else {
            false
        }
    }
}
```

### Integration Flow

```
1. Node B receives message from Node A
   └─► MessageTracker.record_activity(A, MessageReceived, "ethernet")

2. Node B sends ack via different adapter
   └─► MessageTracker.record_activity(A, AckSent, "bluetooth")

3. HeartbeatService checks if heartbeat needed
   └─► MessageTracker.should_suppress_heartbeat(A, 120s)
   └─► Returns TRUE (suppress heartbeat - recent activity)

4. Next heartbeat scheduled for: now + interval + suppression_time
```

---

## Reliability and Retries

### Acknowledgement Timeout

```rust
pub struct AckTimeout {
    /// Expected ack timeout per adapter type
    timeouts: HashMap<String, Duration>,

    /// Default timeout
    default: Duration,
}

impl AckTimeout {
    pub fn get_timeout(&self, adapter_type: &str) -> Duration {
        self.timeouts.get(adapter_type)
            .copied()
            .unwrap_or(self.default)
    }
}

// Configuration
[acknowledgement.timeouts]
ethernet = 5        # 5 seconds
bluetooth = 10      # 10 seconds
bluetooth_le = 15   # 15 seconds
lorawan = 60        # 60 seconds (long propagation)
i2p = 30            # 30 seconds
satellite = 120     # 2 minutes
dun = 180           # 3 minutes
```

### Retry Strategy

```rust
pub struct MessageRetry {
    /// Message awaiting ack
    message_id: [u8; 32],

    /// Attempt count
    attempts: u32,

    /// Max attempts
    max_attempts: u32,

    /// Last sent time
    last_sent: Instant,

    /// Adapter used for last attempt
    last_adapter: String,
}

impl MessageSender {
    async fn handle_ack_timeout(&mut self, message_id: &[u8; 32]) -> Result<()> {
        if let Some(retry) = self.pending_acks.get_mut(message_id) {
            retry.attempts += 1;

            if retry.attempts >= retry.max_attempts {
                warn!("Message {} failed after {} attempts", hex::encode(message_id), retry.attempts);
                self.mark_failed(message_id);
                return Ok(());
            }

            // Try different adapter on retry
            let next_adapter = self.select_retry_adapter(&retry.last_adapter).await?;

            info!("Retrying message {} on adapter {} (attempt {}/{})",
                hex::encode(message_id), next_adapter, retry.attempts + 1, retry.max_attempts);

            self.resend_message(message_id, &next_adapter).await?;

            retry.last_sent = Instant::now();
            retry.last_adapter = next_adapter;
        }

        Ok(())
    }
}
```

---

## Spectrum-Constrained Adapter Handling

### Minimal Acknowledgement Format

For LoRaWAN SF12 (11-51 bytes):

```rust
pub struct MinimalAck {
    /// Truncated message ID
    message_id: [u8; 8],

    /// Ack type (1 byte)
    ack_type: u8,

    /// Optional: Node ID hash (4 bytes)
    sender_hash: Option<[u8; 4]>,
}

// Size: 9 bytes (without sender) or 13 bytes (with sender)
// Leaves room for LoRaWAN headers/metadata
```

### Adapter-Specific Ack Strategy

```rust
impl AckSender {
    async fn send_ack_via_adapter(
        &self,
        adapter_id: &str,
        recipient: &NodeId,
        message_id: &[u8; 32],
        ack_type: AckType,
    ) -> Result<()> {
        let adapter = self.adapter_manager.get_adapter(adapter_id)?;

        // Check adapter capabilities
        match adapter.max_payload_size() {
            size if size >= 128 => {
                // Full ack with signature
                let ack = MessageAck::full(message_id, ack_type, self.sign_ack(message_id)?);
                adapter.send_ack(recipient, ack).await
            }
            size if size >= 32 => {
                // Short hash without signature
                let ack = MessageAck::short(truncate_message_id(message_id), ack_type);
                adapter.send_ack(recipient, ack).await
            }
            size if size >= 9 => {
                // Minimal ack
                let ack = MinimalAck {
                    message_id: truncate_message_id(message_id),
                    ack_type: ack_type as u8,
                    sender_hash: None,
                };
                adapter.send_minimal_ack(recipient, ack).await
            }
            _ => {
                // Too constrained - rely on message-level retries
                warn!("Adapter {} too constrained for acks (max {} bytes)", adapter_id, size);
                Ok(())
            }
        }
    }
}
```

---

## Multi-Hop Relay Confirmation

### Relay Acknowledgement Flow

```
Message path: A → B → C → D (final recipient)

1. A sends message to B
2. B receives, sends "Received" ack to A
3. B forwards to C
4. B sends "Relayed to C" ack to A
5. C receives, sends "Received" ack to B
6. C forwards to D
7. C sends "Relayed to D" ack to B
8. D receives (final), sends "Delivered" ack to C
9. D optionally sends "Delivered" ack to A (end-to-end confirmation)
```

### Implementation

```rust
pub struct RelayAck {
    /// Original message ID
    message_id: [u8; 32],

    /// Relay hop information
    relay_info: RelayInfo,

    /// Signature
    signature: Vec<u8>,
}

pub struct RelayInfo {
    /// Node that relayed
    relay_node: NodeId,

    /// Next hop (or final recipient)
    next_hop: NodeId,

    /// Hop count
    hop_number: u8,

    /// Total expected hops (if known)
    total_hops: Option<u8>,
}

impl RelayHandler {
    async fn handle_relay(&mut self, message: Message, next_hop: NodeId) -> Result<()> {
        // Forward message
        self.send_to_next_hop(&message, &next_hop).await?;

        // Send relay confirmation back to sender
        let relay_ack = RelayAck {
            message_id: message.id(),
            relay_info: RelayInfo {
                relay_node: self.node_id.clone(),
                next_hop,
                hop_number: message.hop_count + 1,
                total_hops: None,
            },
            signature: self.sign_relay_ack(&message.id(), &next_hop)?,
        };

        // Send ack back to previous hop (or originator)
        self.send_ack(&message.sender, relay_ack).await?;

        // Record activity (suppresses heartbeat)
        self.message_tracker.record_activity(
            &message.sender,
            ActivityType::Relayed,
            "various",  // May have used different adapters
        );

        Ok(())
    }
}
```

---

## Configuration

```toml
[acknowledgement]
enabled = true
require_acks = true

# Timeouts per adapter type
[acknowledgement.timeouts]
ethernet = 5
bluetooth = 10
bluetooth_le = 15
lorawan = 60
i2p = 30
satellite = 120
dun = 180

# Retry configuration
[acknowledgement.retry]
max_attempts = 3
retry_different_adapter = true  # Use different adapter on retry
backoff_multiplier = 1.5        # Exponential backoff

# Short hash configuration
[acknowledgement.short_hash]
enabled = true
max_age_secs = 300  # Short hashes only valid for 5 minutes
min_adapter_payload = 32  # Use short hash if adapter payload < 32 bytes

# Relay confirmation
[acknowledgement.relay]
send_relay_acks = true
send_end_to_end_ack = false  # Optional: final recipient acks to originator

# Integration with heartbeat suppression
[acknowledgement.heartbeat_suppression]
enabled = true
suppress_timeout_secs = 120  # Don't send heartbeat if ack within 2 minutes
```

---

## Performance Considerations

### Batching

When multiple messages are received in quick succession:

```rust
impl AckBatcher {
    async fn maybe_batch_acks(&mut self) -> Option<BatchAck> {
        // Wait up to 100ms to collect multiple acks
        tokio::time::sleep(Duration::from_millis(100)).await;

        if self.pending_acks.len() >= 2 {
            // Batch multiple acks into one message
            let batch = BatchAck {
                message_ids: self.pending_acks.drain(..).collect(),
                ack_type: AckType::Received,
                timestamp: current_timestamp(),
                signature: self.sign_batch(&self.pending_acks)?,
            };

            Some(batch)
        } else {
            None
        }
    }
}
```

### Memory Management

```rust
// Limit pending acks to prevent memory exhaustion
const MAX_PENDING_ACKS: usize = 10_000;

// Expire old pending acks
async fn cleanup_old_acks(&mut self) {
    let now = Instant::now();
    self.pending_acks.retain(|_, retry| {
        now.duration_since(retry.last_sent) < Duration::from_secs(300)
    });
}
```

---

## Testing

### Unit Tests

```rust
#[tokio::test]
async fn test_round_robin_ack_selection() {
    let tracker = MessageTracker::new();
    let ack_sender = AckSender::new(tracker);

    // Receive message on ethernet
    let adapter = ack_sender.select_ack_adapter(&peer, "ethernet").await.unwrap();

    // Should select different adapter (bluetooth)
    assert_eq!(adapter, "bluetooth");
}

#[tokio::test]
async fn test_short_hash_collision() {
    let hash1 = [0u8; 32];
    let hash2 = [0u8; 32];
    hash2[16] = 1;  // Different in second half

    let short1 = truncate_message_id(&hash1);
    let short2 = truncate_message_id(&hash2);

    // Should be same (collision)
    assert_eq!(short1, short2);

    // But full hashes different
    assert_ne!(hash1, hash2);
}

#[tokio::test]
async fn test_heartbeat_suppression_after_ack() {
    let mut tracker = MessageTracker::new();

    // Record ack activity
    tracker.record_activity(&peer, ActivityType::AckReceived, "ethernet");

    // Should suppress heartbeat
    assert!(tracker.should_suppress_heartbeat(&peer, Duration::from_secs(120)));

    // Wait past threshold
    tokio::time::sleep(Duration::from_secs(121)).await;

    // Should not suppress
    assert!(!tracker.should_suppress_heartbeat(&peer, Duration::from_secs(120)));
}
```

---

## Related Documents

- [Heartbeat Protocol](./heartbeat-protocol.md)
- [Account and Identity Model](./account-identity-model.md)
- [Adapter Privacy Architecture](./adapter-privacy-architecture.md)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-01-13 | Claude | Initial design |
