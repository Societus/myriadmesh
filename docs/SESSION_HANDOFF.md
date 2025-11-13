# Phase 3 Heartbeat Implementation - Session Handoff

**Date:** 2025-01-13
**Branch:** `claude/phase-3-readiness-analysis-011CV4L3bSdqmcw9p9DD5KcS`
**Status:** Ready for Implementation
**Next Task:** Implement Phase 1 - Core Heartbeat Broadcasting

---

## 🎯 Summary

This session completed all preparatory work for implementing heartbeat broadcasting. We conducted a comprehensive code review, documented all design decisions, created the configuration system, and identified exactly what needs to be implemented.

**Overall Completion:**
- ✅ Design & Documentation: 100%
- ✅ Configuration System: 100%
- ⚠️ Implementation: 0% (ready to start)

---

## 📚 What Was Completed

### 1. Comprehensive Code Review
**File:** Code review findings documented in conversation

**Key Findings:**
- **10 Critical TODOs** identified across codebase
- **6 MUST FIX items** blocking production:
  1. Implement heartbeat broadcasting (heartbeat.rs:169-172)
  2. Implement signature verification (heartbeat.rs:205, 259)
  3. Fix API to query actual system state
  4. Store metrics in database
  5. Create config.toml with defaults
  6. Add API authentication

**Overall Assessment:** 70% complete, solid foundation

---

### 2. Design Documentation (5 Files, ~4,000 Lines)

**Created:**
- `docs/design/heartbeat-protocol.md` (1,200 lines)
- `docs/design/message-acknowledgement-protocol.md` (800 lines)
- `docs/design/account-identity-model.md` (700 lines)
- `docs/design/bootstrap-trust-system.md` (650 lines)
- `docs/design/emergency-protocol.md` (650 lines)

**Key Decisions Documented:**

| Aspect | Decision | File Reference |
|--------|----------|----------------|
| Local Broadcast | Optional, opt-in | heartbeat-protocol.md:79-115 |
| Backhaul Heartbeats | Configurable per adapter | heartbeat-protocol.md:349-417 |
| Message = Proof-of-Life | Suppress heartbeat after ack | heartbeat-protocol.md:419-470 |
| Round-Robin Acks | Different adapter when possible | message-acknowledgement-protocol.md:37-86 |
| Adapter Health Checks | Periodic for unused adapters | heartbeat-protocol.md:589-696 |
| Bootstrap Trust | Reputation + age requirements | bootstrap-trust-system.md:52-157 |
| Emergency Coordination | Bootstrap nodes coordinate | emergency-protocol.md:95-197 |
| Account Model | Persistent accounts, ephemeral nodes | account-identity-model.md:17-134 |

---

### 3. Configuration System

**Created:**
- `crates/myriadnode/config.toml` (500+ lines, fully documented)

**Updated:**
- `crates/myriadnode/src/config.rs` - Added per-adapter heartbeat settings

**New Configuration Fields:**
```rust
pub struct AdapterConfig {
    pub allow_heartbeat: bool,
    pub heartbeat_interval_override: Option<u64>,
}
```

**Key Defaults:**
- Privacy by default: geolocation disabled, local broadcast opt-in
- Backhaul excluded: `allow_backhaul_mesh = false`
- Cellular excluded: `allow_heartbeat = false` (cost savings)
- 60-second heartbeat interval with 300-second timeout

---

## 🚀 What's Ready to Implement

### Phase 1: Core Heartbeat Broadcasting

**Location:** `crates/myriadnode/src/heartbeat.rs`

**TODOs to Fix:**
1. **Line 169-172:** Implement broadcast loop
2. **Line 205:** Implement signature verification
3. **Line 259:** Implement signature generation

**Required Changes:**

#### 1. Update HeartbeatService Structure

**Current:**
```rust
pub struct HeartbeatService {
    config: HeartbeatConfig,
    local_node_id: NodeId,
    node_map: Arc<RwLock<NodeMap>>,
}
```

**Needs:**
```rust
pub struct HeartbeatService {
    config: HeartbeatConfig,
    local_node_id: NodeId,
    node_map: Arc<RwLock<NodeMap>>,

    // NEW: Add these
    adapter_manager: Arc<RwLock<AdapterManager>>,
    backhaul_detector: Arc<BackhaulDetector>,
    keypair: Arc<KeyPair>,  // For signing
    rate_limiter: Arc<RwLock<HeartbeatRateLimiter>>,
}
```

#### 2. Implement Adapter Info Collection

**Location:** New method in HeartbeatService

```rust
async fn collect_adapter_info(&self) -> Result<Vec<AdapterInfo>> {
    let manager = self.adapter_manager.read().await;
    let mut adapters = Vec::new();

    for adapter_id in manager.adapter_ids() {
        // Get adapter
        let adapter = manager.get_adapter(&adapter_id)?;

        // Check if heartbeat allowed (from config)
        let adapter_config = self.get_adapter_config(&adapter_id)?;
        if !adapter_config.allow_heartbeat {
            continue;
        }

        // Check if backhaul (exclude by default)
        if self.backhaul_detector.is_backhaul(&adapter_id)? {
            if !adapter_config.allow_backhaul_mesh {
                continue;
            }
        }

        // Collect metrics
        let info = AdapterInfo {
            adapter_id: adapter_id.clone(),
            adapter_type: adapter.adapter_type(),
            active: adapter.is_ready(),
            is_backhaul: self.backhaul_detector.is_backhaul(&adapter_id)?,
            bandwidth_bps: adapter.get_bandwidth(),
            latency_ms: adapter.get_latency(),
            reliability: adapter.get_reliability(),
            privacy_level: estimate_privacy_level(&adapter_id),
        };

        adapters.push(info);
    }

    Ok(adapters)
}
```

#### 3. Implement Signature Generation

**Location:** New method in HeartbeatService

```rust
fn sign_heartbeat(&self, heartbeat: &HeartbeatMessage) -> Result<Vec<u8>> {
    // Canonical serialization
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&heartbeat.node_id.as_bytes());
    bytes.extend_from_slice(&heartbeat.timestamp.to_be_bytes());

    // Serialize adapters (deterministic)
    let adapters_json = serde_json::to_vec(&heartbeat.adapters)?;
    bytes.extend_from_slice(&adapters_json);

    // Include geolocation if present
    if let Some(geo) = &heartbeat.geolocation {
        let geo_json = serde_json::to_vec(geo)?;
        bytes.extend_from_slice(&geo_json);
    }

    // Sign with node's keypair
    let signature = self.keypair.sign(&bytes);

    Ok(signature.to_bytes().to_vec())
}
```

#### 4. Implement Signature Verification

**Location:** Update `handle_heartbeat()` method

```rust
pub async fn handle_heartbeat(&self, heartbeat: HeartbeatMessage) -> Result<()> {
    debug!("Received heartbeat from node {:?}", heartbeat.node_id);

    // Verify signature
    let public_key = heartbeat.node_id.to_public_key();

    // Reconstruct signed bytes
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&heartbeat.node_id.as_bytes());
    bytes.extend_from_slice(&heartbeat.timestamp.to_be_bytes());
    let adapters_json = serde_json::to_vec(&heartbeat.adapters)?;
    bytes.extend_from_slice(&adapters_json);
    if let Some(geo) = &heartbeat.geolocation {
        let geo_json = serde_json::to_vec(geo)?;
        bytes.extend_from_slice(&geo_json);
    }

    // Verify
    let signature = Signature::from_bytes(&heartbeat.signature)?;
    public_key.verify(&bytes, &signature)?;

    // Check timestamp freshness (replay protection)
    let now = current_timestamp();
    let age = (now as i64 - heartbeat.timestamp as i64).abs();
    if age > 300 {
        bail!("Heartbeat timestamp too old or too far in future");
    }

    // Check rate limiting
    if !self.rate_limiter.write().await.allow(&heartbeat.node_id) {
        warn!("Rate limiting heartbeat from {:?}", heartbeat.node_id);
        return Ok(());
    }

    // ... rest of existing logic
}
```

#### 5. Implement Broadcast Loop

**Location:** Update line 169-172 in `start()` method

```rust
// Start heartbeat broadcasting task
let config = self.config.clone();
let local_node_id = self.local_node_id;
let adapter_manager = Arc::clone(&self.adapter_manager);
let backhaul_detector = Arc::clone(&self.backhaul_detector);
let keypair = Arc::clone(&self.keypair);

tokio::spawn(async move {
    let mut ticker = interval(Duration::from_secs(config.interval_secs));

    loop {
        ticker.tick().await;

        // Collect adapter information
        match self.collect_adapter_info().await {
            Ok(adapters) => {
                if adapters.is_empty() {
                    debug!("No adapters available for heartbeat broadcast");
                    continue;
                }

                // Generate heartbeat
                let heartbeat = HeartbeatMessage {
                    node_id: local_node_id,
                    timestamp: current_timestamp(),
                    adapters,
                    geolocation: None,  // TODO: Implement geolocation collection
                    signature: Vec::new(),
                };

                // Sign heartbeat
                match self.sign_heartbeat(&heartbeat) {
                    Ok(signature) => {
                        let mut signed_heartbeat = heartbeat;
                        signed_heartbeat.signature = signature;

                        // Broadcast via all eligible adapters
                        self.broadcast_via_adapters(signed_heartbeat).await;
                    }
                    Err(e) => {
                        warn!("Failed to sign heartbeat: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to collect adapter info: {}", e);
            }
        }
    }
});
```

#### 6. Implement Rate Limiting

**Location:** New struct and implementation

```rust
pub struct HeartbeatRateLimiter {
    /// Last heartbeat received per node
    last_received: HashMap<NodeId, Instant>,

    /// Minimum interval between heartbeats
    min_interval: Duration,

    /// Global rate limit (heartbeats per second)
    max_per_second: usize,

    /// Recent heartbeat timestamps (sliding window)
    recent: VecDeque<Instant>,
}

impl HeartbeatRateLimiter {
    pub fn new(min_interval_secs: u64, max_per_second: usize) -> Self {
        Self {
            last_received: HashMap::new(),
            min_interval: Duration::from_secs(min_interval_secs),
            max_per_second,
            recent: VecDeque::new(),
        }
    }

    pub fn allow(&mut self, node_id: &NodeId) -> bool {
        let now = Instant::now();

        // Check per-node rate limit
        if let Some(last) = self.last_received.get(node_id) {
            if now.duration_since(*last) < self.min_interval {
                return false;
            }
        }

        // Check global rate limit
        // Remove entries older than 1 second
        while let Some(front) = self.recent.front() {
            if now.duration_since(*front) > Duration::from_secs(1) {
                self.recent.pop_front();
            } else {
                break;
            }
        }

        if self.recent.len() >= self.max_per_second {
            return false;
        }

        // Allow heartbeat
        self.last_received.insert(*node_id, now);
        self.recent.push_back(now);

        true
    }
}
```

---

## 📋 Files to Modify

### Primary Files:
1. **crates/myriadnode/src/heartbeat.rs**
   - Add adapter_manager, backhaul_detector, keypair fields
   - Implement collect_adapter_info()
   - Implement sign_heartbeat()
   - Update handle_heartbeat() with verification
   - Implement broadcast_via_adapters()
   - Add HeartbeatRateLimiter struct

2. **crates/myriadnode/src/node.rs**
   - Pass AdapterManager, BackhaulDetector, KeyPair to HeartbeatService::new()
   - Update HeartbeatService initialization

3. **crates/myriadmesh-crypto/src/lib.rs** (if needed)
   - Verify signing/verification helpers exist
   - Add any missing utility functions

4. **crates/myriadnode/tests/integration_tests.rs**
   - Add test_heartbeat_broadcasting()
   - Add test_heartbeat_signature_verification()
   - Add test_heartbeat_rate_limiting()
   - Add test_adapter_selection()

---

## 🧪 Test Cases to Add

```rust
#[tokio::test]
async fn test_heartbeat_broadcasting() {
    // Create mock adapter manager with test adapters
    // Create heartbeat service
    // Trigger broadcast
    // Verify heartbeat was signed and broadcast
}

#[tokio::test]
async fn test_heartbeat_signature_verification() {
    // Create heartbeat with valid signature
    // Verify it passes
    // Create heartbeat with invalid signature
    // Verify it fails
}

#[tokio::test]
async fn test_heartbeat_rate_limiting() {
    // Send multiple heartbeats rapidly
    // Verify rate limiting kicks in
}

#[tokio::test]
async fn test_adapter_selection_excludes_backhaul() {
    // Create adapter marked as backhaul
    // Verify it's excluded from heartbeat broadcast
}

#[tokio::test]
async fn test_adapter_selection_respects_config() {
    // Create adapter with allow_heartbeat = false
    // Verify it's excluded from heartbeat broadcast
}
```

---

## 📊 Current Code Statistics

**Commits:**
- 91e3339 - Phase 3: Add comprehensive integration test suite
- be7e1c3 - Phase 3: Add comprehensive protocol design documentation
- d50ad33 - Phase 3: Add comprehensive configuration file and per-adapter settings

**Test Coverage:**
- Unit tests: 23 passing ✅
- Integration tests: 21 passing ✅
- Total: 44 tests, 0 failures

**Lines of Code:**
- Design docs: ~4,000 lines
- Configuration: ~700 lines
- Existing heartbeat: ~500 lines
- TODO: ~200-300 lines to implement

---

## ⚠️ Known Issues (To Be Fixed)

From code review, these remain unfixed:

1. **heartbeat.rs:169-172** - Broadcast loop not implemented ⚠️
2. **heartbeat.rs:205** - Signature verification not implemented ⚠️
3. **heartbeat.rs:259** - Signature generation not implemented ⚠️
4. **api.rs:139** - Uptime always returns 0
5. **api.rs:299** - Backhaul status hardcoded
6. **api.rs:300** - Health status hardcoded
7. **monitor.rs:130,162,195** - Metrics not stored in database
8. **heartbeat.rs:355** - Unsafe unwrap() in current_timestamp()

**Phase 1 will fix:** Issues 1-3
**Future phases:** Issues 4-8

---

## 🎯 Success Criteria for Phase 1

When Phase 1 is complete, you should be able to:

1. ✅ Start MyriadNode with heartbeat service enabled
2. ✅ See heartbeats broadcast every 60 seconds
3. ✅ Heartbeats include adapter information
4. ✅ Heartbeats are signed with Ed25519
5. ✅ Received heartbeats are verified
6. ✅ Backhaul adapters are excluded (configurable)
7. ✅ Cellular adapter excluded by default
8. ✅ Rate limiting prevents spam
9. ✅ All tests pass
10. ✅ No panics or unwrap() failures

---

## 📚 Reference Documentation

**Design Specifications:**
- `/docs/design/heartbeat-protocol.md` - Complete protocol specification
- `/docs/design/message-acknowledgement-protocol.md` - Ack protocol (Phase 2)
- `/docs/design/account-identity-model.md` - Identity system (Phase 4)
- `/docs/design/bootstrap-trust-system.md` - Trust system (Phase 4)
- `/docs/design/emergency-protocol.md` - Emergency mode (Phase 5)

**Configuration:**
- `/crates/myriadnode/config.toml` - Reference configuration

**Existing Code:**
- `/crates/myriadnode/src/heartbeat.rs` - Current implementation (TODOs marked)
- `/crates/myriadnode/src/config.rs` - Configuration structs
- `/crates/myriadnode/src/node.rs` - Node orchestration

---

## 🚦 Next Session Checklist

When starting the next session:

1. [ ] Review this handoff document
2. [ ] Review `docs/design/heartbeat-protocol.md` sections 1-6
3. [ ] Open `crates/myriadnode/src/heartbeat.rs`
4. [ ] Start with updating HeartbeatService struct (add new fields)
5. [ ] Implement collect_adapter_info()
6. [ ] Implement sign_heartbeat()
7. [ ] Update handle_heartbeat() with verification
8. [ ] Implement broadcast loop (lines 169-172)
9. [ ] Add HeartbeatRateLimiter
10. [ ] Run tests: `cargo test --package myriadnode`
11. [ ] Commit and push

**Estimated Time:** 2-3 hours for Phase 1

---

## 💡 Implementation Tips

### Tip 1: Start with Signature Helpers
Implement signing/verification first, test them independently, then integrate.

### Tip 2: Mock AdapterManager for Tests
Create a simple mock AdapterManager that returns test adapters for unit tests.

### Tip 3: Add Logging
Use `tracing::debug!()` liberally to debug broadcast behavior.

### Tip 4: Test Each Function Independently
Don't try to implement everything at once. Test each method as you write it.

### Tip 5: Use the Design Docs
The heartbeat-protocol.md has pseudo-code and examples for most functions.

---

## 🔗 Important Links

**Repository:** (your repo URL)
**Branch:** `claude/phase-3-readiness-analysis-011CV4L3bSdqmcw9p9DD5KcS`
**Design Docs:** `/docs/design/`
**Config:** `/crates/myriadnode/config.toml`

---

## ✅ Session Summary

**What Was Done:**
- ✅ Comprehensive code review (identified 10 TODOs)
- ✅ Created 5 design documents (~4,000 lines)
- ✅ Created complete config.toml (500+ lines)
- ✅ Updated config.rs with per-adapter settings
- ✅ All changes committed and pushed

**What's Next:**
- ⏭️ Implement Phase 1: Core Heartbeat Broadcasting
- ⏭️ Fix 3 critical TODOs in heartbeat.rs
- ⏭️ Add comprehensive tests
- ⏭️ Verify everything works end-to-end

**Readiness:** 🟢 Ready to implement - all prerequisites complete

---

**End of Handoff Document**
