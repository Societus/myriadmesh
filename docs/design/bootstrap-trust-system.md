# Bootstrap Trust and Reputation System

**Status:** Design Phase
**Version:** 1.0
**Last Updated:** 2025-01-13

---

## Overview

The Bootstrap Trust and Reputation System prevents malicious nodes from becoming bootstrap nodes and propagating false mesh information. It enables decentralized emergency coordination during "state of emergency" conditions.

---

## Core Problems

### Problem 1: Malicious Bootstrap Nodes

**Scenario:**
```
1. Isolated mesh forms (e.g., disaster area, no internet)
2. Malicious node advertises itself as bootstrap
3. New nodes connect to malicious bootstrap
4. Malicious bootstrap provides false node directory
5. Mesh is compromised
```

**Impact:** Sybil attacks, routing manipulation, censorship

---

### Problem 2: Trust Bootstrapping

**Chicken-and-egg problem:**
- Can't trust bootstrap without reputation
- Can't build reputation without being bootstrap

---

### Problem 3: Emergency Coordination

**Scenario:**
```
Major disaster → Internet down → Isolated meshes form

Need: Coordinated message routing, priority handling, resource allocation
Problem: Who should coordinate? How to elect leader?
```

---

## Solution Architecture

### Three-Tier Trust System

```
Tier 1: Seed Nodes (Hardcoded, Highest Trust)
  └─► Operated by MyriadMesh foundation or community
  └─► Hardcoded in software
  └─► Limited number (5-10 globally)
  └─► Used for initial bootstrapping only

Tier 2: Trusted Nodes (Reputation-Based, Medium Trust)
  └─► Earned through service and uptime
  └─► Can bootstrap new nodes
  └─► Reputation score > threshold
  └─► Elected as emergency coordinators

Tier 3: Regular Nodes (No Trust, Low Privilege)
  └─► New nodes start here
  └─► Cannot bootstrap others
  └─► Build reputation over time
```

---

## Reputation System

### Reputation Score

```rust
pub struct NodeReputation {
    /// Node identifier
    node_id: NodeId,

    /// Overall reputation score (0.0 - 1.0)
    score: f64,

    /// Components
    uptime_score: f64,          // % uptime over last 30 days
    relay_success_rate: f64,    // % messages successfully relayed
    bandwidth_contribution: f64, // Bandwidth provided to mesh
    honesty_score: f64,         // Detected lying/malicious behavior
    age_score: f64,             // How long node has existed

    /// Total messages relayed
    total_relays: u64,

    /// Successful relays
    successful_relays: u64,

    /// Failed relays
    failed_relays: u64,

    /// First seen timestamp
    first_seen: u64,

    /// Last seen timestamp
    last_seen: u64,

    /// Reports of malicious behavior
    malicious_reports: Vec<MaliciousReport>,
}

impl NodeReputation {
    pub fn calculate_overall_score(&self) -> f64 {
        // Weighted average
        let weights = ReputationWeights {
            uptime: 0.25,
            relay_success: 0.30,
            bandwidth: 0.15,
            honesty: 0.20,
            age: 0.10,
        };

        weights.uptime * self.uptime_score +
        weights.relay_success * self.relay_success_rate +
        weights.bandwidth * self.bandwidth_contribution +
        weights.honesty * self.honesty_score +
        weights.age * self.age_score
    }

    pub fn can_bootstrap(&self) -> bool {
        self.score >= 0.7  // 70% threshold
            && self.age_score >= 0.5  // At least 15 days old
            && self.honesty_score >= 0.9  // No malicious behavior
    }

    pub fn can_coordinate_emergency(&self) -> bool {
        self.score >= 0.8  // 80% threshold
            && self.uptime_score >= 0.9  // Very high uptime
            && self.honesty_score >= 0.95  // Nearly perfect honesty
    }
}
```

### Uptime Tracking

```rust
pub struct UptimeTracker {
    /// Heartbeat history for node
    heartbeats: VecDeque<u64>,  // Timestamps

    /// Expected interval
    expected_interval: Duration,

    /// Tracking period
    tracking_period: Duration,  // Default: 30 days
}

impl UptimeTracker {
    pub fn record_heartbeat(&mut self, timestamp: u64) {
        self.heartbeats.push_back(timestamp);

        // Remove old heartbeats outside tracking period
        let cutoff = timestamp - self.tracking_period.as_secs();
        while let Some(first) = self.heartbeats.front() {
            if *first < cutoff {
                self.heartbeats.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn calculate_uptime_score(&self) -> f64 {
        if self.heartbeats.is_empty() {
            return 0.0;
        }

        // Expected number of heartbeats in tracking period
        let expected_count = self.tracking_period.as_secs() / self.expected_interval.as_secs();

        // Actual heartbeats received
        let actual_count = self.heartbeats.len() as u64;

        // Score = actual / expected (capped at 1.0)
        (actual_count as f64 / expected_count as f64).min(1.0)
    }
}
```

### Relay Success Rate

```rust
pub struct RelayTracker {
    /// Successful relays
    successes: u64,

    /// Failed relays
    failures: u64,

    /// Recent relay history (for trend analysis)
    recent: VecDeque<RelayResult>,
}

pub struct RelayResult {
    timestamp: u64,
    success: bool,
    reason: Option<String>,  // Failure reason
}

impl RelayTracker {
    pub fn calculate_success_rate(&self) -> f64 {
        let total = self.successes + self.failures;
        if total == 0 {
            return 1.0;  // Benefit of doubt for new nodes
        }

        self.successes as f64 / total as f64
    }

    pub fn record_relay(&mut self, success: bool, reason: Option<String>) {
        if success {
            self.successes += 1;
        } else {
            self.failures += 1;
        }

        self.recent.push_back(RelayResult {
            timestamp: current_timestamp(),
            success,
            reason,
        });

        // Keep last 1000 relays
        if self.recent.len() > 1000 {
            self.recent.pop_front();
        }
    }
}
```

### Age Score

```rust
impl NodeReputation {
    pub fn calculate_age_score(&self) -> f64 {
        let now = current_timestamp();
        let age_secs = now - self.first_seen;

        // Age milestones (in days)
        const DAY: u64 = 86400;
        match age_secs / DAY {
            0..=6 => 0.0,         // < 1 week: no trust
            7..=13 => 0.2,        // 1 week: minimal trust
            14..=29 => 0.5,       // 2-4 weeks: medium trust
            30..=89 => 0.8,       // 1-3 months: high trust
            _ => 1.0,             // 3+ months: full trust
        }
    }
}
```

### Honesty Score

```rust
pub struct MaliciousReport {
    /// Reporting node
    reporter: NodeId,

    /// Reporter's reputation at time of report
    reporter_reputation: f64,

    /// Type of malicious behavior
    behavior_type: MaliciousBehavior,

    /// Evidence (e.g., invalid signatures, fake messages)
    evidence: Vec<u8>,

    /// Timestamp
    timestamp: u64,
}

pub enum MaliciousBehavior {
    InvalidSignature,
    FakeMessage,
    RouteManipulation,
    SybilAttack,
    DenialOfService,
    Censorship,
}

impl NodeReputation {
    pub fn calculate_honesty_score(&self) -> f64 {
        if self.malicious_reports.is_empty() {
            return 1.0;  // Perfect honesty
        }

        // Weight reports by reporter's reputation
        let total_weight: f64 = self.malicious_reports.iter()
            .map(|report| report.reporter_reputation)
            .sum();

        // Each weighted report reduces score
        let penalty = (total_weight * 0.1).min(1.0);

        (1.0 - penalty).max(0.0)
    }

    pub fn report_malicious_behavior(
        &mut self,
        reporter: NodeId,
        reporter_reputation: f64,
        behavior: MaliciousBehavior,
        evidence: Vec<u8>,
    ) {
        let report = MaliciousReport {
            reporter,
            reporter_reputation,
            behavior_type: behavior,
            evidence,
            timestamp: current_timestamp(),
        };

        self.malicious_reports.push(report);

        // Recalculate scores
        self.honesty_score = self.calculate_honesty_score();
        self.score = self.calculate_overall_score();
    }
}
```

---

## Bootstrap Node Selection

### Centralized Seed Nodes (Tier 1)

**Hardcoded in software:**

```rust
pub const SEED_NODES: &[&str] = &[
    "seed1.myriadmesh.org:4001",
    "seed2.myriadmesh.org:4001",
    "seed3.myriadmesh.org:4001",
    "seed4.myriadmesh.org:4001",
    "seed5.myriadmesh.org:4001",
];

impl BootstrapManager {
    pub fn get_seed_nodes(&self) -> Vec<String> {
        SEED_NODES.iter().map(|s| s.to_string()).collect()
    }

    pub fn is_seed_node(&self, address: &str) -> bool {
        SEED_NODES.contains(&address)
    }
}
```

**Responsibilities:**
- Initial bootstrap for new nodes
- Provide node directory
- Provide trusted node list
- Emergency coordination (fallback)

**Trust:** Implicit (operated by foundation)

---

### Decentralized Trusted Nodes (Tier 2)

**Eligibility:**

```rust
impl BootstrapManager {
    pub fn is_eligible_bootstrap(&self, node_id: &NodeId) -> bool {
        if let Some(reputation) = self.reputation_system.get_reputation(node_id) {
            reputation.can_bootstrap()
        } else {
            false
        }
    }

    pub async fn elect_bootstrap_nodes(&self) -> Vec<NodeId> {
        // Get all nodes with sufficient reputation
        let candidates: Vec<_> = self.reputation_system
            .all_nodes()
            .into_iter()
            .filter(|(_, rep)| rep.can_bootstrap())
            .collect();

        // Sort by reputation score
        let mut sorted = candidates;
        sorted.sort_by(|a, b| b.1.score.partial_cmp(&a.1.score).unwrap());

        // Select top N nodes
        const MAX_BOOTSTRAP_NODES: usize = 50;
        sorted.into_iter()
            .take(MAX_BOOTSTRAP_NODES)
            .map(|(node_id, _)| node_id)
            .collect()
    }
}
```

**Responsibilities:**
- Provide node directory to new nodes
- Verify other bootstrap nodes
- Participate in reputation scoring
- Emergency coordination (elected)

**Trust:** Earned through reputation

---

## Bootstrap Protocol

### New Node Bootstrapping

```rust
impl NewNodeBootstrap {
    pub async fn bootstrap(&mut self) -> Result<Vec<NodeId>> {
        // 1. Try hardcoded seed nodes first
        let mut discovered_nodes = Vec::new();

        for seed in SEED_NODES {
            match self.connect_to_seed(seed).await {
                Ok(nodes) => {
                    discovered_nodes.extend(nodes);
                }
                Err(e) => {
                    warn!("Failed to connect to seed {}: {}", seed, e);
                }
            }
        }

        // 2. Verify received node list
        let verified_nodes = self.verify_node_list(&discovered_nodes).await?;

        // 3. Connect to multiple bootstrap nodes (don't trust single source)
        const MIN_BOOTSTRAP_SOURCES: usize = 3;
        let bootstrap_nodes = self.select_bootstrap_nodes(&verified_nodes, MIN_BOOTSTRAP_SOURCES);

        // 4. Cross-verify information from multiple sources
        let consensus_nodes = self.cross_verify(bootstrap_nodes).await?;

        Ok(consensus_nodes)
    }

    async fn verify_node_list(&self, nodes: &[NodeInfo]) -> Result<Vec<NodeInfo>> {
        let mut verified = Vec::new();

        for node in nodes {
            // Verify signature
            if node.verify_signature().is_ok() {
                // Verify reputation claim
                if self.verify_reputation_claim(&node.node_id, node.claimed_reputation).await.is_ok() {
                    verified.push(node.clone());
                }
            }
        }

        Ok(verified)
    }

    async fn cross_verify(&self, bootstrap_nodes: Vec<NodeId>) -> Result<Vec<NodeInfo>> {
        // Query multiple bootstrap nodes for same information
        let mut node_lists = Vec::new();

        for bootstrap in &bootstrap_nodes {
            let list = self.query_node_list(bootstrap).await?;
            node_lists.push(list);
        }

        // Find consensus (nodes that appear in majority of lists)
        let consensus = self.find_consensus(node_lists);

        Ok(consensus)
    }
}
```

### Preventing Malicious Bootstrap

```rust
impl BootstrapManager {
    pub async fn verify_bootstrap_node(&self, node_id: &NodeId) -> Result<bool> {
        // 1. Check reputation from multiple sources
        let mut reputation_scores = Vec::new();

        for seed in SEED_NODES {
            if let Ok(rep) = self.query_reputation(seed, node_id).await {
                reputation_scores.push(rep);
            }
        }

        // Need at least 2 sources
        if reputation_scores.len() < 2 {
            bail!("Insufficient reputation sources");
        }

        // 2. Calculate median reputation (resistant to outliers)
        let median_reputation = self.median(&reputation_scores);

        // 3. Verify meets threshold
        if median_reputation < 0.7 {
            warn!("Node {} has insufficient reputation: {}", node_id, median_reputation);
            return Ok(false);
        }

        // 4. Check for malicious reports
        let reports = self.query_malicious_reports(node_id).await?;
        if !reports.is_empty() {
            warn!("Node {} has malicious reports: {:?}", node_id, reports);
            return Ok(false);
        }

        Ok(true)
    }
}
```

---

## Emergency Coordinator Election

### State of Emergency Detection

```rust
pub enum EmergencyState {
    Normal,
    Degraded,      // Some nodes unreachable
    Emergency,     // Widespread failure
    Critical,      // Total isolation
}

impl EmergencyDetector {
    pub fn assess_state(&self) -> EmergencyState {
        // Indicators
        let internet_reachable = self.can_reach_internet();
        let seed_nodes_reachable = self.can_reach_seed_nodes();
        let local_mesh_size = self.count_local_peers();

        match (internet_reachable, seed_nodes_reachable, local_mesh_size) {
            (true, true, _) => EmergencyState::Normal,
            (true, false, _) => EmergencyState::Degraded,
            (false, _, size) if size > 10 => EmergencyState::Emergency,
            (false, _, _) => EmergencyState::Critical,
        }
    }
}
```

### Coordinator Election

**Algorithm: Highest Reputation + Age**

```rust
impl EmergencyCoordinator {
    pub async fn elect_coordinator(&self, peers: &[NodeId]) -> Result<NodeId> {
        // Get reputation for all peers
        let mut candidates: Vec<_> = peers.iter()
            .filter_map(|node_id| {
                let rep = self.reputation_system.get_reputation(node_id)?;
                if rep.can_coordinate_emergency() {
                    Some((node_id.clone(), rep))
                } else {
                    None
                }
            })
            .collect();

        if candidates.is_empty() {
            bail!("No eligible coordinators");
        }

        // Sort by combined score: reputation * 0.7 + age * 0.3
        candidates.sort_by(|a, b| {
            let score_a = a.1.score * 0.7 + a.1.age_score * 0.3;
            let score_b = b.1.score * 0.7 + b.1.age_score * 0.3;
            score_b.partial_cmp(&score_a).unwrap()
        });

        // Highest score wins
        Ok(candidates[0].0.clone())
    }

    pub async fn announce_coordinator(&self, coordinator: &NodeId) -> Result<()> {
        // Create announcement
        let announcement = EmergencyAnnouncement {
            coordinator: coordinator.clone(),
            elected_by: self.node_id.clone(),
            timestamp: current_timestamp(),
            state: EmergencyState::Emergency,
            signature: self.sign_announcement(coordinator)?,
        };

        // Broadcast to all peers
        self.broadcast_announcement(announcement).await?;

        Ok(())
    }
}
```

### Coordinator Responsibilities

```rust
pub struct EmergencyCoordinator {
    /// Am I the coordinator?
    is_coordinator: bool,

    /// Current emergency level
    emergency_level: u8,  // 0-5

    /// Message scheduling
    message_scheduler: MessageScheduler,

    /// Spectrum allocation
    spectrum_allocator: SpectrumAllocator,

    /// Priority routing
    priority_router: PriorityRouter,
}

impl EmergencyCoordinator {
    pub async fn coordinate_emergency(&mut self) -> Result<()> {
        if !self.is_coordinator {
            return Ok(());
        }

        // 1. Assess resource constraints
        let available_bandwidth = self.assess_bandwidth().await?;
        let available_spectrum = self.assess_spectrum().await?;

        // 2. Set priority levels
        self.priority_router.set_priorities(vec![
            (MessagePriority::Emergency, 0.5),    // 50% of resources
            (MessagePriority::HighPriority, 0.3), // 30% of resources
            (MessagePriority::Normal, 0.2),       // 20% of resources
        ]);

        // 3. Schedule messages to conserve spectrum
        self.message_scheduler.enable_batching(true);
        self.message_scheduler.set_batch_window(Duration::from_secs(60));

        // 4. Allocate spectrum for emergency tags
        self.spectrum_allocator.allocate_tag_bandwidth("FEMA", 1024 * 10)?; // 10 KB/s

        info!("Emergency coordination active at level {}", self.emergency_level);

        Ok(())
    }
}
```

---

## Reputation Distribution and Synchronization

### Problem

How do nodes agree on reputation scores?

### Solution: Gossip Protocol

```rust
pub struct ReputationGossip {
    /// Local reputation database
    local_reputations: HashMap<NodeId, NodeReputation>,

    /// Pending updates to gossip
    pending_updates: Vec<ReputationUpdate>,

    /// Gossip interval
    interval: Duration,
}

pub struct ReputationUpdate {
    node_id: NodeId,
    score: f64,
    components: ReputationComponents,
    reporter: NodeId,
    timestamp: u64,
    signature: Vec<u8>,
}

impl ReputationGossip {
    pub async fn gossip_loop(&mut self) {
        let mut ticker = interval(self.interval);

        loop {
            ticker.tick().await;

            // Select random peers
            let peers = self.select_random_peers(3);

            for peer in peers {
                // Exchange reputation updates
                let updates = self.exchange_updates(&peer).await;

                // Merge updates into local database
                self.merge_updates(updates).await;
            }
        }
    }

    async fn merge_updates(&mut self, updates: Vec<ReputationUpdate>) {
        for update in updates {
            // Verify signature
            if update.verify().is_err() {
                continue;
            }

            // Get existing reputation
            let existing = self.local_reputations.entry(update.node_id.clone())
                .or_insert_with(|| NodeReputation::new(update.node_id.clone()));

            // Merge using weighted average (weight by reporter reputation)
            self.merge_reputation(existing, &update);
        }
    }

    fn merge_reputation(&mut self, existing: &mut NodeReputation, update: &ReputationUpdate) {
        // Weight update by reporter's reputation
        let reporter_rep = self.local_reputations.get(&update.reporter)
            .map(|r| r.score)
            .unwrap_or(0.5);  // Default medium trust for unknown reporters

        // Weighted average
        let new_score = existing.score * 0.8 + update.score * 0.2 * reporter_rep;

        existing.score = new_score;
        existing.last_updated = update.timestamp;
    }
}
```

---

## Configuration

```toml
[bootstrap]
enabled = true

# Seed nodes (hardcoded, highest trust)
seed_nodes = [
    "seed1.myriadmesh.org:4001",
    "seed2.myriadmesh.org:4001",
    "seed3.myriadmesh.org:4001",
]

# Reputation thresholds
[bootstrap.reputation]
min_bootstrap_score = 0.7
min_coordinator_score = 0.8
min_age_days = 15

# Emergency detection
[bootstrap.emergency]
enabled = true
detection_interval_secs = 60
internet_check_hosts = ["8.8.8.8", "1.1.1.1"]

# Reputation gossip
[bootstrap.gossip]
enabled = true
interval_secs = 300  # 5 minutes
peers_per_round = 3
```

---

## Security Considerations

### Sybil Attack Resistance

**Problem:** Attacker creates many nodes to gain reputation

**Mitigations:**
1. **Age requirement**: New nodes can't be bootstraps (15+ days)
2. **Proof of work**: Reputation requires actual relay work
3. **Bandwidth limits**: Can't accumulate bandwidth score without resources
4. **Cross-verification**: Multiple sources must agree on reputation

### Reputation Poisoning

**Problem:** Malicious nodes report false reputations

**Mitigations:**
1. **Weight by reporter reputation**: Reports from low-rep nodes ignored
2. **Outlier detection**: Median-based scoring resistant to outliers
3. **Evidence requirement**: Malicious reports must include cryptographic proof

### Coordinator Hijacking

**Problem:** Attacker becomes emergency coordinator

**Mitigations:**
1. **High reputation threshold**: 80% reputation + 95% honesty
2. **Transparent election**: All nodes see election process
3. **Challenge mechanism**: Nodes can challenge coordinator
4. **Rotation**: Coordinators rotate periodically

---

## Related Documents

- [Heartbeat Protocol](./heartbeat-protocol.md)
- [State of Emergency Protocol](./emergency-protocol.md)
- [Account and Identity Model](./account-identity-model.md)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-01-13 | Claude | Initial design |
