# MyriadMesh - Bug Fixes Action Plan

**Date:** 2025-11-16
**Branch:** `claude/code-analysis-bugs-01MhK1mANpMg5K52FKg7N5EJ`
**Priority:** CRITICAL

---

## Executive Summary

This action plan addresses 12 CRITICAL bugs and 27 HIGH priority issues discovered in the comprehensive code analysis. The plan is organized into 5 phases spanning approximately 3-4 months for complete resolution.

**Total Issues:** 78
- **CRITICAL:** 12 (must fix immediately)
- **HIGH:** 27 (fix within 2 sprints)
- **MEDIUM:** 24 (backlog)
- **LOW:** 15 (nice to have)

---

## PHASE 1: Critical Bug Fixes (Week 1-2)

### Goal: Fix bugs that cause crashes, panics, or complete system failure

#### Bug #1: Message::size() Calculation Error
**File:** `crates/myriadmesh-protocol/src/message.rs:316-317`
**Effort:** 30 minutes
**Risk:** Low

**Current Code:**
```rust
pub fn size(&self) -> usize {
    HEADER_SIZE + self.payload.len()
}

const HEADER_SIZE: usize = 4 + 1 + 1 + 1 + 1 + 1 + 2 + 16 + 32 + 32 + 8 + 64;
//                                                          ^^
//                                               Should use NODE_ID_SIZE (64)
```

**Fix:**
```rust
const HEADER_SIZE: usize = 4 + 1 + 1 + 1 + 1 + 1 + 2 + 16 +
                          NODE_ID_SIZE + NODE_ID_SIZE + 8 + SIGNATURE_SIZE;
// Or explicitly:
// = 4 + 1 + 1 + 1 + 1 + 1 + 2 + 16 + 64 + 64 + 8 + 64 = 227 bytes
```

**Testing:**
```rust
#[test]
fn test_header_size_calculation() {
    let expected = 4 + 1 + 1 + 1 + 1 + 1 + 2 + 16 + NODE_ID_SIZE + NODE_ID_SIZE + 8 + SIGNATURE_SIZE;
    assert_eq!(HEADER_SIZE, expected);
    assert_eq!(HEADER_SIZE, 227); // Explicit check
}
```

---

#### Bug #2: Blocking Sleep in Async Context
**File:** `crates/myriadmesh-network/src/i2p/embedded_router.rs:233-250, 308`
**Effort:** 1 hour
**Risk:** Low

**Current Code:**
```rust
pub fn wait_ready(&self, timeout: Duration) -> Result<()> {
    let start = Instant::now();
    while !self.ready.load(Ordering::SeqCst) {
        if start.elapsed() > timeout {
            return Err(I2pRouterError::TimeoutError(timeout));
        }
        std::thread::sleep(Duration::from_millis(500)); // BLOCKS RUNTIME
    }
    Ok(())
}
```

**Fix:**
```rust
pub async fn wait_ready(&self, timeout: Duration) -> Result<()> {
    let start = Instant::now();
    while !self.ready.load(Ordering::SeqCst) {
        if start.elapsed() > timeout {
            return Err(I2pRouterError::TimeoutError(timeout));
        }
        tokio::time::sleep(Duration::from_millis(500)).await; // ASYNC SLEEP
    }
    Ok(())
}
```

**Update callsite:**
```rust
// Line 308 in initialize()
router.wait_ready(Duration::from_secs(60)).await?; // Add .await
```

**Testing:**
- Run existing integration tests
- Add test to verify no runtime blocking

---

#### Bug #3: Float Comparison Panic on NaN
**File:** `crates/myriadmesh-routing/src/geographic.rs:136, 176`
**Effort:** 15 minutes
**Risk:** Low

**Current Code:**
```rust
distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap()); // Line 136
candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap()); // Line 176
```

**Fix:**
```rust
distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
```

**Testing:**
```rust
#[test]
fn test_nan_handling() {
    let mut table = GeoRoutingTable::new();
    // Add node with invalid coordinates that might produce NaN
    let node_id = NodeId::generate();
    table.add_location(node_id, GeoCoordinates { lat: f64::NAN, lon: 0.0 });
    // Should not panic
    let result = table.find_nodes_near(&GeoCoordinates { lat: 0.0, lon: 0.0 }, 5);
    assert!(result.is_ok());
}
```

---

#### Bug #4: I2P Onion Router Empty Vector Unwrap
**File:** `crates/myriadmesh-i2p/src/onion.rs:1017-1018`
**Effort:** 15 minutes
**Risk:** Low

**Current Code:**
```rust
let min_time = times_ms.iter().min().unwrap();
let max_time = times_ms.iter().max().unwrap();
```

**Fix:**
```rust
let min_time = times_ms.iter().min().ok_or(OnionError::NoBuildTimes)?;
let max_time = times_ms.iter().max().ok_or(OnionError::NoBuildTimes)?;

// Add error variant to OnionError:
#[derive(Debug, thiserror::Error)]
pub enum OnionError {
    // ... existing variants ...
    #[error("No tunnel build times available")]
    NoBuildTimes,
}
```

---

#### Bug #5: Blocking Multicast Setup in Async
**File:** `crates/myriadmesh-network/src/adapters/ethernet.rs:230-262, 439`
**Effort:** 2 hours
**Risk:** Medium

**Fix Option 1 - Convert to Async:**
```rust
async fn setup_multicast(&mut self) -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:4001")?;
    socket.join_multicast_v4(
        &"224.0.0.251".parse().unwrap(),
        &Ipv4Addr::UNSPECIFIED,
    )?;

    *self.multicast_socket.lock().await = Some(socket);
    Ok(())
}
```

**Fix Option 2 - Use spawn_blocking:**
```rust
async fn initialize(&mut self) -> Result<()> {
    // Run blocking setup in dedicated thread pool
    let socket = tokio::task::spawn_blocking(|| {
        let socket = UdpSocket::bind("0.0.0.0:4001")?;
        socket.join_multicast_v4(...)?;
        Ok::<_, Error>(socket)
    }).await??;

    *self.multicast_socket.lock().await = Some(socket);
    Ok(())
}
```

**Recommendation:** Use Option 2 (spawn_blocking) to keep socket setup logic synchronous

---

#### Bug #6: Database Pool Never Closed
**File:** `crates/myriadnode/src/storage.rs:101-104`
**Effort:** 15 minutes
**Risk:** Low

**Current Code:**
```rust
pub async fn close(&self) -> Result<()> {
    // TODO: Implement proper shutdown
    Ok(())
}
```

**Fix:**
```rust
pub async fn close(&self) -> Result<()> {
    self.pool.close().await;
    Ok(())
}
```

**Testing:**
```rust
#[tokio::test]
async fn test_storage_close() {
    let storage = NodeStorage::new("sqlite::memory:").await.unwrap();
    storage.close().await.unwrap();

    // Verify connections are closed (pool should reject new ops)
    let result = storage.get_node_info().await;
    assert!(result.is_err());
}
```

---

## PHASE 2: Implement Core Routing Logic (Week 3-5)

### Goal: Make the router actually route messages

#### Bug #7: Router Has No Routing Logic
**File:** `crates/myriadmesh-routing/src/router.rs:376-387`
**Effort:** 2-3 weeks
**Risk:** High - Major feature implementation

**Current Code:**
```rust
async fn forward_message(&self, message: Message) -> Result<(), RoutingError> {
    // TODO: Check if destination is reachable
    // For now, we'll try to send and cache on failure

    let mut queue = self.outbound_queue.write().await;
    queue.enqueue(message)
        .map_err(|e| RoutingError::QueueFull(e.to_string()))?;
    Ok(())
}
```

**Implementation Plan:**

**Step 1: Add DHT Integration (3 days)**
```rust
async fn forward_message(&self, mut message: Message) -> Result<(), RoutingError> {
    // 1. Decrement TTL
    message.ttl = message.ttl.saturating_sub(1);
    if message.ttl == 0 {
        return Err(RoutingError::TtlExceeded);
    }

    // 2. Query DHT for destination
    let dest_info = self.dht.find_node(&message.destination).await?;

    // 3. Check if destination is reachable
    if !dest_info.is_reachable() {
        // Cache for offline delivery
        self.offline_cache.cache_for_offline(message.destination, message).await?;
        return Ok(());
    }

    // ... continue to step 2
}
```

**Step 2: Add Path Selection (5 days)**
```rust
    // 4. Determine next hop using routing strategy
    let next_hop = self.select_next_hop(&message, &dest_info).await?;

    // 5. Select best adapter based on priority and metrics
    let adapter = self.select_adapter(&next_hop, message.priority).await?;

    // 6. Queue for transmission
    let mut queue = self.outbound_queue.write().await;
    queue.enqueue(message)?;

    Ok(())
}

async fn select_next_hop(
    &self,
    message: &Message,
    dest_info: &NodeInfo,
) -> Result<NodeId, RoutingError> {
    // Priority-based routing strategy selection
    match message.priority {
        Priority::Emergency | Priority::High => {
            // Use multipath for high priority
            self.multipath_router.get_best_path(&message.destination).await
        }
        Priority::Normal => {
            // Use geographic if location available
            if let Some(location) = dest_info.location {
                self.geo_router.greedy_next_hop(&location).await
            } else {
                // Fallback to DHT routing
                Ok(dest_info.node_id)
            }
        }
        Priority::Low | Priority::Background => {
            // Use adaptive routing for efficiency
            self.adaptive_router.get_path(&message.destination).await
        }
    }
}
```

**Step 3: Integrate Weighted Tier System (4 days)**
```rust
async fn select_adapter(
    &self,
    next_hop: &NodeId,
    priority: Priority,
) -> Result<AdapterId, RoutingError> {
    // Get adapter scores from performance monitor
    let adapters = self.performance_monitor.get_adapter_rankings(next_hop).await?;

    // Apply priority-based weighting
    let weighted_scores = adapters.iter().map(|(adapter, score)| {
        let weight = match priority {
            Priority::Emergency => score.latency_weight * 0.8 + score.reliability_weight * 0.2,
            Priority::High => score.latency_weight * 0.6 + score.reliability_weight * 0.4,
            Priority::Normal => score.balanced_weight,
            Priority::Low => score.bandwidth_weight * 0.6 + score.cost_weight * 0.4,
            Priority::Background => score.cost_weight * 0.8 + score.power_weight * 0.2,
        };
        (adapter.clone(), weight)
    }).collect::<Vec<_>>();

    // Select adapter with highest weighted score
    weighted_scores.iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
        .map(|(adapter, _)| adapter.clone())
        .ok_or(RoutingError::NoAvailableAdapter)
}
```

**Step 4: Add Retry Logic (3 days)**
```rust
// Modify QueuedMessage usage
impl Router {
    async fn process_outbound_queue(&self) -> Result<()> {
        loop {
            let mut queue = self.outbound_queue.write().await;

            if let Some(mut queued) = queue.dequeue()? {
                drop(queue); // Release lock

                // Attempt delivery
                match self.send_via_adapter(&queued.message).await {
                    Ok(_) => {
                        // Success - record in stats
                        self.stats.write().await.messages_sent += 1;
                    }
                    Err(e) => {
                        // Failure - check retry
                        queued.retry_count += 1;
                        if queued.retry_count < MAX_RETRIES {
                            // Exponential backoff
                            let backoff = Duration::from_secs(2_u64.pow(queued.retry_count));
                            queued.next_retry = Some(current_time() + backoff.as_secs());

                            // Re-queue
                            self.outbound_queue.write().await.enqueue_with_retry(queued)?;
                        } else {
                            // Max retries exceeded - cache offline
                            self.offline_cache.cache_for_offline(
                                queued.message.destination,
                                queued.message
                            ).await?;
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}
```

**Testing Strategy:**
1. Unit tests for each routing strategy
2. Integration tests for path selection
3. End-to-end tests for message delivery
4. Failure scenario tests (offline nodes, adapter failures)
5. Performance tests under load

---

#### Bug #8: TTL Never Decremented
**Effort:** 30 minutes (included in Bug #7)
**Note:** Already included in forward_message implementation above

---

#### Bug #9: Retry/Failover Logic Unused
**Effort:** 3 days (included in Bug #7)
**Note:** Already included in queue processing implementation above

---

#### Bug #10: Advanced Routing Modules Never Used
**Effort:** 1 week (included in Bug #7)
**Note:** Already included in select_next_hop implementation above

---

## PHASE 3: Resource Management Fixes (Week 6-7)

### Goal: Fix memory leaks, socket leaks, and resource exhaustion

#### Bug #11: JNI Raw Pointer Management
**File:** `crates/myriadmesh-android/src/lib.rs`
**Effort:** 1 week
**Risk:** High - Requires careful testing

**Current Pattern:**
```rust
#[no_mangle]
pub unsafe extern "C" fn Java_..._nativeInit(...) -> jlong {
    Box::into_raw(Box::new(node)) as jlong  // LEAK!
}
```

**Fix - Use Global Handle Registry:**
```rust
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use once_cell::sync::Lazy;

static ANDROID_NODES: Lazy<Mutex<HashMap<u64, Arc<Mutex<AndroidNode>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeInit(
    env: JNIEnv,
    _class: JClass,
    config_json: JString,
) -> jlong {
    let config_str: String = env.get_string(config_json)
        .expect("Failed to get config string")
        .into();

    let config: AndroidConfig = serde_json::from_str(&config_str)
        .expect("Failed to parse config");

    match AndroidNode::new(config) {
        Ok(node) => {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::SeqCst);
            ANDROID_NODES.lock().unwrap().insert(handle, Arc::new(Mutex::new(node)));
            handle as jlong
        }
        Err(e) => {
            // Log error
            0 // Return 0 to indicate failure
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeStart(
    _env: JNIEnv,
    _obj: JObject,
    handle: jlong,
) -> jboolean {
    let nodes = ANDROID_NODES.lock().unwrap();

    if let Some(node_arc) = nodes.get(&(handle as u64)) {
        let mut node = node_arc.lock().unwrap();
        match node.start() {
            Ok(_) => JNI_TRUE,
            Err(_) => JNI_FALSE,
        }
    } else {
        JNI_FALSE // Invalid handle
    }
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeDestroy(
    _env: JNIEnv,
    _obj: JObject,
    handle: jlong,
) {
    ANDROID_NODES.lock().unwrap().remove(&(handle as u64));
    // Arc will be dropped, node cleaned up automatically
}
```

**Testing:**
1. Test handle creation
2. Test handle reuse after destroy
3. Test invalid handle rejection
4. Test memory leak with repeated init/destroy
5. Test concurrent access from multiple threads

---

#### Bug #12-24: Unbounded Channels
**Files:** 13 adapter files
**Effort:** 2 days
**Risk:** Low

**Pattern to Replace:**
```rust
let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
```

**With:**
```rust
let (incoming_tx, incoming_rx) = mpsc::channel(1000); // bounded

// Or for high-throughput adapters:
let (incoming_tx, incoming_rx) = mpsc::channel(10000);
```

**Capacity Guidelines:**
- Ethernet/Wi-Fi: 10,000 (high throughput)
- Cellular: 5,000 (medium throughput)
- LoRa/Radio: 1,000 (low throughput)
- BLE/Bluetooth: 500 (very low throughput)

**Handle Backpressure:**
```rust
// In adapter send loops:
match incoming_tx.try_send((addr, frame)) {
    Ok(_) => {}
    Err(TrySendError::Full(_)) => {
        // Channel full - backpressure
        warn!("Incoming channel full, dropping frame");
        stats.dropped_frames += 1;
    }
    Err(TrySendError::Closed(_)) => {
        // Channel closed - shutdown
        break;
    }
}
```

---

#### Bug #25-27: Task Handle Leaks
**Files:** `events.rs`, `node.rs`, `monitor.rs`, `failover.rs`, `heartbeat.rs`
**Effort:** 1 week
**Risk:** Medium

**Pattern:**

1. Add shutdown channel to struct
2. Store JoinHandles
3. Implement graceful shutdown

**Example for TUI Events:**
```rust
pub struct EventHandler {
    event_tx: mpsc::UnboundedSender<Event>,
    tick_tx: mpsc::UnboundedSender<Event>,
    shutdown_tx: broadcast::Sender<()>,  // NEW
    event_task: Option<JoinHandle<()>>,  // NEW
    tick_task: Option<JoinHandle<()>>,   // NEW
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> (Self, mpsc::UnboundedReceiver<Event>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (tick_tx, mut tick_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, _) = broadcast::channel(1);

        // Event task with shutdown
        let mut shutdown_rx1 = shutdown_tx.subscribe();
        let event_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx1.recv() => break,
                    result = tokio::task::spawn_blocking(|| {
                        event::poll(Duration::from_millis(100))
                    }) => {
                        if let Ok(Ok(true)) = result {
                            // Process event
                        }
                    }
                }
            }
        });

        // Tick task with shutdown
        let mut shutdown_rx2 = shutdown_tx.subscribe();
        let tick_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                tokio::select! {
                    _ = shutdown_rx2.recv() => break,
                    _ = interval.tick() => {
                        let _ = tick_tx.send(Event::Tick);
                    }
                }
            }
        });

        (
            Self {
                event_tx,
                tick_tx,
                shutdown_tx,
                event_task: Some(event_task),
                tick_task: Some(tick_task),
            },
            event_rx,
        )
    }

    pub async fn shutdown(mut self) {
        // Signal shutdown
        let _ = self.shutdown_tx.send(());

        // Wait for tasks
        if let Some(task) = self.event_task.take() {
            let _ = task.await;
        }
        if let Some(task) = self.tick_task.take() {
            let _ = task.await;
        }
    }
}
```

**Apply this pattern to:**
- TUI events (2 tasks)
- Monitor (3 tasks)
- Failover manager (1 task)
- Heartbeat service (2 tasks)
- Pairing manager (1 task)

---

## PHASE 4: Medium Priority Fixes (Week 8-10)

### Concurrency Issues

#### Lock Ordering Documentation
**File:** `crates/myriadnode/src/failover.rs`
**Effort:** 2 days

Add documentation and enforce ordering:
```rust
// Lock acquisition order (MUST follow this order to prevent deadlock):
// 1. adapters (RwLock)
// 2. current_primary (RwLock)
// 3. failover_history (RwLock)
// 4. config (RwLock)

impl FailoverManager {
    async fn check_and_failover(&self) -> Result<()> {
        // Always acquire in order:
        let adapters = self.adapters.read().await;          // Lock 1
        let current = self.current_primary.read().await;     // Lock 2

        // ... logic ...

        drop(current);  // Release in reverse order
        drop(adapters);

        // If need to write:
        let mut history = self.failover_history.write().await; // Lock 3
        history.push(...);
    }
}
```

#### Cache TOCTOU Race Fix
**File:** `crates/myriadmesh-appliance/src/cache.rs`
**Effort:** 1 day

```rust
async fn store(&self, msg: CachedMessage) -> Result<()> {
    // Single atomic operation
    let mut cache = self.messages.write().await;

    // Check and insert atomically
    let current_size = cache.values().map(|v| v.len()).sum::<usize>();
    let msg_size = msg.size();

    if current_size + msg_size > self.config.max_cache_size_bytes {
        self.evict_messages_locked(&mut cache).await?;
    }

    cache.entry(msg.destination).or_insert_with(Vec::new).push(msg);
    Ok(())
}
```

#### License Cache LRU Eviction
**File:** `crates/myriadmesh-network/src/license.rs`
**Effort:** 1 day

```rust
use lru::LruCache;

pub struct LicenseManager {
    cache: Arc<RwLock<LruCache<String, (bool, Instant)>>>,
    // ... other fields
}

impl LicenseManager {
    pub fn new(config: LicenseConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(1000))),
            config,
        }
    }

    pub async fn validate_callsign(&self, callsign: &str) -> Result<bool> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some((is_valid, cached_at)) = cache.peek(callsign) {
                if cached_at.elapsed() < Duration::from_secs(3600) {
                    return Ok(*is_valid);
                }
            }
        }

        // Not in cache or expired - validate
        let is_valid = self.check_fcc_database(callsign).await?;

        // Update cache (LRU handles eviction automatically)
        let mut cache = self.cache.write().await;
        cache.put(callsign.to_string(), (is_valid, Instant::now()));

        Ok(is_valid)
    }
}
```

---

## PHASE 5: Documentation & Testing (Week 11-12)

### Update Specifications
1. Protocol spec: Update Node ID size to 64 bytes
2. Protocol spec: Update header size to 227 bytes
3. Protocol spec: Update default TTL to 32 or revert implementation to 10
4. Crypto spec: Document 24-hour key rotation decision
5. Routing spec: Document weighted tier system algorithm

### Add Integration Tests
1. End-to-end routing tests
2. Failover scenario tests
3. Graceful shutdown tests
4. Resource cleanup tests
5. JNI lifecycle tests

### Security Audit
1. Re-audit after all fixes
2. Verify no new vulnerabilities introduced
3. Update security documentation

---

## EFFORT SUMMARY

| Phase | Duration | Team Size | Issues Fixed |
|-------|----------|-----------|--------------|
| Phase 1 | 2 weeks | 2 devs | 6 CRITICAL |
| Phase 2 | 3 weeks | 3 devs | 4 CRITICAL + routing |
| Phase 3 | 2 weeks | 2 devs | 2 CRITICAL + 13 HIGH |
| Phase 4 | 3 weeks | 2 devs | 3 MEDIUM + features |
| Phase 5 | 2 weeks | 1 dev | Testing + docs |
| **TOTAL** | **12 weeks** | **2-3 devs** | **78 issues** |

---

## RISK MITIGATION

### High-Risk Items
1. **Routing Implementation (Bug #7)**
   - Risk: Complex feature, potential architecture changes
   - Mitigation: Incremental implementation with feature flags
   - Fallback: Implement simple DHT-only routing first

2. **JNI Pointer Management (Bug #11)**
   - Risk: Android compatibility, testing challenges
   - Mitigation: Extensive testing on multiple Android versions
   - Fallback: Keep old implementation behind feature flag

### Testing Strategy
1. Run full test suite after each fix
2. Add regression tests for each bug
3. Performance testing after routing implementation
4. Memory leak testing with valgrind/heaptrack
5. Android integration testing on real devices

---

## SUCCESS CRITERIA

### Phase 1 Complete
- [ ] All CRITICAL panics fixed
- [ ] No blocking operations in async context
- [ ] All protocol tests passing
- [ ] Database resources properly cleaned up

### Phase 2 Complete
- [ ] Router can forward messages
- [ ] TTL properly decremented
- [ ] Retry logic functional
- [ ] At least one routing strategy working (DHT-based)

### Phase 3 Complete
- [ ] No memory leaks (valgrind clean)
- [ ] No file descriptor leaks
- [ ] Graceful shutdown implemented
- [ ] All channels bounded

### Phase 4 Complete
- [ ] No known race conditions
- [ ] Lock ordering documented
- [ ] Caches have proper eviction

### Phase 5 Complete
- [ ] All tests passing
- [ ] Documentation updated
- [ ] Security audit complete
- [ ] Production ready

---

## TRACKING

Create GitHub issues for each bug:
- Tag with priority (P0-P3)
- Assign to phases
- Track in project board
- Link to this action plan

**Branch:** `claude/code-analysis-bugs-01MhK1mANpMg5K52FKg7N5EJ`
**Next:** Create detailed tickets and begin Phase 1
