# Heartbeat Protocol Design

**Status:** Design Phase
**Version:** 1.0
**Last Updated:** 2025-01-13

---

## Executive Summary

The MyriadMesh Heartbeat Protocol enables nodes to discover and monitor peer availability across multiple network adapters while preserving privacy and minimizing bandwidth usage. The protocol integrates with the message acknowledgement system to avoid redundant transmissions and supports both normal operation and emergency coordination modes.

---

## Table of Contents

1. [Design Principles](#design-principles)
2. [Architecture Overview](#architecture-overview)
3. [Heartbeat Message Format](#heartbeat-message-format)
4. [Discovery Mechanisms](#discovery-mechanisms)
5. [Adapter Selection](#adapter-selection)
6. [Message Suppression](#message-suppression)
7. [Round-Robin Acknowledgement](#round-robin-acknowledgement)
8. [Adapter Health Checking](#adapter-health-checking)
9. [Timing and Rate Limiting](#timing-and-rate-limiting)
10. [Security Model](#security-model)
11. [Configuration](#configuration)
12. [Implementation Plan](#implementation-plan)

---

## Design Principles

### Core Tenets

1. **Privacy by Default**: Heartbeats reveal minimal information; local broadcast is optional
2. **Proof-of-Life Optimization**: Message confirmations serve as implicit heartbeats
3. **Bandwidth Conservation**: Suppress heartbeats when activity proves liveness
4. **Adapter Redundancy**: Periodically verify rarely-used adapters remain functional
5. **User Control**: Granular configuration per adapter and network type
6. **Emergency Adaptability**: Support state-of-emergency coordination mode

### Trade-offs

| Aspect | Choice | Rationale |
|--------|--------|-----------|
| **Discovery** | Optional Local Broadcast + DHT | Privacy by default, global reach optional |
| **Information Disclosure** | Active Adapter Only | Minimize fingerprinting |
| **Backhaul Usage** | Optional, Granular Control | User decides based on threat model |
| **Frequency** | Adaptive based on activity | Conserve bandwidth, implicit proof-of-life |
| **Storage** | In-Memory with expiry | Privacy-preserving, no forensic artifacts |

---

## Architecture Overview

### System Components

```
┌─────────────────────────────────────────────────────────────┐
│                      MyriadNode                              │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────────┐          ┌──────────────────┐        │
│  │ HeartbeatService │◄────────►│ AdapterManager   │        │
│  └────────┬─────────┘          └──────────────────┘        │
│           │                                                  │
│           ├──────────► BackhaulDetector                     │
│           ├──────────► MessageTracker (proof-of-life)       │
│           ├──────────► ReputationSystem                     │
│           └──────────► EmergencyCoordinator                 │
│                                                              │
│  ┌───────────────────────────────────────────────┐         │
│  │         Heartbeat Broadcasting                 │         │
│  ├───────────────────────────────────────────────┤         │
│  │  • Adaptive timing (suppress if active)       │         │
│  │  • Per-adapter configuration                  │         │
│  │  • Round-robin acknowledgement                │         │
│  │  • Ed25519 signatures                         │         │
│  └───────────────────────────────────────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

### Data Flow

```
1. Normal Operation:
   Node A ──[Message]──► Node B
   Node B ──[Ack/Hash]──► Node A (via different adapter if possible)

   → If Ack received within timeout: Suppress heartbeat
   → If no activity for N seconds: Send heartbeat

2. Heartbeat Broadcast:
   Node A ──[Heartbeat]──► Local Network (mDNS/Broadcast)
          └─[Heartbeat]──► DHT (if configured)
          └─[Heartbeat]──► Bootstrap Nodes (if configured)

3. Adapter Health Check:
   If rarely-used adapter (e.g., LoRaWAN) not used in >24h:
   Node A ──[Ping/Heartbeat]──► Node B (via LoRaWAN)
   Node B ──[Pong/Ack]──────────► Node A

   → Confirms adapter redundancy remains functional
```

---

## Heartbeat Message Format

### Current Structure (Phase 1)

```rust
pub struct HeartbeatMessage {
    /// Node identifier (can be ephemeral for privacy)
    pub node_id: NodeId,

    /// Unix timestamp (with jitter to prevent timing analysis)
    pub timestamp: u64,

    /// ONLY the active adapter that sent this heartbeat
    /// (not all adapters - reduces fingerprinting)
    pub active_adapter: AdapterInfo,

    /// Optional geolocation (default: None for privacy)
    pub geolocation: Option<GeolocationData>,

    /// Ed25519 signature of (node_id || timestamp || active_adapter)
    pub signature: Vec<u8>,

    /// Optional: Emergency mode flag
    pub emergency_mode: Option<EmergencyState>,
}

pub struct AdapterInfo {
    /// Generic type (e.g., "ip-based", "radio", "bluetooth")
    /// Not specific type to reduce fingerprinting
    pub adapter_type: String,

    /// Contact address for this adapter
    pub address: String,

    /// Capability flags (optional)
    pub capabilities: Option<AdapterCapabilities>,
}

pub struct EmergencyState {
    /// Is this node in emergency mode?
    pub active: bool,

    /// Emergency coordinator node (if any)
    pub coordinator: Option<NodeId>,

    /// Emergency level (0-5, where 5 is critical)
    pub level: u8,
}
```

### Message Size Analysis

| Field | Size | Notes |
|-------|------|-------|
| node_id | 32 bytes | Ed25519 public key |
| timestamp | 8 bytes | u64 unix timestamp |
| active_adapter | ~100 bytes | Type + address + capabilities |
| geolocation | 0-40 bytes | Optional (default: None) |
| signature | 64 bytes | Ed25519 signature |
| emergency_mode | 0-36 bytes | Optional |
| **Total** | **~240 bytes** | Without geolocation/emergency |

**Spectrum Efficiency:**
- LoRaWAN: Fits in single packet (242 bytes max)
- BLE: Fits in 2-3 advertising packets
- Satellite: Acceptable for Iridium SBD (340 bytes max)

---

## Discovery Mechanisms

### Method 1: Local Network Broadcast (Optional)

**Technology:** mDNS (Bonjour) + UDP Multicast

```rust
// Configuration
[heartbeat.discovery.local]
enabled = false  # Privacy default: disabled
multicast_address = "239.255.77.77"
port = 4001
ttl = 1  # Don't cross router boundaries
```

**Behavior:**
- Advertise service: `_myriadmesh._tcp.local`
- Broadcast heartbeat to multicast group
- Zero-config discovery for local mesh

**Privacy Impact:** 🔴 Low - Reveals presence on local network

**Use Case:** Home networks, office deployments, non-sensitive environments

---

### Method 2: DHT-Based Discovery (Global)

**Technology:** Kademlia DHT (future integration)

```rust
// Node publishes heartbeat to DHT
dht.put(node_id, heartbeat_message, ttl=300);

// Other nodes query nearby nodes
nearby_nodes = dht.find_nodes_near(my_location, radius_km);
```

**Privacy Impact:** 🟡 Medium - Queries reveal interest

**Use Case:** Global mesh, internet-connected nodes

---

### Method 3: Bootstrap Nodes (Trusted Relays)

**Technology:** Direct connection to known bootstrap nodes

```rust
[heartbeat.discovery.bootstrap]
nodes = ["bootstrap.myriadmesh.org:4001"]
enabled = true
require_reputation_threshold = 0.7  # Only trust reputable bootstraps
```

**Behavior:**
- New nodes connect to bootstrap nodes
- Bootstrap nodes maintain node registry
- Bootstrap nodes require reputation before advertising others
- Emergency mode: Bootstrap becomes coordinator

**Privacy Impact:** 🟡 Medium - Bootstrap sees all connections

**Use Case:** Initial discovery, emergency coordination

---

## Adapter Selection

### Decision Tree

```
For each heartbeat broadcast:

1. Check if backhaul detection is enabled:
   ├─ If YES: Query BackhaulDetector
   │   ├─ If adapter is backhaul AND allow_backhaul_mesh = false
   │   │   └─ SKIP this adapter
   │   └─ If adapter is backhaul AND allow_backhaul_mesh = true
   │       └─ CHECK per-adapter config
   └─ If NO: Proceed to per-adapter config

2. Check per-adapter configuration:
   ├─ adapters.{type}.allow_heartbeat = true?
   │   └─ BROADCAST on this adapter
   └─ adapters.{type}.allow_heartbeat = false?
       └─ SKIP this adapter

3. Check privacy threshold (optional):
   ├─ adapter.privacy_level >= min_heartbeat_privacy?
   │   └─ BROADCAST on this adapter
   └─ adapter.privacy_level < min_heartbeat_privacy?
       └─ SKIP this adapter
```

### Example Configuration

```toml
[heartbeat]
enabled = true
broadcast_on_backhaul = false  # Global default

# Privacy filter
min_heartbeat_privacy = 0.0  # 0.0 = all, 1.0 = only anonymous adapters

# Per-adapter overrides
[network.adapters.ethernet]
allow_heartbeat = true

[network.adapters.cellular]
allow_heartbeat = false  # Never broadcast over cellular

[network.adapters.i2p]
allow_heartbeat = true

[network.adapters.bluetooth]
allow_heartbeat = true

[network.adapters.lorawan]
allow_heartbeat = true
heartbeat_interval_override = 300  # Less frequent due to spectrum constraints
```

---

## Message Suppression

### Proof-of-Life via Message Activity

**Principle:** If a node has recently confirmed message receipt, a heartbeat is redundant.

```rust
struct MessageTracker {
    /// Last confirmed activity per peer
    last_activity: HashMap<NodeId, Instant>,

    /// Suppression threshold
    suppress_heartbeat_after_activity: Duration,  // Default: 120 seconds
}

impl HeartbeatService {
    async fn should_send_heartbeat(&self, peer: &NodeId) -> bool {
        if let Some(last_active) = self.message_tracker.last_activity(peer) {
            let since_activity = Instant::now() - last_active;

            // Suppress if recent activity
            if since_activity < self.config.suppress_heartbeat_after_activity {
                debug!("Suppressing heartbeat to {}: recent message activity", peer);
                return false;
            }
        }

        true
    }
}
```

### Activity Types that Suppress Heartbeats

1. **Message Acknowledgement Received** - Peer confirmed receipt
2. **Message Relayed** - Peer forwarded message to next hop
3. **DHT Response** - Peer responded to DHT query
4. **Adapter Health Check Response** - Peer responded to ping

### Configuration

```toml
[heartbeat.suppression]
enabled = true
suppress_after_message_ack = true
suppress_after_relay = true
suppress_after_dht_query = true
suppress_timeout_secs = 120  # Don't send heartbeat if activity within 2 minutes
```

---

## Round-Robin Acknowledgement

### Multi-Adapter Message Flow

**Goal:** Confirm multiple adapters are functional by using different adapters for send/ack.

```
Scenario: Node A has Ethernet + Bluetooth, Node B has Ethernet + Bluetooth

Message Send:
  Node A ──[Message via Ethernet]──► Node B

Acknowledgement (Round-Robin):
  Node B ──[Ack via Bluetooth]──────► Node A

Result:
  ✓ Ethernet confirmed (message delivery)
  ✓ Bluetooth confirmed (ack delivery)
  ✓ Both adapters verified functional
  ✓ No separate heartbeat needed
```

### Short Hash for Spectrum-Constrained Adapters

**Problem:** LoRaWAN/Satellite have tiny payload limits (11-50 bytes)

**Solution:** Send truncated message hash for acknowledgement

```rust
pub struct MessageAck {
    /// Message ID (truncated hash)
    pub message_hash: [u8; 8],  // First 8 bytes of BLAKE3 hash

    /// Acknowledgement type
    pub ack_type: AckType,  // Received, Relayed, Delivered

    /// Signature (optional for low-bandwidth adapters)
    pub signature: Option<Vec<u8>>,
}

// Total size: 8 + 1 + 64 = 73 bytes (with signature)
//            8 + 1 = 9 bytes (without signature for LoRaWAN)
```

**Security Consideration:** 8-byte hash has collision probability of 1 in 2^64 (acceptable for short time windows)

### Implementation

```rust
impl AdapterManager {
    async fn send_acknowledgement(
        &self,
        message_hash: [u8; 8],
        recipient: NodeId,
        exclude_adapter: Option<String>,  // Adapter that sent original message
    ) -> Result<()> {
        // Get all ready adapters
        let adapters = self.get_ready_adapters();

        // Filter out the adapter that sent the original message
        let available = if let Some(exclude) = exclude_adapter {
            adapters.into_iter()
                .filter(|a| a.id() != exclude)
                .collect()
        } else {
            adapters
        };

        // Prefer different adapter for round-robin verification
        if let Some(alternate_adapter) = available.first() {
            let ack = MessageAck {
                message_hash,
                ack_type: AckType::Received,
                signature: Some(self.sign_ack(&message_hash)?),
            };

            alternate_adapter.send_ack(recipient, ack).await?;
        } else {
            // Fallback: use same adapter if no alternative
            // (better to confirm than not acknowledge)
        }

        Ok(())
    }
}
```

---

## Adapter Health Checking

### Problem Statement

If two nodes primarily communicate via Wi-Fi, their LoRaWAN/BLE adapters may never be tested and could fail silently.

### Solution: Periodic Redundant Adapter Checks

```rust
struct AdapterHealthChecker {
    /// Last time each adapter was used per peer
    adapter_last_used: HashMap<(NodeId, String), Instant>,

    /// Health check interval per adapter type
    check_intervals: HashMap<String, Duration>,
}

impl AdapterHealthChecker {
    async fn check_redundant_adapters(&self, peer: &NodeId) {
        let peer_adapters = self.get_peer_adapters(peer);

        for adapter_info in peer_adapters {
            let last_used = self.adapter_last_used
                .get(&(peer.clone(), adapter_info.id.clone()))
                .copied()
                .unwrap_or(Instant::now() - Duration::from_secs(86400 * 365)); // Ancient

            let interval = self.check_intervals
                .get(&adapter_info.adapter_type)
                .copied()
                .unwrap_or(Duration::from_secs(86400)); // Default: 24 hours

            if last_used.elapsed() > interval {
                info!("Adapter {} to peer {} unused for {:?}, sending health check",
                    adapter_info.id, peer, last_used.elapsed());

                self.send_health_check(peer, &adapter_info.id).await;
            }
        }
    }

    async fn send_health_check(&self, peer: &NodeId, adapter_id: &str) {
        // Send minimal heartbeat or ping
        let ping = HeartbeatMessage::minimal(self.node_id, adapter_id);

        match self.adapter_manager.send_via_adapter(adapter_id, peer, ping).await {
            Ok(_) => info!("Health check sent via {}", adapter_id),
            Err(e) => warn!("Health check failed on {}: {}", adapter_id, e),
        }
    }
}
```

### Check Intervals by Adapter Type

```toml
[heartbeat.health_checks]
enabled = true

# How long to wait before checking unused adapters
[heartbeat.health_checks.intervals]
ethernet = 86400      # 24 hours
bluetooth = 43200     # 12 hours
bluetooth_le = 43200  # 12 hours
lorawan = 604800      # 7 days (expensive, check less often)
cellular = 86400      # 24 hours
i2p = 43200           # 12 hours
dun = 2592000         # 30 days (DUN is expensive, check infrequently)
```

### Special Case: DUN (Dial-Up Networking)

**Problem:** Repeated dial-ins to the same node waste time and spectrum

**Solution:** Trusted nodes report DUN health to mesh

```rust
pub struct DUNHealthReport {
    /// Node providing DUN
    pub dun_node: NodeId,

    /// Is DUN reachable?
    pub reachable: bool,

    /// Last successful connection
    pub last_successful: u64,

    /// Reporting node (must be trusted)
    pub reporter: NodeId,

    /// Reporter's reputation score
    pub reporter_reputation: f64,
}

// Nodes cache DUN health reports from trusted peers
// Only dial if:
//  1. No recent report exists, OR
//  2. Report is from low-reputation node, OR
//  3. Need to verify (periodic check)
```

---

## Timing and Rate Limiting

### Adaptive Heartbeat Interval

```rust
impl HeartbeatService {
    fn calculate_next_heartbeat(&self, peer: &NodeId) -> Duration {
        let base_interval = self.config.interval_secs;

        // Add jitter to prevent timing analysis
        let jitter = rand::random::<u64>() % (base_interval / 3);
        let jittered = base_interval + jitter - (base_interval / 6);

        // Check for recent activity
        if let Some(last_activity) = self.message_tracker.last_activity(peer) {
            let since_activity = Instant::now() - last_activity;

            // If recent activity, extend interval (adaptive backoff)
            if since_activity < Duration::from_secs(60) {
                // Very recent activity - wait longer
                return Duration::from_secs(jittered * 2);
            }
        }

        Duration::from_secs(jittered)
    }
}
```

### Rate Limiting

**Per-Node Limits:**
```rust
struct HeartbeatRateLimiter {
    /// Minimum interval between heartbeats from same node
    min_interval_per_node: Duration,  // Default: 30 seconds

    /// Last heartbeat received per node
    last_received: HashMap<NodeId, Instant>,
}

impl HeartbeatRateLimiter {
    fn should_accept(&mut self, node_id: &NodeId) -> bool {
        if let Some(last) = self.last_received.get(node_id) {
            if last.elapsed() < self.min_interval_per_node {
                warn!("Rate limiting heartbeat from {}: too frequent", node_id);
                return false;
            }
        }

        self.last_received.insert(node_id.clone(), Instant::now());
        true
    }
}
```

**Global Limits:**
```rust
struct GlobalRateLimiter {
    /// Maximum heartbeats per second (all sources)
    max_per_second: usize,  // Default: 100

    /// Sliding window counter
    recent_heartbeats: VecDeque<Instant>,
}
```

---

## Security Model

### Signature Generation

```rust
use myriadmesh_crypto::{KeyPair, Signature};

impl HeartbeatService {
    fn sign_heartbeat(&self, heartbeat: &HeartbeatMessage) -> Vec<u8> {
        // Canonical serialization for signing
        let canonical = self.serialize_for_signing(heartbeat);

        // Sign with node's private key
        let signature = self.keypair.sign(&canonical);

        signature.to_bytes().to_vec()
    }

    fn serialize_for_signing(&self, hb: &HeartbeatMessage) -> Vec<u8> {
        // Deterministic serialization (exclude signature field)
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&hb.node_id.as_bytes());
        bytes.extend_from_slice(&hb.timestamp.to_be_bytes());
        bytes.extend_from_slice(hb.active_adapter.to_canonical_bytes());

        if let Some(geo) = &hb.geolocation {
            bytes.extend_from_slice(&geo.to_canonical_bytes());
        }

        bytes
    }
}
```

### Signature Verification

```rust
impl HeartbeatService {
    fn verify_heartbeat(&self, heartbeat: &HeartbeatMessage) -> Result<()> {
        // Extract public key from NodeId
        let public_key = heartbeat.node_id.to_public_key();

        // Serialize message for verification
        let canonical = self.serialize_for_signing(heartbeat);

        // Verify signature
        let signature = Signature::from_bytes(&heartbeat.signature)
            .map_err(|e| anyhow!("Invalid signature format: {}", e))?;

        public_key.verify(&canonical, &signature)
            .map_err(|e| anyhow!("Signature verification failed: {}", e))?;

        // Check timestamp freshness (prevent replay attacks)
        let current_time = current_timestamp();
        let time_diff = (current_time as i64 - heartbeat.timestamp as i64).abs();

        if time_diff > 300 {  // 5 minutes tolerance
            bail!("Heartbeat timestamp too old or too far in future");
        }

        Ok(())
    }
}
```

### Replay Attack Prevention

```rust
struct ReplayProtection {
    /// Recently seen (node_id, timestamp) pairs
    seen_heartbeats: LruCache<(NodeId, u64), Instant>,

    /// Cache size
    max_cache_size: usize,  // Default: 10,000
}

impl ReplayProtection {
    fn is_replay(&mut self, node_id: &NodeId, timestamp: u64) -> bool {
        let key = (node_id.clone(), timestamp);

        if self.seen_heartbeats.contains(&key) {
            warn!("Replay attack detected: duplicate heartbeat from {}", node_id);
            return true;
        }

        self.seen_heartbeats.put(key, Instant::now());
        false
    }
}
```

---

## Configuration

### Complete Example

```toml
[heartbeat]
enabled = true
interval_secs = 60
timeout_secs = 300
include_geolocation = false  # Privacy default
store_remote_geolocation = false
max_nodes = 10000

# Discovery methods
[heartbeat.discovery]
method = "hybrid"  # Options: local, dht, bootstrap, hybrid

[heartbeat.discovery.local]
enabled = false  # Privacy default: opt-in
multicast_address = "239.255.77.77"
port = 4001

[heartbeat.discovery.bootstrap]
enabled = true
nodes = ["bootstrap.example.com:4001"]
require_reputation = 0.7

# Adapter selection
broadcast_on_backhaul = false  # Don't use internet uplinks
min_heartbeat_privacy = 0.0    # Accept all privacy levels (0.0-1.0)

# Per-adapter configuration
[network.adapters.ethernet]
allow_heartbeat = true

[network.adapters.cellular]
allow_heartbeat = false

[network.adapters.lorawan]
allow_heartbeat = true
heartbeat_interval_override = 300  # Less frequent

# Message suppression
[heartbeat.suppression]
enabled = true
suppress_after_message_ack = true
suppress_timeout_secs = 120

# Adapter health checks
[heartbeat.health_checks]
enabled = true

[heartbeat.health_checks.intervals]
ethernet = 86400       # 24 hours
bluetooth = 43200      # 12 hours
lorawan = 604800       # 7 days
dun = 2592000          # 30 days

# Rate limiting
[heartbeat.rate_limiting]
min_interval_per_node_secs = 30
max_heartbeats_per_second = 100

# Security
[heartbeat.security]
require_signatures = true
timestamp_tolerance_secs = 300  # Allow 5 min clock skew
enable_replay_protection = true
replay_cache_size = 10000
```

---

## Implementation Plan

### Phase 1: Core Heartbeat Broadcasting (Immediate)

**Files to modify:**
- `crates/myriadnode/src/heartbeat.rs` - Implement broadcasting
- `crates/myriadnode/src/config.rs` - Add configuration options
- `crates/myriadmesh-crypto/src/lib.rs` - Signature helpers
- `crates/myriadnode/tests/integration_tests.rs` - Add tests

**Tasks:**
1. Implement `HeartbeatService::broadcast_loop()`
2. Integrate with AdapterManager to send heartbeats
3. Implement Ed25519 signature generation
4. Implement signature verification
5. Add jittered timing
6. Add per-adapter configuration
7. Add backhaul detection integration
8. Implement rate limiting (per-node + global)
9. Add replay attack protection
10. Create unit tests and integration tests

### Phase 2: Message Suppression & Round-Robin (Next)

**New files:**
- `crates/myriadnode/src/message_tracker.rs` - Track message activity
- `crates/myriadnode/src/acknowledgement.rs` - Ack protocol

**Tasks:**
1. Create MessageTracker to monitor peer activity
2. Implement heartbeat suppression logic
3. Implement round-robin acknowledgement
4. Add short hash acknowledgement for constrained adapters
5. Update tests

### Phase 3: Adapter Health Checking (Follow-up)

**Tasks:**
1. Implement AdapterHealthChecker
2. Add periodic redundant adapter verification
3. Implement DUN health reporting
4. Add configuration for check intervals
5. Update tests

### Phase 4: Discovery Mechanisms (Later)

**Tasks:**
1. Implement mDNS/Bonjour local discovery
2. Implement DHT integration (requires DHT implementation)
3. Implement bootstrap node system
4. Add reputation-based trust for bootstraps
5. Update tests

### Phase 5: Emergency Coordination (Future)

**Tasks:**
1. Implement EmergencyCoordinator
2. Add state-of-emergency detection
3. Implement bootstrap command-and-control
4. Add ephemeral tag system (FEMA, emergency alerts)
5. Integrate with amateur radio translation
6. Update tests

---

## Related Documents

- [Message Acknowledgement Protocol](./message-acknowledgement-protocol.md) (to be created)
- [Account and Identity Model](./account-identity-model.md) (to be created)
- [Bootstrap Trust and Reputation System](./bootstrap-trust-system.md) (to be created)
- [State of Emergency Protocol](./emergency-protocol.md) (to be created)
- [Adapter Privacy Architecture](./adapter-privacy-architecture.md) (existing)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-01-13 | Claude | Initial design based on user requirements |

---

## Open Questions

1. Should heartbeats support message bundling (multiple heartbeats in one packet)?
2. What is the reputation threshold for bootstrap trust?
3. How should emergency mode be triggered (manual, automatic, or both)?
4. Should DUN health reports be signed by multiple nodes for redundancy?
5. What is the collision handling strategy for 8-byte message hash?

---

## Appendix A: Message Size Optimization

For severely constrained adapters (LoRaWAN SF12, satellite):

```rust
pub struct MinimalHeartbeat {
    node_id_hash: [u8; 4],  // Truncated hash (coordination within local mesh)
    timestamp_delta: u16,    // Seconds since epoch % 65536
    adapter_type: u8,        // Enum: 0=IP, 1=Radio, 2=BT, etc.
    // No signature (rely on message-level auth)
}
// Total: 7 bytes (fits in LoRaWAN SF12 with room for headers)
```

**Use case:** Emergency coordination when bandwidth is critically limited.
