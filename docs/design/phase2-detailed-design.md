# Phase 2: Core Protocol - Detailed Design Document

**Version:** 1.0
**Date:** 2025-11-11
**Status:** Review

## Executive Summary

This document provides the detailed design for Phase 2 of MyriadMesh, implementing the core protocol components: DHT, message routing, network abstraction, and the first network adapter (Ethernet/UDP).

**Key Design Decisions:**
1. ✅ **No Proof-of-Work** for node IDs (optimized for embedded devices)
2. ✅ **Reputation-based** Sybil resistance
3. ✅ **E2E encryption** with optional content tagging for relay filtering
4. ✅ **Availability-first** with security-first principles for designated sensitive traffic
5. ✅ **Strict resource limits** to prevent DoS attacks

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Security Model](#security-model)
3. [DHT Implementation](#dht-implementation)
4. [Message Router](#message-router)
5. [Network Abstraction Layer](#network-abstraction-layer)
6. [Ethernet Adapter](#ethernet-adapter)
7. [Resource Limits](#resource-limits)
8. [Testing Strategy](#testing-strategy)
9. [Migration Path](#migration-path)
10. [Open Questions](#open-questions)

---

## Architecture Overview

### Component Diagram

```
┌─────────────────────────────────────────────────────────┐
│                   Application Layer                      │
│         (Will be implemented in Phase 3)                 │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│              Message Router (NEW)                        │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Priority Queue Manager                          │   │
│  │  - Emergency (224-255)                           │   │
│  │  - High (192-223)                                │   │
│  │  - Normal (128-191)                              │   │
│  │  - Low (64-127)                                  │   │
│  │  - Background (0-63)                             │   │
│  └──────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Routing Engine                                  │   │
│  │  - Direct routing                                │   │
│  │  - Multi-hop routing                             │   │
│  │  - Store-and-forward                             │   │
│  └──────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Message Deduplication Cache                     │   │
│  │  - LRU cache (10,000 entries, 1 hour TTL)       │   │
│  └──────────────────────────────────────────────────┘   │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│              DHT Manager (NEW)                           │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Kademlia Routing Table                          │   │
│  │  - 256 k-buckets (k=20)                          │   │
│  │  - Node info, reputation, last_seen              │   │
│  └──────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────┐   │
│  │  DHT Storage                                     │   │
│  │  - Node records                                  │   │
│  │  - Route records (performance metrics)           │   │
│  │  - Cached messages                               │   │
│  │  - Max 100MB per node                            │   │
│  └──────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Reputation System                               │   │
│  │  - Successful relay tracking                     │   │
│  │  - Failed relay tracking                         │   │
│  │  - Uptime tracking                               │   │
│  └──────────────────────────────────────────────────┘   │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│         Network Abstraction Layer (NEW)                  │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Adapter Manager                                 │   │
│  │  - Adapter registration                          │   │
│  │  - Lifecycle management                          │   │
│  │  - Health monitoring                             │   │
│  └──────────────────────────────────────────────────┘   │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│           Ethernet Adapter (NEW)                         │
│  - UDP transport on port 4001                            │
│  - Multicast discovery (239.255.77.77)                   │
│  - IPv4/IPv6 support                                     │
│  - MTU 1400 (safe for most networks)                     │
└──────────────────────────────────────────────────────────┘
```

### Data Flow

**Outbound Message:**
```
Application
    ↓
Message Router
    ↓ (consult DHT for destination)
    ↓
DHT Manager → Returns node info & best route
    ↓
Message Router → Selects network adapter
    ↓
Network Abstraction Layer
    ↓
Ethernet Adapter → Sends via UDP
```

**Inbound Message:**
```
Ethernet Adapter → Receives UDP packet
    ↓
Network Abstraction Layer
    ↓
Message Router → Verify signature, decrypt
    ↓ (check if for us or relay)
    ↓
[If for us] → Deliver to application
[If relay] → Route to next hop
    ↓
DHT Manager → Update routing metrics
```

---

## Security Model

### Threat Model (Phase 2 Scope)

**In Scope:**
- ✅ Sybil attacks (multiple fake node identities)
- ✅ Message replay attacks
- ✅ Message tampering
- ✅ DoS via message flooding
- ✅ DoS via storage exhaustion
- ✅ Malicious relay nodes dropping messages
- ✅ Passive eavesdropping
- ✅ Man-in-the-middle attacks

**Out of Scope (Later Phases):**
- ⏸️ Advanced traffic analysis
- ⏸️ Timing attacks
- ⏸️ Eclipse attacks (mitigated in Phase 4)
- ⏸️ Quantum computing threats (post-quantum crypto in Phase 6)

### E2E Encryption with Optional Content Tags

**Core Principle**: *All message payloads are E2E encrypted. Relay nodes CANNOT read content. Content tags are optional metadata for routing decisions.*

#### Message Structure (Updated from Phase 1)

```rust
pub struct MessageFrame {
    // === CLEARTEXT HEADER (can be read by relays) ===
    pub magic: [u8; 4],              // "MYMS"
    pub version: u8,                 // 0x01
    pub flags: MessageFlags,         // Updated with new flags
    pub message_type: MessageType,
    pub priority: u8,                // 0-255
    pub ttl: u8,                     // Hop count
    pub payload_length: u16,
    pub message_id: [u8; 16],
    pub source_node_id: [u8; 32],
    pub dest_node_id: [u8; 32],
    pub timestamp: u64,

    // === NEW: Routing Metadata (optional, cleartext) ===
    pub routing_flags: RoutingFlags,   // NEW
    pub content_tags: Vec<String>,     // NEW (optional, max 10 tags, 32 bytes each)

    // === E2E ENCRYPTED PAYLOAD ===
    pub payload: Vec<u8>,             // Encrypted with XSalsa20-Poly1305

    // === SIGNATURE ===
    pub signature: [u8; 64],          // Ed25519 signature of entire frame
}
```

#### New Routing Flags

```rust
bitflags! {
    pub struct RoutingFlags: u8 {
        /// Message is strictly E2E encrypted (default)
        const E2E_STRICT = 0b0000_0001;

        /// User-designated sensitive content (relays MUST forward)
        const SENSITIVE = 0b0000_0010;

        /// Relays MAY use content tags for filtering
        const RELAY_FILTERABLE = 0b0000_0100;

        /// Request multi-path routing (future)
        const MULTI_PATH = 0b0000_1000;

        /// Message is anonymous (route via i2p) (Phase 4)
        const ANONYMOUS = 0b0001_0000;
    }
}
```

#### Content Tag System

**Tag Format**: `category:value` or `flag`

**Standard Tags** (extensible):
```rust
pub mod standard_tags {
    /// Content classification
    pub const NSFW: &str = "nsfw";
    pub const POLITICAL: &str = "political";
    pub const COMMERCIAL: &str = "commercial";
    pub const EDUCATIONAL: &str = "educational";

    /// Media types
    pub const MEDIA_IMAGE: &str = "media:image";
    pub const MEDIA_VIDEO: &str = "media:video";
    pub const MEDIA_AUDIO: &str = "media:audio";
    pub const MEDIA_DOCUMENT: &str = "media:document";

    /// Size hints
    pub const SIZE_SMALL: &str = "size:small";    // <10KB
    pub const SIZE_MEDIUM: &str = "size:medium";  // 10KB-1MB
    pub const SIZE_LARGE: &str = "size:large";    // >1MB

    /// User-defined priority hints
    pub const PRIORITY_EMERGENCY: &str = "priority:emergency";
    pub const PRIORITY_HIGH: &str = "priority:high";
    pub const PRIORITY_NORMAL: &str = "priority:normal";
}
```

**Tag Rules**:
- Maximum 10 tags per message
- Each tag max 32 bytes
- Tags are **optional** and **untrusted** (sender can lie)
- Relays use tags as **hints only**, not security boundaries
- Sensitive content should use NO tags (default E2E_STRICT)

#### Relay Filtering Policy

**Relay Node Configuration**:
```yaml
relay_policy:
  # Enable relay filtering based on tags
  enable_filtering: false  # Default: false (relay everything)

  # Blocked tags (relay will refuse these)
  blocked_tags:
    - "nsfw"
    - "commercial"

  # Allowed tags (if set, only relay these)
  allowed_tags: []  # Empty = allow all

  # Always relay sensitive messages (ignore filtering)
  always_relay_sensitive: true  # Recommended

  # Size limits
  max_message_size: 1048576  # 1MB
  max_relay_rate: 1000       # messages/minute
```

**Relay Decision Logic**:
```rust
fn should_relay(msg: &MessageFrame, policy: &RelayPolicy) -> bool {
    // MUST relay if SENSITIVE flag set (availability guarantee)
    if msg.routing_flags.contains(RoutingFlags::SENSITIVE) {
        return true;
    }

    // If filtering disabled, relay everything
    if !policy.enable_filtering {
        return true;
    }

    // If not RELAY_FILTERABLE, relay (no tags to filter on)
    if !msg.routing_flags.contains(RoutingFlags::RELAY_FILTERABLE) {
        return true;
    }

    // Check blocked tags
    for tag in &msg.content_tags {
        if policy.blocked_tags.contains(tag) {
            return false;  // Refuse relay
        }
    }

    // Check allowed tags (if specified)
    if !policy.allowed_tags.is_empty() {
        let has_allowed = msg.content_tags.iter()
            .any(|tag| policy.allowed_tags.contains(tag));
        if !has_allowed {
            return false;
        }
    }

    true  // Relay
}
```

**Security Properties**:
- ✅ Relays CANNOT read message content (always E2E encrypted)
- ✅ Relays CANNOT verify tag accuracy (sender controls tags)
- ✅ Sensitive messages MUST be relayed (availability guarantee)
- ✅ Default behavior: relay everything (availability-first)
- ✅ Filtering is opt-in for both sender and relay
- ✅ Perfect forward secrecy maintained (X25519 ephemeral keys)

#### Example Use Cases

**Use Case 1: Private Conversation (Default)**
```rust
let msg = MessageFrame {
    routing_flags: RoutingFlags::E2E_STRICT,  // Default
    content_tags: vec![],  // No tags
    payload: encrypt(conversation_data),
    // ...
};
// Result: Relays blindly forward, cannot read or filter
```

**Use Case 2: User Marks as Sensitive**
```rust
let msg = MessageFrame {
    routing_flags: RoutingFlags::SENSITIVE,  // User designated
    content_tags: vec![],  // No tags (privacy)
    payload: encrypt(sensitive_data),
    // ...
};
// Result: ALL relays MUST forward (even if they filter other traffic)
```

**Use Case 3: Large Media File (User Opts-in to Tagging)**
```rust
let msg = MessageFrame {
    routing_flags: RoutingFlags::RELAY_FILTERABLE,
    content_tags: vec!["media:video".to_string(), "size:large".to_string()],
    payload: encrypt(video_data),
    // ...
};
// Result: Relays can choose to refuse based on tags (availability trade-off)
```

**Use Case 4: Automatic Sensitive Content Detection**
```rust
// Application layer can auto-detect and set flags
if contains_private_keys(data) || is_personal_health_info(data) {
    msg.routing_flags = RoutingFlags::SENSITIVE;
    msg.content_tags = vec![];  // No tags for privacy
} else {
    msg.routing_flags = RoutingFlags::E2E_STRICT;
    msg.content_tags = infer_tags(data);  // Optional
}
```

---

## DHT Implementation

### Design Decision: No Proof-of-Work

**Rationale**:
1. **Embedded Device Support**: PoW would significantly delay node onboarding on resource-constrained devices (Raspberry Pi Zero, IoT devices)
2. **Limited Benefit**: PoW only applies to node ID generation, not message-level security
3. **Alternative Approach**: Reputation-based Sybil resistance is more effective for this protocol
4. **Future Option**: Can add "verified node" status with optional PoW in Phase 4 if needed

**Chosen Approach**: Reputation-based Sybil resistance + rate limiting

### Kademlia DHT Specification

#### Routing Table

```rust
pub struct RoutingTable {
    /// Our node ID
    local_node_id: NodeId,

    /// 256 k-buckets (one per bit of node ID)
    buckets: Vec<KBucket>,

    /// Replacement cache for full buckets
    replacement_cache: HashMap<usize, Vec<NodeInfo>>,
}

pub struct KBucket {
    /// Up to k nodes (k=20)
    nodes: Vec<NodeInfo>,

    /// Last time this bucket was updated
    last_updated: Timestamp,

    /// Bucket index (0-255)
    index: usize,
}

pub struct NodeInfo {
    /// Node identifier (32 bytes)
    pub node_id: NodeId,

    /// Ed25519 public key
    pub public_key: PublicKey,

    /// Available network adapters
    pub adapters: Vec<AdapterInfo>,

    /// Last successful communication
    pub last_seen: Timestamp,

    /// Round-trip time (ms)
    pub rtt_ms: f64,

    /// Consecutive failures
    pub failures: u32,

    /// === NEW: Reputation System ===
    pub reputation: NodeReputation,
}

pub struct NodeReputation {
    /// Successful message relays
    pub successful_relays: u64,

    /// Failed relay attempts
    pub failed_relays: u64,

    /// Total uptime (seconds)
    pub uptime_seconds: u64,

    /// First seen timestamp
    pub first_seen: Timestamp,

    /// Reputation score (0.0 - 1.0)
    pub score: f64,
}
```

#### Reputation Score Calculation

```rust
impl NodeReputation {
    /// Calculate reputation score (0.0 - 1.0)
    pub fn calculate_score(&self) -> f64 {
        // Relay reliability (50% weight)
        let total_relays = self.successful_relays + self.failed_relays;
        let reliability = if total_relays > 0 {
            self.successful_relays as f64 / total_relays as f64
        } else {
            0.5  // Neutral for new nodes
        };

        // Uptime score (30% weight)
        // Max out at 90 days
        let uptime_score = (self.uptime_seconds as f64 / (90.0 * 86400.0)).min(1.0);

        // Age score (20% weight)
        // Older nodes (more history) are slightly more trusted
        let age_seconds = now() - self.first_seen;
        let age_score = (age_seconds as f64 / (30.0 * 86400.0)).min(1.0);

        // Weighted average
        reliability * 0.5 + uptime_score * 0.3 + age_score * 0.2
    }

    /// Minimum reputation to be considered trustworthy
    pub const MIN_REPUTATION: f64 = 0.3;

    /// Reputation for relay selection
    pub const GOOD_REPUTATION: f64 = 0.7;
}
```

#### DHT Operations

**FIND_NODE**:
```rust
pub struct FindNodeRequest {
    pub target: NodeId,
    pub query_id: QueryId,
}

pub struct FindNodeResponse {
    pub query_id: QueryId,
    pub nodes: Vec<NodeInfo>,  // Up to k closest nodes
}

impl DhtManager {
    /// Iterative lookup to find k closest nodes
    pub async fn lookup_node(&self, target: NodeId) -> Result<Vec<NodeInfo>> {
        let mut closest = self.routing_table.get_k_closest(target, K);
        let mut queried = HashSet::new();
        let mut pending = Vec::new();

        // Start with alpha parallel queries (alpha=3)
        const ALPHA: usize = 3;
        for node in closest.iter().take(ALPHA) {
            pending.push(self.query_find_node(node, target));
            queried.insert(node.node_id);
        }

        // Iteratively query closer nodes
        while !pending.is_empty() {
            // Wait for any response
            let response = select_any(&mut pending).await?;

            // Add new nodes
            for node in response.nodes {
                if !queried.contains(&node.node_id) {
                    closest.push(node);
                    closest.sort_by_key(|n| xor_distance(n.node_id, target));
                    closest.truncate(K);

                    // Query if promising
                    if pending.len() < ALPHA {
                        pending.push(self.query_find_node(&node, target));
                        queried.insert(node.node_id);
                    }
                }
            }
        }

        Ok(closest)
    }
}
```

**STORE**:
```rust
pub struct StoreRequest {
    pub key: [u8; 32],
    pub value: Vec<u8>,
    pub ttl: u32,  // seconds
    pub signature: Signature,
}

impl DhtManager {
    /// Store value at k closest nodes
    pub async fn store(&self, key: [u8; 32], value: Vec<u8>, ttl: u32) -> Result<()> {
        // Find k closest nodes
        let closest = self.lookup_node(key).await?;

        // Sign the data
        let signature = self.identity.sign(&[&key[..], &value].concat());

        // Send STORE to each node
        let mut tasks = Vec::new();
        for node in closest {
            tasks.push(self.send_store(&node, key, value.clone(), ttl, signature));
        }

        // Wait for majority to succeed
        let results = join_all(tasks).await;
        let success_count = results.iter().filter(|r| r.is_ok()).count();

        if success_count >= (K / 2) {
            Ok(())
        } else {
            Err(DhtError::StoreFailed)
        }
    }

    /// Handle incoming STORE request
    pub fn handle_store(&mut self, req: StoreRequest) -> Result<()> {
        // Check if we're responsible for this key
        if !self.is_responsible_for_key(req.key) {
            return Err(DhtError::NotResponsible);
        }

        // Verify signature
        let node_id = extract_node_id_from_value(&req.value)?;
        let public_key = self.get_public_key(node_id)?;
        if !verify_signature(&public_key, &[&req.key[..], &req.value].concat(), &req.signature) {
            return Err(DhtError::InvalidSignature);
        }

        // Check storage limits
        if self.storage.size() >= MAX_DHT_STORAGE_BYTES {
            return Err(DhtError::StorageFull);
        }

        // Store with expiration
        let expiry = now() + req.ttl;
        self.storage.insert(req.key, req.value, expiry)?;

        Ok(())
    }
}
```

**FIND_VALUE**:
```rust
pub struct FindValueRequest {
    pub key: [u8; 32],
    pub query_id: QueryId,
}

pub enum FindValueResponse {
    Found {
        query_id: QueryId,
        value: Vec<u8>,
        signature: Signature,
    },
    NotFound {
        query_id: QueryId,
        nodes: Vec<NodeInfo>,  // Closer nodes to try
    },
}

impl DhtManager {
    /// Find value in DHT (iterative)
    pub async fn find_value(&self, key: [u8; 32]) -> Result<Option<Vec<u8>>> {
        let mut closest = self.routing_table.get_k_closest(key, K);
        let mut queried = HashSet::new();
        let mut pending = Vec::new();

        const ALPHA: usize = 3;
        for node in closest.iter().take(ALPHA) {
            pending.push(self.query_find_value(node, key));
            queried.insert(node.node_id);
        }

        while !pending.is_empty() {
            let response = select_any(&mut pending).await?;

            match response {
                FindValueResponse::Found { value, signature, .. } => {
                    // Verify signature
                    if self.verify_dht_value(&key, &value, &signature)? {
                        return Ok(Some(value));
                    }
                }
                FindValueResponse::NotFound { nodes, .. } => {
                    // Continue searching
                    for node in nodes {
                        if !queried.contains(&node.node_id) {
                            closest.push(node);
                            closest.sort_by_key(|n| xor_distance(n.node_id, key));

                            if pending.len() < ALPHA {
                                pending.push(self.query_find_value(&node, key));
                                queried.insert(node.node_id);
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }
}
```

#### DHT Storage Types

**Node Records**:
```rust
pub struct NodeRecord {
    pub node_id: NodeId,
    pub public_key: PublicKey,
    pub adapters: Vec<AdapterInfo>,
    pub capabilities: NodeCapabilities,
    pub location: Option<GeoLocation>,
    pub timestamp: Timestamp,
    pub signature: Signature,
}

// Stored at key: BLAKE2b("node:" + node_id)
```

**Route Records**:
```rust
pub struct RouteRecord {
    pub source_node: NodeId,
    pub dest_node: NodeId,
    pub adapter: AdapterType,
    pub metrics: RouteMetrics,
    pub timestamp: Timestamp,
    pub signature: Signature,
}

pub struct RouteMetrics {
    pub latency_ms: f64,
    pub bandwidth_bps: u64,
    pub reliability: f64,  // 0.0 - 1.0
    pub last_test: Timestamp,
    pub sample_count: u32,
}

// Stored at key: BLAKE2b("route:" + source_node + dest_node + adapter)
```

**Cached Messages** (Store-and-Forward):
```rust
pub struct CachedMessage {
    pub message_id: [u8; 16],
    pub dest_node: NodeId,
    pub encrypted_payload: Vec<u8>,
    pub priority: u8,
    pub expires_at: Timestamp,
    pub cache_node: NodeId,
}

// Stored at key: BLAKE2b("cache:" + dest_node + message_id)
```

#### DHT Maintenance

```rust
impl DhtManager {
    /// Periodic maintenance tasks
    pub async fn maintenance_loop(&self) {
        loop {
            // Refresh buckets with no recent activity (every hour)
            self.refresh_stale_buckets().await;

            // Health check nodes in routing table (every 5 minutes)
            self.health_check_nodes().await;

            // Republish stored values (every hour)
            self.republish_values().await;

            // Clean up expired values
            self.cleanup_expired_values().await;

            // Update reputation scores
            self.update_reputation_scores().await;

            sleep(Duration::from_secs(60)).await;
        }
    }

    /// Remove nodes with poor reputation
    async fn prune_bad_reputation_nodes(&mut self) {
        for bucket in &mut self.routing_table.buckets {
            bucket.nodes.retain(|node| {
                node.reputation.score >= NodeReputation::MIN_REPUTATION
            });
        }
    }
}
```

---

## Message Router

### Priority Queue System

```rust
pub struct MessageRouter {
    /// Priority queues (5 levels)
    queues: [VecDeque<QueuedMessage>; 5],

    /// Message deduplication cache
    seen_messages: LruCache<MessageId, Timestamp>,

    /// Store-and-forward cache
    cached_messages: HashMap<NodeId, Vec<CachedMessage>>,

    /// DHT manager reference
    dht: Arc<DhtManager>,

    /// Network adapter manager reference
    adapters: Arc<AdapterManager>,

    /// Routing statistics
    stats: RoutingStats,
}

#[derive(Clone)]
pub struct QueuedMessage {
    pub frame: MessageFrame,
    pub received_at: Timestamp,
    pub retry_count: u32,
}

impl MessageRouter {
    /// Route outbound message
    pub async fn send_message(&mut self, mut frame: MessageFrame) -> Result<()> {
        // Check TTL
        if frame.ttl == 0 {
            return Err(RoutingError::TtlExceeded);
        }

        // Sign message
        frame.signature = self.identity.sign(&frame.to_bytes());

        // Lookup destination in DHT
        let dest_record = self.dht.find_value(node_key(frame.dest_node_id)).await?;

        match dest_record {
            Some(record) => {
                // Destination known, route directly or via relay
                self.route_to_destination(frame, record).await
            }
            None => {
                // Destination unknown, cache for later delivery
                self.cache_message(frame).await
            }
        }
    }

    /// Handle incoming message
    pub async fn receive_message(&mut self, frame: MessageFrame, adapter: AdapterId) -> Result<()> {
        // Verify signature
        if !self.verify_signature(&frame)? {
            self.stats.invalid_signatures += 1;
            return Err(RoutingError::InvalidSignature);
        }

        // Check replay protection
        if self.seen_messages.contains(&frame.message_id) {
            self.stats.replays_detected += 1;
            return Err(RoutingError::ReplayDetected);
        }
        self.seen_messages.put(frame.message_id, now());

        // Check timestamp (±5 minutes)
        let now_ms = now_ms();
        if (now_ms as i64 - frame.timestamp as i64).abs() > 5 * 60 * 1000 {
            self.stats.invalid_timestamps += 1;
            return Err(RoutingError::InvalidTimestamp);
        }

        // Check if message is for us
        if frame.dest_node_id == self.dht.local_node_id() {
            // Deliver to application layer
            self.deliver_to_application(frame).await
        } else {
            // Relay to next hop
            self.relay_message(frame).await
        }
    }

    /// Relay message to next hop
    async fn relay_message(&mut self, mut frame: MessageFrame) -> Result<()> {
        // Check if we should relay (content filtering)
        if !self.should_relay(&frame) {
            self.stats.filtered_messages += 1;
            return Ok(());  // Silently drop
        }

        // Decrement TTL
        frame.ttl -= 1;
        if frame.ttl == 0 {
            self.stats.ttl_exceeded += 1;
            return Err(RoutingError::TtlExceeded);
        }

        // Set relay flag
        frame.flags |= MessageFlags::RELAY;

        // Find next hop
        let dest_record = self.dht.find_value(node_key(frame.dest_node_id)).await?;

        match dest_record {
            Some(record) => {
                // Route to destination or closer relay
                self.route_to_destination(frame, record).await?;

                // Update relay statistics (for reputation)
                self.dht.record_successful_relay(self.dht.local_node_id()).await;

                self.stats.messages_relayed += 1;
                Ok(())
            }
            None => {
                // Cache message for later
                self.cache_message(frame).await?;
                self.stats.messages_cached += 1;
                Ok(())
            }
        }
    }

    /// Check if message should be relayed (content filtering)
    fn should_relay(&self, frame: &MessageFrame) -> bool {
        // Always relay SENSITIVE messages
        if frame.routing_flags.contains(RoutingFlags::SENSITIVE) {
            return true;
        }

        // If filtering disabled, relay everything
        if !self.config.relay_policy.enable_filtering {
            return true;
        }

        // If no tags, relay (E2E_STRICT)
        if !frame.routing_flags.contains(RoutingFlags::RELAY_FILTERABLE) {
            return true;
        }

        // Check blocked tags
        for tag in &frame.content_tags {
            if self.config.relay_policy.blocked_tags.contains(tag) {
                return false;
            }
        }

        true
    }
}
```

### Store-and-Forward

```rust
impl MessageRouter {
    /// Cache message for offline destination
    async fn cache_message(&mut self, frame: MessageFrame) -> Result<()> {
        // Check cache limits per destination
        let cached_count = self.cached_messages
            .get(&frame.dest_node_id)
            .map(|v| v.len())
            .unwrap_or(0);

        if cached_count >= MAX_CACHED_MESSAGES_PER_DEST {
            return Err(RoutingError::CacheFull);
        }

        // Create cache record
        let cache_record = CachedMessage {
            message_id: frame.message_id,
            dest_node: frame.dest_node_id,
            encrypted_payload: frame.payload.clone(),
            priority: frame.priority,
            expires_at: now() + (24 * 3600),  // 24 hours
            cache_node: self.dht.local_node_id(),
        };

        // Store locally
        self.cached_messages
            .entry(frame.dest_node_id)
            .or_insert_with(Vec::new)
            .push(cache_record.clone());

        // Store in DHT for redundancy
        let cache_key = cache_key(frame.dest_node_id, frame.message_id);
        self.dht.store(cache_key, serialize(&cache_record)?, 24 * 3600).await?;

        Ok(())
    }

    /// Retrieve cached messages for a node
    pub async fn retrieve_cached_messages(&mut self, node_id: NodeId) -> Result<Vec<MessageFrame>> {
        let mut messages = Vec::new();

        // Check local cache first
        if let Some(cached) = self.cached_messages.get(&node_id) {
            for record in cached {
                if record.expires_at > now() {
                    // Reconstruct message frame
                    let frame = self.reconstruct_frame(record)?;
                    messages.push(frame);
                }
            }
        }

        // Query DHT for cached messages
        let cache_prefix = cache_prefix(node_id);
        let dht_cached = self.dht.find_values_by_prefix(cache_prefix).await?;

        for data in dht_cached {
            let record: CachedMessage = deserialize(&data)?;
            if record.expires_at > now() {
                let frame = self.reconstruct_frame(&record)?;
                messages.push(frame);
            }
        }

        // Clean up local cache
        self.cached_messages.remove(&node_id);

        Ok(messages)
    }
}
```

---

## Network Abstraction Layer

### Adapter Interface

```rust
#[async_trait]
pub trait NetworkAdapter: Send + Sync {
    /// Initialize adapter with configuration
    async fn initialize(&mut self, config: AdapterConfig) -> Result<()>;

    /// Start the adapter
    async fn start(&mut self) -> Result<()>;

    /// Stop the adapter
    async fn stop(&mut self) -> Result<()>;

    /// Send message frame to destination
    async fn send(&self, destination: Address, frame: &MessageFrame) -> Result<MessageId>;

    /// Receive next message (blocking until message arrives)
    async fn receive(&self) -> Result<(Address, MessageFrame)>;

    /// Discover peers on this network
    async fn discover_peers(&self) -> Result<Vec<PeerInfo>>;

    /// Get adapter status
    fn get_status(&self) -> AdapterStatus;

    /// Get adapter capabilities
    fn get_capabilities(&self) -> AdapterCapabilities;

    /// Test connection to destination
    async fn test_connection(&self, destination: Address) -> Result<TestResults>;

    /// Get local address for this adapter
    fn get_local_address(&self) -> Address;

    /// Parse address string
    fn parse_address(&self, addr_str: &str) -> Result<Address>;
}
```

### Adapter Manager

```rust
pub struct AdapterManager {
    /// Registered adapters
    adapters: HashMap<AdapterId, Box<dyn NetworkAdapter>>,

    /// Adapter performance metrics
    metrics: HashMap<AdapterId, AdapterMetrics>,

    /// Configuration
    config: AdapterManagerConfig,
}

impl AdapterManager {
    /// Register a new adapter
    pub async fn register_adapter(
        &mut self,
        id: AdapterId,
        adapter: Box<dyn NetworkAdapter>,
    ) -> Result<()> {
        adapter.initialize(self.config.adapter_config(id)?).await?;
        adapter.start().await?;
        self.adapters.insert(id, adapter);
        Ok(())
    }

    /// Select best adapter for destination
    pub async fn select_adapter(
        &self,
        destination: NodeId,
        message: &MessageFrame,
    ) -> Result<(AdapterId, Address)> {
        // Get destination node record from DHT
        let node_record = /* get from DHT */;

        // Find common adapters
        let mut candidates = Vec::new();
        for (id, adapter) in &self.adapters {
            if let Some(dest_addr) = self.find_common_adapter(&node_record, *id) {
                let score = self.calculate_adapter_score(*id, message, &node_record);
                candidates.push((score, *id, dest_addr));
            }
        }

        // Sort by score (highest first)
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        // Return best adapter
        candidates.first()
            .map(|(_, id, addr)| (*id, addr.clone()))
            .ok_or(AdapterError::NoCommonAdapter)
    }

    /// Calculate adapter score for message
    fn calculate_adapter_score(
        &self,
        adapter_id: AdapterId,
        message: &MessageFrame,
        node_record: &NodeRecord,
    ) -> f64 {
        let metrics = &self.metrics[&adapter_id];

        // Weight factors based on message priority
        match message.priority {
            224..=255 => {  // EMERGENCY
                metrics.reliability * 0.6 +
                metrics.availability * 0.3 +
                (1.0 - normalize(metrics.latency_ms)) * 0.1
            }
            192..=223 => {  // HIGH
                (1.0 - normalize(metrics.latency_ms)) * 0.4 +
                metrics.reliability * 0.3 +
                normalize(metrics.bandwidth_bps) * 0.2 +
                (1.0 - metrics.cost_per_mb) * 0.1
            }
            _ => {  // NORMAL, LOW, BACKGROUND
                (1.0 - normalize(metrics.latency_ms)) * 0.25 +
                metrics.reliability * 0.25 +
                normalize(metrics.bandwidth_bps) * 0.2 +
                (1.0 - metrics.cost_per_mb) * 0.2 +
                metrics.availability * 0.1
            }
        }
    }
}
```

---

## Ethernet Adapter

### Implementation

```rust
pub struct EthernetAdapter {
    /// UDP socket
    socket: Arc<UdpSocket>,

    /// Multicast socket for discovery
    multicast_socket: Arc<UdpSocket>,

    /// Configuration
    config: EthernetConfig,

    /// Discovered peers
    peers: Arc<RwLock<HashMap<NodeId, SocketAddr>>>,

    /// Running state
    running: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct EthernetConfig {
    pub interface: String,         // "eth0"
    pub bind_address: String,       // "0.0.0.0"
    pub port: u16,                  // 4001
    pub multicast_group: String,    // "239.255.77.77"
    pub multicast_port: u16,        // 4001
    pub mtu: usize,                 // 1400 (safe default)
    pub enable_discovery: bool,     // true
}

impl EthernetAdapter {
    pub async fn new(config: EthernetConfig) -> Result<Self> {
        // Create UDP socket
        let bind_addr = format!("{}:{}", config.bind_address, config.port);
        let socket = UdpSocket::bind(&bind_addr).await?;

        // Create multicast socket for discovery
        let multicast_addr = format!("{}:{}", config.multicast_group, config.multicast_port);
        let multicast_socket = UdpSocket::bind(&multicast_addr).await?;

        // Join multicast group
        multicast_socket.join_multicast_v4(
            config.multicast_group.parse()?,
            Ipv4Addr::UNSPECIFIED,
        )?;

        Ok(Self {
            socket: Arc::new(socket),
            multicast_socket: Arc::new(multicast_socket),
            config,
            peers: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
        })
    }
}

#[async_trait]
impl NetworkAdapter for EthernetAdapter {
    async fn send(&self, destination: Address, frame: &MessageFrame) -> Result<MessageId> {
        // Parse destination address
        let addr: SocketAddr = match destination {
            Address::Ethernet(addr) => addr.parse()?,
            _ => return Err(AdapterError::InvalidAddress),
        };

        // Serialize frame
        let data = frame.to_bytes();

        // Check MTU
        if data.len() > self.config.mtu {
            // TODO: Fragment large messages (Phase 3)
            return Err(AdapterError::MessageTooLarge);
        }

        // Send via UDP
        self.socket.send_to(&data, addr).await?;

        Ok(frame.message_id)
    }

    async fn receive(&self) -> Result<(Address, MessageFrame)> {
        let mut buf = vec![0u8; 65535];

        loop {
            // Receive from UDP socket
            let (len, addr) = self.socket.recv_from(&mut buf).await?;

            // Parse frame
            match MessageFrame::from_bytes(&buf[..len]) {
                Ok(frame) => {
                    let address = Address::Ethernet(addr.to_string());
                    return Ok((address, frame));
                }
                Err(e) => {
                    // Log and continue
                    eprintln!("Failed to parse frame: {}", e);
                    continue;
                }
            }
        }
    }

    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        if !self.config.enable_discovery {
            return Ok(Vec::new());
        }

        // Send multicast DISCOVERY message
        let discovery_msg = /* create DISCOVERY frame */;
        let data = discovery_msg.to_bytes();

        self.multicast_socket.send_to(
            &data,
            format!("{}:{}", self.config.multicast_group, self.config.multicast_port),
        ).await?;

        // Wait for responses (timeout 5 seconds)
        let timeout = Duration::from_secs(5);
        let start = Instant::now();
        let mut peers = Vec::new();

        while start.elapsed() < timeout {
            let remaining = timeout - start.elapsed();

            match timeout_at(Instant::now() + remaining, self.receive()).await {
                Ok(Ok((addr, frame))) => {
                    if frame.message_type == MessageType::Discovery {
                        peers.push(PeerInfo {
                            node_id: frame.source_node_id,
                            address: addr,
                            adapters: vec![AdapterType::Ethernet],
                        });
                    }
                }
                _ => break,
            }
        }

        Ok(peers)
    }

    fn get_capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            adapter_type: AdapterType::Ethernet,
            max_message_size: self.config.mtu,
            typical_latency_ms: 5.0,
            typical_bandwidth_bps: 100_000_000,  // 100 Mbps
            reliability: 0.99,
            range_meters: 100.0,  // Local network
            power_consumption: PowerConsumption::None,  // Mains powered
            cost_per_mb: 0.0,  // Free
            supports_broadcast: true,
            supports_multicast: true,
        }
    }
}
```

---

## Resource Limits

### DHT Storage Limits

```rust
pub const MAX_DHT_STORAGE_BYTES: usize = 100 * 1024 * 1024;  // 100MB
pub const MAX_DHT_KEYS: usize = 10_000;
pub const MAX_VALUE_SIZE: usize = 1 * 1024 * 1024;  // 1MB per value
```

### Message Caching Limits

```rust
pub const MAX_CACHED_MESSAGES_PER_DEST: usize = 100;
pub const MAX_CACHED_MESSAGE_AGE_SECS: u64 = 24 * 3600;  // 24 hours
pub const MAX_TOTAL_CACHED_MESSAGES: usize = 10_000;
pub const MAX_CACHED_MESSAGE_SIZE: usize = 1 * 1024 * 1024;  // 1MB
```

### Rate Limiting

```rust
pub struct RateLimiter {
    /// Messages per minute per node
    per_node_limit: u32,  // Default: 1000/min

    /// Total messages per minute
    global_limit: u32,  // Default: 10000/min

    /// Tracking
    node_counters: HashMap<NodeId, (u32, Instant)>,
    global_counter: (u32, Instant),
}

impl MessageRouter {
    fn check_rate_limit(&mut self, source_node: NodeId) -> Result<()> {
        // Check per-node limit
        let entry = self.rate_limiter.node_counters
            .entry(source_node)
            .or_insert((0, Instant::now()));

        // Reset counter if minute elapsed
        if entry.1.elapsed() >= Duration::from_secs(60) {
            entry.0 = 0;
            entry.1 = Instant::now();
        }

        entry.0 += 1;
        if entry.0 > self.rate_limiter.per_node_limit {
            return Err(RoutingError::RateLimitExceeded);
        }

        // Check global limit
        if self.rate_limiter.global_counter.1.elapsed() >= Duration::from_secs(60) {
            self.rate_limiter.global_counter = (0, Instant::now());
        }

        self.rate_limiter.global_counter.0 += 1;
        if self.rate_limiter.global_counter.0 > self.rate_limiter.global_limit {
            return Err(RoutingError::GlobalRateLimitExceeded);
        }

        Ok(())
    }
}
```

### Memory Limits

```rust
pub const MAX_ROUTING_TABLE_SIZE: usize = 256 * 20;  // 5120 nodes
pub const MAX_SEEN_MESSAGE_CACHE_SIZE: usize = 10_000;
pub const MAX_PENDING_QUERIES: usize = 100;
```

---

## Testing Strategy

### Unit Tests

Each module must have unit tests covering:
- ✅ Core functionality
- ✅ Error conditions
- ✅ Edge cases
- ✅ Resource limits

### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    #[tokio::test]
    async fn test_end_to_end_message_delivery() {
        // Create two nodes
        let node_a = setup_test_node(4001).await;
        let node_b = setup_test_node(4002).await;

        // Node A sends message to Node B
        let message = create_test_message(node_b.node_id());
        node_a.send_message(message.clone()).await.unwrap();

        // Node B receives message
        let received = node_b.receive_message().await.unwrap();

        assert_eq!(received.message_id, message.message_id);
        assert_eq!(received.source_node_id, node_a.node_id());
    }

    #[tokio::test]
    async fn test_multi_hop_routing() {
        // Create three nodes: A -> B -> C
        let node_a = setup_test_node(4001).await;
        let node_b = setup_test_node(4002).await;
        let node_c = setup_test_node(4003).await;

        // Set up routing: A knows B, B knows C, A doesn't know C
        connect_nodes(&node_a, &node_b).await;
        connect_nodes(&node_b, &node_c).await;

        // A sends message to C (via B)
        let message = create_test_message(node_c.node_id());
        node_a.send_message(message.clone()).await.unwrap();

        // C receives message
        let received = node_c.receive_message().await.unwrap();
        assert_eq!(received.message_id, message.message_id);
    }

    #[tokio::test]
    async fn test_store_and_forward() {
        let node_a = setup_test_node(4001).await;
        let node_b = setup_test_node(4002).await;

        // Node B is offline
        node_b.stop().await;

        // A sends message to B
        let message = create_test_message(node_b.node_id());
        node_a.send_message(message.clone()).await.unwrap();

        // Message should be cached
        let cached = node_a.get_cached_messages(node_b.node_id()).await.unwrap();
        assert_eq!(cached.len(), 1);

        // Node B comes online
        node_b.start().await;

        // B retrieves cached messages
        let retrieved = node_b.retrieve_cached_messages().await.unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].message_id, message.message_id);
    }

    #[tokio::test]
    async fn test_content_tag_filtering() {
        let relay_node = setup_test_node_with_policy(4002, RelayPolicy {
            enable_filtering: true,
            blocked_tags: vec!["nsfw".to_string()],
            ..Default::default()
        }).await;

        // Message with NSFW tag should be filtered
        let mut message = create_test_message(/* dest */);
        message.routing_flags = RoutingFlags::RELAY_FILTERABLE;
        message.content_tags = vec!["nsfw".to_string()];

        let should_relay = relay_node.should_relay(&message);
        assert_eq!(should_relay, false);

        // Message with SENSITIVE flag should always relay
        message.routing_flags = RoutingFlags::SENSITIVE;
        let should_relay = relay_node.should_relay(&message);
        assert_eq!(should_relay, true);
    }
}
```

### Performance Tests

```rust
#[tokio::test]
async fn benchmark_message_throughput() {
    let node = setup_test_node(4001).await;

    let start = Instant::now();
    let count = 10_000;

    for _ in 0..count {
        let message = create_test_message(/* random dest */);
        node.send_message(message).await.unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = count as f64 / elapsed.as_secs_f64();

    println!("Throughput: {:.2} messages/sec", throughput);
    assert!(throughput > 1000.0);  // Should handle >1000 msg/sec
}
```

### Security Tests

```rust
#[tokio::test]
async fn test_replay_attack_prevention() {
    let node = setup_test_node(4001).await;

    let message = create_test_message(node.node_id());

    // First send succeeds
    node.receive_message(message.clone()).await.unwrap();

    // Replay should be rejected
    let result = node.receive_message(message.clone()).await;
    assert!(matches!(result, Err(RoutingError::ReplayDetected)));
}

#[tokio::test]
async fn test_signature_verification() {
    let node = setup_test_node(4001).await;

    let mut message = create_test_message(node.node_id());

    // Tamper with message
    message.payload[0] ^= 0xFF;

    // Should be rejected
    let result = node.receive_message(message).await;
    assert!(matches!(result, Err(RoutingError::InvalidSignature)));
}
```

---

## Migration Path

### From Phase 1 to Phase 2

**Existing Code** (Phase 1):
- ✅ `myriadmesh-crypto` - No changes needed
- ✅ `myriadmesh-protocol` - Minor updates for new flags/tags
- ✅ `myriadmesh-core` - Integration point for new crates

**New Crates** (Phase 2):
- 🆕 `myriadmesh-dht` - Kademlia DHT implementation
- 🆕 `myriadmesh-routing` - Message router
- 🆕 `myriadmesh-network` - Network abstraction layer
- 🆕 `myriadmesh-adapters/ethernet` - Ethernet adapter

**Updated Workspace Structure**:
```toml
[workspace]
members = [
    "crates/myriadmesh-core",
    "crates/myriadmesh-crypto",      # Phase 1 (no changes)
    "crates/myriadmesh-protocol",    # Phase 1 (minor updates)
    "crates/myriadmesh-dht",         # Phase 2 (NEW)
    "crates/myriadmesh-routing",     # Phase 2 (NEW)
    "crates/myriadmesh-network",     # Phase 2 (NEW)
    "crates/myriadmesh-adapters/ethernet",  # Phase 2 (NEW)
]
```

### Protocol Version Compatibility

**Phase 1 Nodes**: Protocol v1, no DHT, no routing
**Phase 2 Nodes**: Protocol v1, DHT enabled, routing enabled

**Compatibility Strategy**:
- Phase 2 nodes can communicate with Phase 1 nodes (direct only)
- Phase 1 nodes cannot relay or use DHT
- Gradual migration: Deploy Phase 2, Phase 1 still works

---

## Open Questions

### For User Review

1. **Content Tag Namespace**: Should we define a formal tag schema, or allow freeform tags?

2. **Relay Incentives**: Should we implement reputation-based incentives for relay nodes in Phase 2, or defer to Phase 4?

3. **DHT Bootstrap**: How should new nodes bootstrap into the DHT?
   - Option A: Hardcoded bootstrap nodes
   - Option B: DNS-based discovery
   - Option C: Local multicast discovery only

4. **Storage Replication Factor**: How many nodes should store each DHT value? (Currently k=20, maybe too high?)

5. **Message Priority Enforcement**: Should relay nodes be allowed to downgrade message priority, or must they preserve it?

6. **Anonymous Routing**: Should we add basic Tor/i2p integration in Phase 2, or wait for Phase 4?

---

## Implementation Timeline

**Week 1-2**: DHT Implementation
- Routing table
- Kademlia operations
- Storage layer

**Week 3-4**: Message Router
- Priority queues
- Routing logic
- Store-and-forward

**Week 5-6**: Network Abstraction
- Adapter interface
- Adapter manager
- Performance metrics

**Week 7-8**: Ethernet Adapter
- UDP transport
- Multicast discovery
- Testing

**Week 9-10**: Integration & Testing
- End-to-end tests
- Performance tuning
- Documentation

**Week 11-12**: Security Review & Hardening
- Security testing
- Rate limiting tuning
- Bug fixes

**Total**: 12 weeks (3 months)

---

## Success Criteria

Phase 2 is complete when:

- ✅ Two nodes can discover each other via multicast
- ✅ Nodes can exchange messages via Ethernet adapter
- ✅ DHT stores and retrieves node records
- ✅ Messages route via multi-hop (at least 3 hops)
- ✅ Store-and-forward works for offline nodes
- ✅ Content tag filtering works as specified
- ✅ All tests pass (unit, integration, security)
- ✅ Performance meets targets (>1000 msg/sec)
- ✅ Documentation is complete

---

## Next Steps

1. **Review this document** and provide feedback
2. **Answer open questions** above
3. **Approve design** or request changes
4. **Begin implementation** (Week 1: DHT)

---

## Appendix: Configuration Example

```yaml
# MyriadMesh Node Configuration (Phase 2)

node:
  # Node identity (generated on first run)
  identity_file: "~/.myriadmesh/node.key"

  # Network adapters
  adapters:
    ethernet:
      enabled: true
      interface: "eth0"
      port: 4001
      multicast_discovery: true
      multicast_group: "239.255.77.77"

# DHT Configuration
dht:
  # Routing table parameters
  k_bucket_size: 20
  alpha: 3  # Parallel queries

  # Storage limits
  max_storage_bytes: 104857600  # 100MB
  max_keys: 10000
  max_value_size: 1048576  # 1MB

  # Maintenance
  bucket_refresh_interval: 3600  # 1 hour
  republish_interval: 3600

  # Bootstrap nodes
  bootstrap_nodes:
    - "bootstrap1.myriadmesh.org:4001"
    - "bootstrap2.myriadmesh.org:4001"

# Message Router Configuration
routing:
  # Priority queue sizes
  queue_size_per_priority: 1000

  # Rate limiting
  rate_limit_per_node: 1000  # messages/minute
  rate_limit_global: 10000

  # Store-and-forward
  max_cached_messages_per_dest: 100
  max_cached_message_age: 86400  # 24 hours

  # Relay policy
  relay_policy:
    enable_filtering: false
    blocked_tags: []
    allowed_tags: []
    always_relay_sensitive: true
    max_message_size: 1048576
    max_relay_rate: 1000

# Security
security:
  # Reputation
  min_reputation: 0.3

  # Message validation
  max_timestamp_drift: 300  # ±5 minutes
  seen_message_cache_size: 10000
  seen_message_ttl: 3600
```
