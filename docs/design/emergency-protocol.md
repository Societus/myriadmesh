# State of Emergency Protocol

**Status:** Design Phase
**Version:** 1.0
**Last Updated:** 2025-01-13

---

## Overview

The State of Emergency Protocol enables MyriadMesh to adapt to catastrophic events (natural disasters, infrastructure failures, widespread internet outages) by shifting from distributed operation to coordinated emergency response mode.

---

## Emergency States

### State Levels

```rust
pub enum EmergencyState {
    /// Normal operation - internet accessible
    Normal = 0,

    /// Degraded - some nodes unreachable, seed nodes down
    Degraded = 1,

    /// Emergency - widespread failure, isolated mesh
    Emergency = 2,

    /// Critical - total isolation, spectrum constraints
    Critical = 3,

    /// Catastrophic - life-threatening conditions, minimal resources
    Catastrophic = 4,
}

impl EmergencyState {
    pub fn from_level(level: u8) -> Self {
        match level {
            0 => EmergencyState::Normal,
            1 => EmergencyState::Degraded,
            2 => EmergencyState::Emergency,
            3 => EmergencyState::Critical,
            4 => EmergencyState::Catastrophic,
            _ => EmergencyState::Catastrophic,
        }
    }

    pub fn is_emergency(&self) -> bool {
        matches!(self, EmergencyState::Emergency | EmergencyState::Critical | EmergencyState::Catastrophic)
    }
}
```

---

## Emergency Detection

### Automatic Detection

```rust
pub struct EmergencyDetector {
    /// Indicators
    internet_reachable: bool,
    seed_nodes_reachable: u8,  // Count of reachable seeds
    local_mesh_size: usize,
    message_success_rate: f64,
    adapter_failure_rate: f64,

    /// Thresholds
    config: EmergencyConfig,
}

impl EmergencyDetector {
    pub async fn assess_state(&self) -> EmergencyState {
        // Check internet connectivity
        self.internet_reachable = self.check_internet_connectivity().await;

        // Check seed node connectivity
        self.seed_nodes_reachable = self.count_reachable_seeds().await;

        // Count local mesh peers
        self.local_mesh_size = self.count_local_peers().await;

        // Calculate message success rate (last hour)
        self.message_success_rate = self.calculate_message_success_rate();

        // Calculate adapter failure rate
        self.adapter_failure_rate = self.calculate_adapter_failure_rate();

        // Determine state
        self.determine_emergency_level()
    }

    fn determine_emergency_level(&self) -> EmergencyState {
        match (
            self.internet_reachable,
            self.seed_nodes_reachable,
            self.local_mesh_size,
            self.message_success_rate,
        ) {
            // Normal: Internet works, seeds reachable
            (true, seeds, _, _) if seeds >= 2 => EmergencyState::Normal,

            // Degraded: Internet works but seeds unreachable
            (true, _, _, _) => EmergencyState::Degraded,

            // Emergency: No internet, large local mesh
            (false, _, size, rate) if size > 10 && rate > 0.7 => EmergencyState::Emergency,

            // Critical: No internet, small mesh or low success rate
            (false, _, size, rate) if size > 3 && rate > 0.3 => EmergencyState::Critical,

            // Catastrophic: Isolated or failing
            _ => EmergencyState::Catastrophic,
        }
    }

    async fn check_internet_connectivity(&self) -> bool {
        // Try to ping well-known hosts
        for host in &self.config.internet_check_hosts {
            if ping(host).await.is_ok() {
                return true;
            }
        }

        false
    }
}
```

### Manual Trigger

```rust
impl EmergencyCoordinator {
    /// Manually trigger emergency state
    pub async fn declare_emergency(
        &mut self,
        level: u8,
        reason: String,
        auth: AdminAuth,
    ) -> Result<()> {
        // Verify admin authorization
        auth.verify()?;

        // Set emergency state
        self.state = EmergencyState::from_level(level);

        // Log declaration
        info!("Emergency declared: level {}, reason: {}", level, reason);

        // Broadcast emergency announcement
        self.announce_emergency_state(level, reason).await?;

        // Activate emergency mode
        self.activate_emergency_mode().await?;

        Ok(())
    }
}
```

---

## Coordinator Election

### Election Algorithm

```rust
impl EmergencyCoordinator {
    pub async fn elect_coordinator(&self) -> Result<NodeId> {
        // 1. Get all eligible nodes (reputation >= 0.8)
        let candidates = self.get_eligible_coordinators().await?;

        if candidates.is_empty() {
            // Fallback: highest reputation regardless of threshold
            return self.elect_fallback_coordinator().await;
        }

        // 2. Calculate election score for each candidate
        let mut scored: Vec<(NodeId, f64)> = candidates.into_iter()
            .map(|node_id| {
                let score = self.calculate_election_score(&node_id);
                (node_id, score)
            })
            .collect();

        // 3. Sort by score (highest first)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // 4. Return winner
        Ok(scored[0].0.clone())
    }

    fn calculate_election_score(&self, node_id: &NodeId) -> f64 {
        let rep = self.reputation_system.get_reputation(node_id).unwrap();

        // Weighted combination
        rep.score * 0.4 +                    // 40% overall reputation
        rep.uptime_score * 0.3 +             // 30% uptime
        rep.age_score * 0.2 +                // 20% age
        rep.bandwidth_contribution * 0.1     // 10% bandwidth
    }

    async fn announce_coordinator(&self, coordinator: NodeId) -> Result<()> {
        let announcement = CoordinatorAnnouncement {
            coordinator,
            elected_by: self.node_id.clone(),
            election_score: self.calculate_election_score(&coordinator),
            timestamp: current_timestamp(),
            emergency_level: self.state.level(),
            signature: self.sign_announcement(&coordinator)?,
        };

        // Broadcast to all peers
        self.broadcast(announcement).await?;

        Ok(())
    }
}
```

### Coordinator Acceptance

```rust
impl Node {
    async fn handle_coordinator_announcement(
        &mut self,
        announcement: CoordinatorAnnouncement,
    ) -> Result<()> {
        // 1. Verify announcement signature
        announcement.verify()?;

        // 2. Verify coordinator eligibility
        if !self.verify_coordinator_eligibility(&announcement.coordinator).await? {
            warn!("Coordinator {} not eligible, challenging", announcement.coordinator);
            return self.challenge_coordinator(&announcement).await;
        }

        // 3. Accept coordinator
        self.emergency_coordinator.set_coordinator(announcement.coordinator.clone());

        info!("Accepted {} as emergency coordinator", announcement.coordinator);

        Ok(())
    }

    async fn challenge_coordinator(&self, announcement: &CoordinatorAnnouncement) -> Result<()> {
        // Send challenge to coordinator
        let challenge = CoordinatorChallenge {
            challenged_node: announcement.coordinator.clone(),
            challenger: self.node_id.clone(),
            reason: ChallengeReason::InsufficientReputation,
            timestamp: current_timestamp(),
            signature: self.sign_challenge()?,
        };

        self.broadcast(challenge).await?;

        // If multiple nodes challenge, trigger re-election
        Ok(())
    }
}
```

---

## Emergency Coordination Features

### 1. Message Prioritization

```rust
pub enum MessagePriority {
    /// Catastrophic emergency (life-threatening)
    Emergency = 5,

    /// High priority (time-sensitive)
    High = 4,

    /// Important (needs delivery soon)
    Important = 3,

    /// Normal message
    Normal = 2,

    /// Low priority (delay acceptable)
    Low = 1,

    /// Bulk/background (defer indefinitely)
    Bulk = 0,
}

pub struct PriorityRouter {
    /// Resource allocation per priority level
    allocations: HashMap<MessagePriority, f64>,

    /// Queues per priority
    queues: HashMap<MessagePriority, VecDeque<Message>>,

    /// Total available bandwidth
    total_bandwidth: u64,
}

impl PriorityRouter {
    pub fn set_emergency_allocations(&mut self) {
        // Emergency mode: prioritize critical messages
        self.allocations.insert(MessagePriority::Emergency, 0.50);   // 50%
        self.allocations.insert(MessagePriority::High, 0.25);        // 25%
        self.allocations.insert(MessagePriority::Important, 0.15);   // 15%
        self.allocations.insert(MessagePriority::Normal, 0.08);      // 8%
        self.allocations.insert(MessagePriority::Low, 0.02);         // 2%
        self.allocations.insert(MessagePriority::Bulk, 0.00);        // Drop bulk
    }

    pub async fn route_message(&mut self, message: Message) -> Result<()> {
        // Add to appropriate queue
        let queue = self.queues.entry(message.priority).or_insert_with(VecDeque::new);
        queue.push_back(message);

        // Process queues in priority order
        self.process_queues().await
    }

    async fn process_queues(&mut self) -> Result<()> {
        // Process in priority order
        for priority in [
            MessagePriority::Emergency,
            MessagePriority::High,
            MessagePriority::Important,
            MessagePriority::Normal,
            MessagePriority::Low,
            MessagePriority::Bulk,
        ] {
            let allocation = self.allocations.get(&priority).copied().unwrap_or(0.0);
            let bandwidth = (self.total_bandwidth as f64 * allocation) as u64;

            if let Some(queue) = self.queues.get_mut(&priority) {
                self.send_from_queue(queue, bandwidth).await?;
            }
        }

        Ok(())
    }
}
```

### 2. Spectrum Conservation

```rust
pub struct SpectrumConservation {
    /// Message batching window
    batch_window: Duration,

    /// Pending messages to batch
    pending: Vec<Message>,

    /// Compression enabled?
    compression_enabled: bool,

    /// Max payload size per adapter
    max_payloads: HashMap<String, usize>,
}

impl SpectrumConservation {
    pub async fn enable_emergency_mode(&mut self) {
        // Increase batching window (collect more messages before sending)
        self.batch_window = Duration::from_secs(60);

        // Enable compression
        self.compression_enabled = true;

        // Reduce max payloads for spectrum efficiency
        for (adapter, max_size) in &mut self.max_payloads {
            if adapter.contains("lorawan") || adapter.contains("satellite") {
                *max_size = (*max_size / 2).max(11);  // Half size, min 11 bytes
            }
        }

        info!("Spectrum conservation mode enabled");
    }

    pub async fn batch_messages(&mut self) -> Result<Vec<u8>> {
        // Wait for batch window
        tokio::time::sleep(self.batch_window).await;

        // Collect messages
        let messages = std::mem::take(&mut self.pending);

        // Batch into single payload
        let batched = self.create_batch(messages)?;

        // Compress if enabled
        if self.compression_enabled {
            Ok(compress(&batched)?)
        } else {
            Ok(batched)
        }
    }
}
```

### 3. Ephemeral Emergency Tags

```rust
pub struct EmergencyTag {
    /// Tag identifier (e.g., "FEMA", "POLICE", "MEDICAL")
    tag: String,

    /// Assigned priority
    priority: MessagePriority,

    /// Bandwidth allocation (bytes/sec)
    bandwidth_allocation: u64,

    /// Expiry timestamp
    expires_at: u64,

    /// Created by coordinator
    creator: NodeId,

    /// Signature
    signature: Vec<u8>,
}

impl EmergencyCoordinator {
    pub async fn create_emergency_tag(
        &mut self,
        tag: String,
        priority: MessagePriority,
        bandwidth_kb_sec: u64,
        ttl: Duration,
    ) -> Result<()> {
        // Only coordinator can create tags
        if !self.is_coordinator {
            bail!("Only coordinator can create emergency tags");
        }

        let emergency_tag = EmergencyTag {
            tag: tag.clone(),
            priority,
            bandwidth_allocation: bandwidth_kb_sec * 1024,
            expires_at: current_timestamp() + ttl.as_secs(),
            creator: self.node_id.clone(),
            signature: self.sign_tag(&tag)?,
        };

        // Broadcast tag to all nodes
        self.broadcast_emergency_tag(emergency_tag).await?;

        info!("Created emergency tag: {} (priority: {:?}, bandwidth: {} KB/s)",
            tag, priority, bandwidth_kb_sec);

        Ok(())
    }

    pub async fn handle_emergency_tag(&mut self, tag: EmergencyTag) -> Result<()> {
        // Verify tag signature
        tag.verify()?;

        // Verify creator is coordinator
        if tag.creator != self.current_coordinator {
            warn!("Emergency tag from non-coordinator, ignoring");
            return Ok(());
        }

        // Install tag
        self.active_tags.insert(tag.tag.clone(), tag.clone());

        // Configure routing for tag
        self.priority_router.set_tag_allocation(&tag.tag, tag.bandwidth_allocation);

        info!("Installed emergency tag: {}", tag.tag);

        Ok(())
    }
}
```

### 4. Amateur Radio Integration

```rust
pub struct AmateurRadioGateway {
    /// Radio callsign
    callsign: String,

    /// Supported bands
    bands: Vec<String>,  // e.g., ["2m", "70cm", "HF"]

    /// APRS integration
    aprs_enabled: bool,

    /// Voice-to-text transcription
    transcription_enabled: bool,
}

impl AmateurRadioGateway {
    pub async fn relay_amateur_radio_message(
        &mut self,
        mode: RadioMode,
        content: RadioContent,
    ) -> Result<()> {
        match mode {
            RadioMode::APRS => {
                // Parse APRS packet
                let message = self.parse_aprs(content)?;

                // Inject into mesh with FEMA tag
                self.inject_tagged_message(message, "FEMA").await?;
            }
            RadioMode::Voice => {
                // Transcribe voice to text (if enabled)
                if self.transcription_enabled {
                    let text = self.transcribe_voice(content).await?;

                    // Inject into mesh with EMERGENCY tag
                    self.inject_tagged_message(text, "EMERGENCY").await?;
                }
            }
            RadioMode::Digital => {
                // Already digital, inject directly
                self.inject_tagged_message(content.to_string(), "EMERGENCY").await?;
            }
        }

        Ok(())
    }

    async fn inject_tagged_message(&self, content: String, tag: &str) -> Result<()> {
        let message = Message {
            sender: self.gateway_account.clone(),
            content,
            tag: Some(tag.to_string()),
            priority: MessagePriority::Emergency,
            timestamp: current_timestamp(),
            signature: self.sign_message(&content)?,
        };

        // Broadcast to mesh
        self.mesh_node.send_message(message).await?;

        Ok(())
    }
}
```

---

## Emergency Message Formats

### Emergency Alert

```rust
pub struct EmergencyAlert {
    /// Alert type
    alert_type: AlertType,

    /// Severity (1-5)
    severity: u8,

    /// Geographic area (optional)
    area: Option<GeographicArea>,

    /// Alert message
    message: String,

    /// Expiry time
    expires_at: u64,

    /// Issuing authority
    issuer: String,  // e.g., "FEMA", "Police", "Emergency Coordinator"

    /// Signature
    signature: Vec<u8>,
}

pub enum AlertType {
    Evacuation,
    ShelterInPlace,
    MedicalEmergency,
    Fire,
    Flood,
    Earthquake,
    HazardousMaterials,
    Other(String),
}

impl EmergencyAlert {
    pub fn to_minimal_payload(&self) -> Vec<u8> {
        // Minimal format for LoRaWAN/Satellite
        let mut payload = Vec::new();

        // 1 byte: alert type
        payload.push(self.alert_type.to_u8());

        // 1 byte: severity
        payload.push(self.severity);

        // 4 bytes: expiry (hours from now)
        let hours_until_expiry = ((self.expires_at - current_timestamp()) / 3600) as u32;
        payload.extend_from_slice(&hours_until_expiry.to_be_bytes()[2..]);  // 2 bytes

        // 1 byte: message hash (for deduplication)
        let hash = blake3::hash(self.message.as_bytes());
        payload.push(hash.as_bytes()[0]);

        // 8 bytes total (fits in LoRaWAN SF12)
        payload
    }
}
```

### Triage Message

```rust
pub struct TriageMessage {
    /// Patient/incident ID
    incident_id: String,

    /// Triage category
    category: TriageCategory,

    /// Location
    location: Option<GeolocationData>,

    /// Description
    description: String,

    /// Reporter
    reporter: AccountAddress,

    /// Timestamp
    timestamp: u64,
}

pub enum TriageCategory {
    /// Immediate (life-threatening)
    Red,

    /// Urgent (serious but stable)
    Yellow,

    /// Delayed (minor injuries)
    Green,

    /// Deceased or expectant
    Black,
}

impl TriageMessage {
    pub fn priority(&self) -> MessagePriority {
        match self.category {
            TriageCategory::Red => MessagePriority::Emergency,
            TriageCategory::Yellow => MessagePriority::High,
            TriageCategory::Green => MessagePriority::Important,
            TriageCategory::Black => MessagePriority::Normal,
        }
    }
}
```

---

## Heartbeat Frequency Adaptation

### Emergency Heartbeat Schedule

```rust
impl HeartbeatService {
    pub async fn set_emergency_heartbeat_schedule(&mut self, level: u8) {
        match EmergencyState::from_level(level) {
            EmergencyState::Normal => {
                self.interval = Duration::from_secs(60);
            }
            EmergencyState::Degraded => {
                self.interval = Duration::from_secs(45);
            }
            EmergencyState::Emergency => {
                // More frequent heartbeats for coordination
                self.interval = Duration::from_secs(30);
            }
            EmergencyState::Critical => {
                // Very frequent to maintain mesh connectivity
                self.interval = Duration::from_secs(20);
            }
            EmergencyState::Catastrophic => {
                // Balance between connectivity and spectrum conservation
                self.interval = Duration::from_secs(15);
            }
        }

        info!("Heartbeat interval adjusted to {:?} for emergency level {}", self.interval, level);
    }
}
```

---

## Coordinator Rotation

### Scheduled Rotation

```rust
impl EmergencyCoordinator {
    pub async fn rotation_loop(&mut self) {
        let rotation_interval = Duration::from_secs(3600 * 4);  // 4 hours

        let mut ticker = interval(rotation_interval);

        loop {
            ticker.tick().await;

            if self.is_coordinator {
                info!("Coordinator rotation: stepping down");

                // Elect new coordinator
                let new_coordinator = self.elect_coordinator().await.unwrap();

                if new_coordinator != self.node_id {
                    // Announce handoff
                    self.announce_handoff(&new_coordinator).await.unwrap();

                    // Step down
                    self.is_coordinator = false;
                }
            }
        }
    }

    async fn announce_handoff(&self, new_coordinator: &NodeId) -> Result<()> {
        let handoff = CoordinatorHandoff {
            old_coordinator: self.node_id.clone(),
            new_coordinator: new_coordinator.clone(),
            timestamp: current_timestamp(),
            signature: self.sign_handoff(new_coordinator)?,
        };

        self.broadcast(handoff).await?;

        Ok(())
    }
}
```

---

## Configuration

```toml
[emergency]
enabled = true

# Detection
[emergency.detection]
check_interval_secs = 60
internet_check_hosts = ["8.8.8.8", "1.1.1.1"]
min_mesh_size_emergency = 10
min_mesh_size_critical = 3

# Coordinator election
[emergency.coordinator]
min_reputation = 0.8
min_uptime = 0.9
min_age_days = 30
rotation_interval_secs = 14400  # 4 hours

# Message prioritization
[emergency.prioritization]
emergency_allocation = 0.50   # 50% bandwidth
high_allocation = 0.25        # 25% bandwidth
important_allocation = 0.15   # 15% bandwidth
normal_allocation = 0.08      # 8% bandwidth
low_allocation = 0.02         # 2% bandwidth

# Spectrum conservation
[emergency.spectrum]
batch_window_secs = 60
enable_compression = true
reduce_payload_size = 0.5  # Half size on emergency

# Emergency tags
[emergency.tags]
max_active_tags = 10
default_tag_ttl_secs = 86400  # 24 hours

# Amateur radio integration
[emergency.amateur_radio]
enabled = false
callsign = "N0CALL"
aprs_enabled = false
transcription_enabled = false
```

---

## Testing and Simulation

### Emergency Drill

```rust
pub struct EmergencyDrill {
    /// Simulated emergency level
    simulated_level: u8,

    /// Duration of drill
    duration: Duration,

    /// Participating nodes
    participants: Vec<NodeId>,
}

impl EmergencyDrill {
    pub async fn run_drill(&mut self) -> Result<DrillReport> {
        info!("Starting emergency drill: level {}", self.simulated_level);

        // 1. Announce drill start
        self.announce_drill_start().await?;

        // 2. Simulate emergency
        let start = Instant::now();

        while start.elapsed() < self.duration {
            // Collect metrics
            self.collect_metrics().await?;

            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        // 3. Announce drill end
        self.announce_drill_end().await?;

        // 4. Generate report
        Ok(self.generate_report())
    }

    fn generate_report(&self) -> DrillReport {
        DrillReport {
            participants: self.participants.len(),
            coordinator_elected_in: self.metrics.coordinator_election_time,
            avg_message_latency: self.metrics.avg_latency,
            message_success_rate: self.metrics.success_rate,
            spectrum_efficiency: self.metrics.spectrum_efficiency,
        }
    }
}
```

---

## Related Documents

- [Heartbeat Protocol](./heartbeat-protocol.md)
- [Bootstrap Trust and Reputation System](./bootstrap-trust-system.md)
- [Message Acknowledgement Protocol](./message-acknowledgement-protocol.md)
- [Account and Identity Model](./account-identity-model.md)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-01-13 | Claude | Initial design |
