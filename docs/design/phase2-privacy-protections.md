# Phase 2: Comprehensive Privacy Protections

**Version:** 2.0
**Date:** 2025-11-12
**Status:** Updated with comprehensive privacy strategies

## Privacy Protection Strategy

MyriadMesh implements defense-in-depth privacy protections that adapt to network constraints while maintaining user transparency and control.

### Design Principles

1. **Layered Defense**: Multiple complementary privacy techniques
2. **Network-Adaptive**: Adjust strategies based on adapter constraints (LoRa vs Ethernet)
3. **User Transparency**: Explicit notifications when privacy is reduced
4. **Graceful Degradation**: Availability-first with privacy warnings
5. **User Control**: Sender can opt-out with full disclosure

---

## Privacy Protection Layers

### Layer 1: Route Randomization (Always On)

**Cost**: Negligible
**Benefit**: Medium-High
**Applies To**: All messages

Instead of always selecting the "best" relay, randomly select from top N candidates:

```rust
impl MessageRouter {
    fn select_relay_with_privacy(
        &self,
        candidates: Vec<RelayNode>,
    ) -> RelayNode {
        // Sort by reputation and performance
        let mut sorted = candidates;
        sorted.sort_by_key(|n| OrderedFloat(-n.reputation.score));

        // Take top k (configurable, default 5)
        let k = self.config.privacy.randomization_pool_size;
        let top_k = &sorted[0..k.min(sorted.len())];

        // Weighted random selection (higher reputation = higher probability)
        weighted_random_choice(top_k, |n| n.reputation.score)
    }
}
```

**Configuration**:
```yaml
privacy:
  route_randomization:
    enabled: true  # Always on
    pool_size: 5   # Select from top 5
```

**Effect**: Malicious relay only sees 1/k of your traffic

---

### Layer 2: Relay Rotation (Always On)

**Cost**: Negligible
**Benefit**: Medium-High
**Applies To**: All multi-hop routes

Periodically change relay nodes, even for the same destination:

```rust
pub struct RouteCache {
    routes: HashMap<(NodeId, NodeId), CachedRoute>,
    rotation_interval: Duration,
}

pub struct CachedRoute {
    relay: NodeId,
    selected_at: Timestamp,
    message_count: u32,
}

impl RouteCache {
    fn get_relay(&mut self, dest: NodeId) -> Option<NodeId> {
        let key = (self.local_node_id, dest);

        if let Some(cached) = self.routes.get_mut(&key) {
            // Check if rotation needed
            if cached.selected_at.elapsed() > self.rotation_interval {
                // Time to rotate
                return None;
            }

            // Also rotate after N messages (prevent correlation)
            if cached.message_count > self.max_messages_per_relay {
                return None;
            }

            cached.message_count += 1;
            return Some(cached.relay);
        }

        None
    }

    fn cache_relay(&mut self, dest: NodeId, relay: NodeId) {
        let key = (self.local_node_id, dest);
        self.routes.insert(key, CachedRoute {
            relay,
            selected_at: Timestamp::now(),
            message_count: 0,
        });
    }
}
```

**Configuration**:
```yaml
privacy:
  relay_rotation:
    enabled: true
    interval_seconds: 3600      # 1 hour
    max_messages_per_relay: 100 # Also rotate after 100 messages
```

**Effect**: Limits time window for surveillance per relay

---

### Layer 3: Iterative DHT Lookup Privacy

**Cost**: 50-100% lookup latency
**Benefit**: Medium
**Applies To**: DHT queries

Don't reveal which node you're looking up:

```rust
impl DhtManager {
    async fn private_lookup(&self, target: NodeId) -> Result<NodeRecord> {
        if !self.config.privacy.private_dht_lookups {
            // Standard lookup
            return self.lookup_node(target).await;
        }

        // Strategy 1: Start from random node instead of closest
        let start_nodes = self.routing_table.get_random_nodes(ALPHA);

        // Strategy 2: Look up with blinded target
        // (Only works if we have some shared secret with target)
        let blinded_target = self.blind_node_id(target);

        // Perform iterative lookup
        self.iterative_lookup(blinded_target, start_nodes).await
    }

    fn blind_node_id(&self, target: NodeId) -> NodeId {
        // Use consistent hashing with random salt
        // Target can unblind, but DHT nodes cannot correlate
        let salt = self.get_or_create_salt_for_target(target);
        blake2b_hash(&[target.as_bytes(), salt.as_bytes()])
    }
}
```

**Configuration**:
```yaml
privacy:
  dht:
    private_lookups: true
    randomize_start_nodes: true
```

**Effect**: DHT nodes can't determine who you're trying to reach

---

### Layer 4: Network-Adaptive Message Padding

**Cost**: 0-30% bandwidth overhead (adapter-dependent)
**Benefit**: High
**Applies To**: All messages (with intelligent adaptation)

Pad messages to fixed buckets, but adapt to network constraints:

```rust
pub struct AdaptivePadding {
    adapter_policies: HashMap<AdapterType, PaddingPolicy>,
}

pub struct PaddingPolicy {
    enabled: bool,
    buckets: Vec<usize>,
    max_overhead_percent: f64,
    notify_on_reduction: bool,
}

impl AdaptivePadding {
    fn get_policy(adapter_type: AdapterType) -> PaddingPolicy {
        match adapter_type {
            AdapterType::Ethernet | AdapterType::Cellular => PaddingPolicy {
                enabled: true,
                buckets: vec![512, 2048, 8192, 32768, 131072],
                max_overhead_percent: 30.0,
                notify_on_reduction: false,
            },

            AdapterType::LoRaWAN => PaddingPolicy {
                enabled: true,
                // LoRa payload limits by spreading factor
                buckets: vec![51, 115, 222],
                max_overhead_percent: 10.0,  // Strict for spectrum
                notify_on_reduction: true,    // CRITICAL: warn user
            },

            AdapterType::Dialup | AdapterType::APRS => PaddingPolicy {
                enabled: false,  // Too constrained
                buckets: vec![],
                max_overhead_percent: 0.0,
                notify_on_reduction: true,
            },

            _ => PaddingPolicy::default(),
        }
    }

    async fn pad_message(
        &self,
        msg: MessageFrame,
        adapter: AdapterType,
    ) -> Result<(MessageFrame, Option<PrivacyNotification>)> {
        let policy = self.get_policy(adapter);

        if !policy.enabled {
            return Ok((msg, Some(PrivacyNotification::PaddingDisabled {
                reason: "Network does not support padding",
                adapter,
            })));
        }

        let original_size = msg.payload.len();
        let target_bucket = policy.buckets.iter()
            .find(|&&b| b >= original_size)
            .copied();

        match target_bucket {
            Some(target_size) => {
                let overhead_pct = (target_size - original_size) as f64
                    / original_size as f64 * 100.0;

                if overhead_pct > policy.max_overhead_percent {
                    // Padding exceeds spectrum budget
                    if policy.notify_on_reduction {
                        return Ok((msg, Some(PrivacyNotification::PaddingExceedsBudget {
                            adapter,
                            original_size,
                            target_size,
                            max_allowed_overhead: policy.max_overhead_percent,
                            options: PaddingOptions {
                                reduce_to_minimum_priority: true,
                                resend_without_padding: true,
                                queue_for_better_adapter: true,
                            },
                        })));
                    } else {
                        // Reduce padding to max allowed
                        let max_size = original_size +
                            (original_size as f64 * policy.max_overhead_percent / 100.0) as usize;
                        return Ok((self.pad_to_size(msg, max_size), None));
                    }
                }

                Ok((self.pad_to_size(msg, target_size), None))
            }
            None => {
                Ok((msg, Some(PrivacyNotification::MessageTooLargeForPadding {
                    adapter,
                    size: original_size,
                    max_bucket: policy.buckets.last().copied().unwrap_or(0),
                })))
            }
        }
    }
}

/// User-facing privacy notifications
pub enum PrivacyNotification {
    PaddingDisabled {
        reason: &'static str,
        adapter: AdapterType,
    },

    PaddingExceedsBudget {
        adapter: AdapterType,
        original_size: usize,
        target_size: usize,
        max_allowed_overhead: f64,
        options: PaddingOptions,
    },

    MessageTooLargeForPadding {
        adapter: AdapterType,
        size: usize,
        max_bucket: usize,
    },

    OnionRoutingDisabled {
        reason: String,
    },

    OnionRoutingDisabledBySender {
        sender: NodeId,
    },

    PrivacyReducedForPerformance {
        features_disabled: Vec<String>,
    },
}

pub struct PaddingOptions {
    /// Reduce to minimum priority, send later
    pub reduce_to_minimum_priority: bool,

    /// Resend without padding (WARN: privacy loss!)
    pub resend_without_padding: bool,

    /// Queue until better adapter available
    pub queue_for_better_adapter: bool,
}
```

**Configuration**:
```yaml
privacy:
  message_padding:
    enabled: true
    # Per-adapter policies auto-configured based on adapter type
    notify_user_on_reduction: true
```

**Effect**: Cannot correlate messages by size

---

### Layer 5: Context-Aware Timing Obfuscation

**Cost**: 100-500ms latency
**Benefit**: Medium
**Applies To**: Single-recipient messages ONLY

Add random delays to single-user messages (not groups/broadcasts):

```rust
impl MessageRouter {
    async fn apply_timing_obfuscation(&self, msg: &MessageFrame) -> Option<Duration> {
        // Skip if disabled
        if !self.config.privacy.timing_obfuscation {
            return None;
        }

        // Only for single-recipient messages
        if msg.dest_node_id == BROADCAST_ID {
            return None;  // No obfuscation for broadcasts
        }

        if self.is_group_destination(msg.dest_node_id).await {
            return None;  // No obfuscation for multicast/groups
        }

        // Single recipient - apply random delay
        let max_delay = self.config.privacy.max_timing_delay_ms;
        let delay_ms = rand::random::<u64>() % max_delay;
        Some(Duration::from_millis(delay_ms))
    }

    async fn is_group_destination(&self, dest: NodeId) -> bool {
        // Check DHT for group record
        if let Ok(Some(record)) = self.dht.find_value(node_key(dest)).await {
            if let Ok(node_record) = NodeRecord::from_bytes(&record) {
                return node_record.is_group;
            }
        }
        false
    }
}
```

**Configuration**:
```yaml
privacy:
  timing_obfuscation:
    enabled: false  # Off by default (latency cost)
    max_delay_ms: 500
    apply_to_groups: false  # Never apply to groups
```

**Effect**: Harder to correlate request/response timing

---

### Layer 6: Lightweight Onion Routing (SENSITIVE Messages)

**Cost**: 2-3x latency, 2-3x bandwidth
**Benefit**: Very High
**Applies To**: Messages with SENSITIVE flag

Three-hop onion routing with sender opt-out:

```rust
pub struct OnionRoutingConfig {
    pub enabled: bool,
    pub hops: usize,  // 3 recommended
    pub allow_sender_override: bool,
    pub notify_on_override: bool,
}

impl MessageRouter {
    async fn route_sensitive_message(
        &self,
        mut msg: MessageFrame,
    ) -> Result<Vec<PrivacyNotification>> {
        let mut notifications = Vec::new();

        if !msg.routing_flags.contains(RoutingFlags::SENSITIVE) {
            return Ok(notifications);
        }

        // Check if sender explicitly disabled onion routing
        if msg.routing_flags.contains(RoutingFlags::NO_ONION_ROUTING) {
            if self.config.onion.allow_sender_override {
                if self.config.onion.notify_on_override {
                    // Notify sender
                    notifications.push(
                        PrivacyNotification::OnionRoutingDisabledBySender {
                            sender: msg.source_node_id,
                        }
                    );

                    // Notify recipient
                    self.send_privacy_warning_to_recipient(
                        msg.dest_node_id,
                        PrivacyWarning::OnionRoutingDisabled,
                    ).await?;
                }

                // Route directly (still E2E encrypted)
                return self.route_direct(msg).await;
            } else {
                // Override not allowed - force onion routing
                notifications.push(PrivacyNotification::OnionRoutingForced {
                    reason: "Policy requires onion routing for SENSITIVE messages",
                });
            }
        }

        // Select onion route
        let route = self.select_onion_route(
            msg.dest_node_id,
            self.config.onion.hops,
        ).await?;

        // Create onion message
        let onion_msg = self.create_onion_message(msg, route)?;

        // Send through first hop
        self.send_to_first_hop(onion_msg).await?;

        Ok(notifications)
    }

    fn create_onion_message(
        &self,
        msg: MessageFrame,
        route: Vec<NodeId>,
    ) -> Result<OnionMessage> {
        // Encrypt message for final destination
        let mut payload = self.encrypt_for_destination(msg)?;

        // Add layers in reverse order (destination -> first hop)
        for (i, hop) in route.iter().enumerate().rev() {
            let next_hop = if i < route.len() - 1 {
                Some(route[i + 1])
            } else {
                None  // Final hop
            };

            payload = self.encrypt_onion_layer(payload, *hop, next_hop)?;
        }

        Ok(OnionMessage {
            first_hop: route[0],
            encrypted_payload: payload,
        })
    }

    fn select_onion_route(&self, dest: NodeId, hops: usize) -> Result<Vec<NodeId>> {
        // Select high-reputation relays
        let candidates = self.dht
            .get_high_reputation_nodes(min_reputation: 0.7)
            .filter(|n| n.node_id != dest);

        let mut route = Vec::new();
        let mut used = HashSet::new();

        for _ in 0..hops {
            let relay = candidates
                .filter(|n| !used.contains(&n.node_id))
                .choose(&mut rand::thread_rng())
                .ok_or(RoutingError::InsufficientRelays)?;

            route.push(relay.node_id);
            used.insert(relay.node_id);
        }

        route.push(dest);
        Ok(route)
    }
}
```

**New Routing Flag**:
```rust
bitflags! {
    pub struct RoutingFlags: u8 {
        const E2E_STRICT = 0b0000_0001;
        const SENSITIVE = 0b0000_0010;
        const RELAY_FILTERABLE = 0b0000_0100;
        const MULTI_PATH = 0b0000_1000;
        const ANONYMOUS = 0b0001_0000;
        const NO_ONION_ROUTING = 0b0010_0000;  // NEW: Sender opts out
    }
}
```

**Configuration**:
```yaml
privacy:
  onion_routing:
    enabled: true
    hops: 3
    allow_sender_override: true   # Sender can disable
    notify_on_override: true      # Notify both parties
    force_for_sensitive: false    # Can make mandatory
```

**Effect**: Each relay only knows previous/next hop, not source or destination

---

### Layer 7: HVT-Based Adaptive Decoy Traffic

**Cost**: User-configurable bandwidth
**Benefit**: High for high-value targets
**Applies To**: Designated HVTs only

Network-aware decoy traffic generation:

```rust
pub struct DecoyTrafficManager {
    hvt_configs: HashMap<NodeId, HvtConfig>,
    adapter_limits: HashMap<AdapterType, DecoyRate>,
}

pub struct HvtConfig {
    pub target: NodeId,
    pub enabled: bool,
    pub base_rate: f64,  // messages per hour
    pub adapters: Vec<AdapterType>,
}

pub struct DecoyRate {
    pub rate: f64,      // messages per hour
    pub max_rate: f64,  // hard limit
}

impl DecoyTrafficManager {
    fn get_adapter_limits() -> HashMap<AdapterType, DecoyRate> {
        hashmap! {
            // High-bandwidth: generous decoys
            AdapterType::Ethernet => DecoyRate {
                rate: 60.0,      // 1/min
                max_rate: 360.0, // 6/min max
            },

            // Metered: conservative
            AdapterType::Cellular => DecoyRate {
                rate: 10.0,      // 1 per 6 min
                max_rate: 30.0,
            },

            // Constrained: minimal
            AdapterType::LoRaWAN => DecoyRate {
                rate: 1.0,       // 1/hour (duty cycle!)
                max_rate: 2.0,
            },

            AdapterType::Dialup => DecoyRate {
                rate: 0.5,       // 1 per 2 hours
                max_rate: 1.0,
            },

            // Shared spectrum: very minimal
            AdapterType::APRS => DecoyRate {
                rate: 0.1,       // 1 per 10 hours
                max_rate: 0.5,
            },

            AdapterType::FRS_GMRS => DecoyRate {
                rate: 0.2,
                max_rate: 1.0,
            },
        }
    }

    async fn start_decoy_traffic_for_hvt(&mut self, config: HvtConfig) {
        let limits = Self::get_adapter_limits();

        for adapter in config.adapters {
            let limit = limits.get(&adapter).unwrap();
            let actual_rate = config.base_rate.min(limit.max_rate);

            tokio::spawn(async move {
                self.generate_decoy_traffic(
                    config.target,
                    adapter,
                    actual_rate,
                ).await;
            });
        }
    }

    async fn generate_decoy_traffic(
        &self,
        hvt: NodeId,
        adapter: AdapterType,
        rate: f64,
    ) {
        loop {
            // Create realistic decoy
            let dest = self.select_random_destination();
            let size = self.select_realistic_size(adapter);

            let decoy = MessageFrame {
                source_node_id: hvt,
                dest_node_id: dest,
                priority: 128,  // Normal
                payload: random_bytes(size),
                routing_flags: RoutingFlags::E2E_STRICT,
                // ... fully encrypted
            };

            // Send decoy
            self.send_via_adapter(decoy, adapter).await.ok();

            // Poisson-distributed delay
            let delay = exponential_delay(rate);
            sleep(delay).await;
        }
    }
}
```

**Configuration**:
```yaml
privacy:
  decoy_traffic:
    enabled: false  # Off by default

    # High-Value Target configurations
    hvt_targets:
      - node_id: "0x1234..."
        enabled: true
        base_rate: 10.0  # 10/hour
        adapters: ["ethernet", "cellular"]

      - sending_id: "journalist@example.i2p"
        enabled: true
        base_rate: 5.0
        adapters: ["ethernet", "i2p"]

    # Per-adapter limits (auto-configured, can override)
    adapter_limits:
      lora: 1.0      # 1/hour max
      dialup: 0.5    # 1 per 2 hours
      aprs: 0.1      # 1 per 10 hours
```

**Effect**: Hard to distinguish real traffic from noise

---

### Layer 8: Full i2p Integration

**Cost**: High latency (1-10s), moderate bandwidth
**Benefit**: Maximum anonymity
**Applies To**: Application choice or ANONYMOUS flag

**DEFAULT: Mode 2 (Selective Disclosure)** - See [i2p Architecture Document](./i2p-anonymity-architecture.md)

Multiple i2p integration modes:

```rust
pub enum I2pIntegrationMode {
    /// Mode 2: Selective Disclosure (DEFAULT) - Best for most users
    /// Separate i2p identity, capability tokens, no public linkage
    SelectiveDisclosure {
        capability_tokens_enabled: bool,
        separate_identity: bool,
    },

    /// Mode 1: i2p-Only Identity - Maximum anonymity
    /// No clearnet presence
    I2pOnly {
        i2p_dht_only: bool,
    },

    /// Mode 3: Anonymous Rendezvous - Easier discovery
    /// Encrypted pointers in DHT (weaker security)
    AnonymousRendezvous {
        rendezvous_key_rotation_hours: u64,
    },

    /// Mode 4: i2p Transport - For relays/exit nodes
    /// Public linkage, best performance
    Transport {
        relay_traffic: bool,
        exit_traffic: bool,
        max_bandwidth_kbps: u64,
    },

    /// Disabled
    Disabled,
}

pub struct I2pConfig {
    /// DEFAULT: SelectiveDisclosure (Mode 2)
    /// For relays/exits, consider Transport (Mode 4)
    pub mode: I2pIntegrationMode,

    pub sam_host: String,
    pub sam_port: u16,
    pub tunnel_length: usize,        // 3 recommended
    pub tunnel_quantity: usize,      // 2 recommended
    pub tunnel_backup_quantity: usize, // 1
    pub publish_destination: bool,

    /// User acknowledgment of mode risks
    pub user_acknowledged_mode_risks: bool,
}

impl MessageRouter {
    async fn route_via_i2p(&self, msg: MessageFrame) -> Result<()> {
        // Check if i2p available
        match &self.config.i2p.mode {
            I2pIntegrationMode::Disabled => {
                return Err(RoutingError::I2pNotAvailable)
            },
            I2pIntegrationMode::SelectiveDisclosure { .. } => {
                // Mode 2: Use capability tokens for private routing
                self.route_via_capability_token(msg).await
            }
            I2pIntegrationMode::I2pOnly { .. } => {
                // Mode 1: i2p-only routing
                self.route_i2p_only(msg).await
            }
            I2pIntegrationMode::AnonymousRendezvous { .. } => {
                // Mode 3: Rendezvous lookup
                self.route_via_rendezvous(msg).await
            }
            I2pIntegrationMode::Transport { .. } => {
                // Mode 4: Direct i2p transport
                self.route_via_i2p_transport(msg).await
            }
        }
    }

    async fn route_via_capability_token(&self, msg: MessageFrame) -> Result<()> {
        // Mode 2: Look up capability token
        let token = self.get_i2p_capability_token(msg.dest_node_id)?;

        // Verify token is valid
        if token.is_expired() {
            return Err(RoutingError::I2pTokenExpired);
        }

        // Send through i2p using token destination
        self.i2p_client.send(token.i2p_destination, &msg.to_bytes()).await?;
        Ok(())
    }
}
```

**Configuration** (Default: Mode 2):
```yaml
i2p:
  # DEFAULT MODE 2: Selective Disclosure (recommended for most users)
  mode:
    type: "selective_disclosure"  # selective_disclosure, i2p_only, rendezvous, transport

    # Mode 2 specific settings
    selective_disclosure:
      capability_tokens_enabled: true
      separate_identity: true
      token_expiry_days: 30

  sam:
    host: "127.0.0.1"
    port: 7656

  tunnels:
    length: 3
    quantity: 2
    backup_quantity: 1

  # For Mode 2, destination is NOT published publicly
  # Only shared via capability tokens
  publish_destination: false

  # User must acknowledge mode selection
  user_acknowledged_risks: false

# Alternative: Mode 4 for Relay/Exit nodes
i2p_relay_config:
  mode:
    type: "transport"  # Mode 4: For high-performance relays

    transport:
      relay_traffic: true
      exit_traffic: false  # Set true for exit nodes (legal considerations!)
      max_bandwidth_kbps: 2048
      allowed_ports: [80, 443]  # For exit nodes only
```

See the [i2p Anonymity Architecture document](./i2p-anonymity-architecture.md) for:
- Detailed mode comparisons
- Security trade-offs
- Application UI guidance
- User-facing risk/benefit explanations

---

## Privacy Configuration Matrix

| Feature | Default | Cost | When to Enable | Notes |
|---------|---------|------|----------------|-------|
| Route Randomization | ✅ ON | Free | Always | Always on |
| Relay Rotation | ✅ ON | Free | Always | Always on |
| DHT Lookup Privacy | ⚠️ OFF | 50-100% DHT latency | High paranoia | Moderate benefit |
| Message Padding | ✅ ON | 0-30% bandwidth | Always | Adapter-aware |
| Timing Obfuscation | ⚠️ OFF | 100-500ms latency | Single-user msgs | Context-aware |
| Onion Routing | ✅ ON (SENSITIVE) | 2-3x resources | Sensitive msgs | Can opt-out |
| Decoy Traffic | ⚠️ OFF | User-defined | HVTs only | Network-aware |
| i2p Integration | ✅ Mode 2 (DEFAULT) | 2-5x latency | Privacy-conscious users | See i2p architecture doc |

---

## Privacy Notifications System

Applications receive explicit notifications when privacy is compromised:

```rust
/// Privacy notification to application layer
pub enum PrivacyEvent {
    /// Padding was disabled for this message
    PaddingDisabled {
        message_id: MessageId,
        adapter: AdapterType,
        reason: String,
    },

    /// Padding exceeds network budget
    PaddingLimited {
        message_id: MessageId,
        adapter: AdapterType,
        original_size: usize,
        padded_size: usize,
        requested_size: usize,
        user_action_required: PaddingOptions,
    },

    /// Onion routing was disabled by sender
    OnionRoutingDisabled {
        message_id: MessageId,
        sender: NodeId,
    },

    /// Onion routing unavailable (insufficient relays)
    OnionRoutingUnavailable {
        message_id: MessageId,
        required_relays: usize,
        available_relays: usize,
    },

    /// Privacy reduced for performance
    PrivacyReduced {
        message_id: MessageId,
        features_disabled: Vec<String>,
        reason: String,
    },

    /// Message sent unencrypted (should never happen!)
    UnencryptedMessage {
        message_id: MessageId,
        reason: String,
    },
}

/// Application subscribes to privacy events
impl MessageRouter {
    pub async fn subscribe_privacy_events(&self) -> Receiver<PrivacyEvent> {
        self.privacy_event_bus.subscribe()
    }
}
```

---

## Updated Configuration Schema

```yaml
# Complete privacy configuration
privacy:
  # Route randomization (always on)
  route_randomization:
    enabled: true
    pool_size: 5

  # Relay rotation (always on)
  relay_rotation:
    enabled: true
    interval_seconds: 3600
    max_messages_per_relay: 100

  # DHT lookup privacy (optional)
  dht:
    private_lookups: false
    randomize_start_nodes: true

  # Message padding (adapter-aware)
  message_padding:
    enabled: true
    notify_on_reduction: true
    # Per-adapter policies auto-configured

  # Timing obfuscation (context-aware)
  timing_obfuscation:
    enabled: false
    max_delay_ms: 500
    apply_to_groups: false

  # Onion routing (SENSITIVE messages)
  onion_routing:
    enabled: true
    hops: 3
    allow_sender_override: true
    notify_on_override: true
    force_for_sensitive: false

  # Decoy traffic (HVT only)
  decoy_traffic:
    enabled: false
    hvt_targets: []
    # adapter_limits auto-configured

  # i2p integration (DEFAULT: Mode 2 - Selective Disclosure)
  i2p:
    mode: "selective_disclosure"  # selective_disclosure (default), i2p_only, rendezvous, transport, disabled
    # See docs/design/i2p-anonymity-architecture.md for detailed mode documentation
```

---

## Implementation Timeline Impact

Adding comprehensive privacy protections:

**Original Timeline**: 12 weeks
**Updated Timeline**: 14 weeks (+2 weeks)

**Breakdown**:
- Week 1-2: DHT Implementation (unchanged)
- Week 3-4: Message Router (unchanged)
- Week 5-6: Network Abstraction (unchanged)
- Week 7-8: Ethernet Adapter (unchanged)
- **Week 9: Privacy Layer Integration** (NEW)
  - Adaptive padding
  - Route randomization/rotation
  - Privacy notifications
- **Week 10: Onion Routing** (NEW)
  - Lightweight 3-hop onion routing
  - Sender opt-out
  - Recipient notifications
- Week 11-12: Integration & Testing (expanded scope)
- Week 13-14: Security Review & Hardening (expanded scope)

---

## Next Steps

1. ✅ Update design documents with comprehensive privacy
2. Review and approve comprehensive approach
3. Begin Phase 2 implementation with privacy-first design
4. Iterative testing of privacy protections
5. User education materials (privacy trade-offs)

---

This comprehensive privacy protection system provides defense-in-depth while maintaining the availability-first protocol philosophy through intelligent adaptation and user transparency.
