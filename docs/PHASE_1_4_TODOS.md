# Phase 1-4 TODO Tracking

**Generated:** 2025-11-15
**Purpose:** Track remaining work from Phase 1-4 before starting Phase 5

---

## Critical Priority (Must-Do Before Phase 5)

### 1. Blockchain Ledger Integration 🔴
**Estimated Effort:** 1-2 days
**Blocker:** Yes - needed for Phase 5 discovery/test entries

**Tasks:**
- [x] Add `myriadmesh-ledger` dependency to `myriadnode/Cargo.toml`
- [x] Initialize ledger in `myriadnode/src/node.rs` startup
- [x] Add API endpoints in `myriadnode/src/api.rs`:
  - `GET /api/ledger/blocks` - List recent blocks
  - `GET /api/ledger/blocks/:height` - Get specific block
  - `GET /api/ledger/entries` - Query entries by type
  - `POST /api/ledger/entry` - Submit new entry
- [x] Wire ledger into message routing for confirmations
- [ ] Test multi-node ledger consensus

**Files:**
- `crates/myriadnode/Cargo.toml`
- `crates/myriadnode/src/node.rs`
- `crates/myriadnode/src/api.rs`

---

### 2. DHT Iterative Lookups 🔴
**Estimated Effort:** 2-3 days
**Blocker:** Yes - needed for Phase 5 peer discovery

**Tasks:**
- [ ] Implement `iterative_find_node()` in `operations.rs`
- [ ] Implement `iterative_find_value()` in `operations.rs`
- [ ] Add DHT RPC request handler
- [ ] Integrate with myriadnode API
- [ ] Add unit tests for lookup algorithms
- [ ] Test with multi-node network

**Files:**
- `crates/myriadmesh-dht/src/operations.rs`
- `crates/myriadnode/src/api.rs` (add DHT query endpoints)

---

## High Priority (Recommended for Phase 5)

### 3. Store-and-Forward Message Caching 🟡
**Estimated Effort:** 1-2 days
**Important for:** Radio networks with intermittent connectivity

**Tasks:**
- [ ] Create `crates/myriadmesh-routing/src/offline_cache.rs`
- [ ] Implement `OfflineMessageCache` with TTL and priority
- [ ] Add queue management (capacity limits per destination)
- [ ] Integrate with router.rs forwarding logic
- [ ] Add delivery on node reconnection
- [ ] Add unit tests
- [ ] Add cache stats to API

**Files:**
- `crates/myriadmesh-routing/src/offline_cache.rs` (new)
- `crates/myriadmesh-routing/src/router.rs`
- `crates/myriadnode/src/api.rs`

---

## Medium Priority (Quick Wins)

### 4. Metrics Persistence ⚠️ QUICK WIN
**Estimated Effort:** 3-4 hours
**Location:** `crates/myriadnode/src/monitor.rs`

**TODOs:**
- [ ] Line 136: Store ping metrics in database
- [ ] Lines 169-170: Perform actual throughput test, store metrics
- [ ] Lines 202-203: Perform packet loss test, store metrics

**Implementation:**
```rust
// Add to monitor.rs:
async fn store_metrics(&self, adapter_id: &str, metrics: &Metrics) -> Result<()> {
    let conn = self.storage.lock().await;
    conn.execute(
        "INSERT INTO metrics (adapter_id, timestamp, latency_ms, throughput_mbps, packet_loss, success_rate) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![adapter_id, timestamp, latency, throughput, packet_loss, success_rate]
    )?;
    Ok(())
}
```

---

### 5. Message API Endpoints ⚠️ QUICK WIN
**Estimated Effort:** 4-6 hours
**Location:** `crates/myriadnode/src/api.rs`

**TODOs:**
- [ ] Line 242: Implement `send_message`
- [ ] Line 264: Implement `list_messages`
- [ ] Line 205: Track messages_queued count

**Implementation:**
```rust
// POST /api/messages/send
async fn send_message(
    State(node): State<Arc<Node>>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, (StatusCode, String)> {
    let frame = node.create_message_frame(request)?;
    node.router.send_message(frame).await?;
    Ok(Json(SendMessageResponse { message_id }))
}

// GET /api/messages
async fn list_messages(
    State(node): State<Arc<Node>>,
) -> Result<Json<Vec<Message>>, (StatusCode, String)> {
    let messages = node.storage.get_messages().await?;
    Ok(Json(messages))
}
```

---

### 6. Publisher Signature Verification ⚠️ QUICK WIN
**Estimated Effort:** 1-2 hours
**Location:** `crates/myriadmesh-updates/src/verification.rs:181`

**TODO:**
- [ ] Verify publisher signature separately from peer signatures

**Implementation:**
```rust
pub fn verify_publisher_signature(&self, package: &UpdatePackage) -> Result<bool> {
    if package.signature_chain.is_empty() {
        return Ok(false);
    }

    let publisher_sig = &package.signature_chain[0];
    // Verify it's from a known publisher public key
    verify_signature(
        &self.publisher_public_key,
        &package.payload_hash,
        &publisher_sig.signature
    )
}
```

---

### 7. i2p Padding Detection ⚠️ QUICK WIN
**Estimated Effort:** 1 hour
**Location:** `crates/myriadmesh-i2p/src/privacy.rs:176`

**TODO:**
- [ ] Implement proper padding detection based on strategy

**Implementation:**
```rust
pub fn remove_padding(data: &[u8], strategy: &PaddingStrategy) -> Result<Vec<u8>> {
    match strategy {
        PaddingStrategy::ISO7816_4 => {
            // Find last 0x80 byte
            if let Some(pos) = data.iter().rposition(|&b| b == 0x80) {
                Ok(data[..pos].to_vec())
            } else {
                Err(Error::InvalidPadding)
            }
        },
        PaddingStrategy::RFC2630 => {
            // Last byte indicates padding length
            let pad_len = *data.last().ok_or(Error::InvalidPadding)? as usize;
            Ok(data[..data.len() - pad_len].to_vec())
        },
        // ... other strategies
    }
}
```

---

### 8. Local Message Delivery ⚠️ QUICK WIN
**Estimated Effort:** 2-3 hours
**Location:** `crates/myriadmesh-routing/src/router.rs:289`

**TODO:**
- [ ] Implement local delivery integration

**Implementation:**
```rust
async fn deliver_local(&self, frame: Frame) -> Result<()> {
    // Store in local message queue
    self.storage.store_message(&frame).await?;

    // Notify application layer via channel
    if let Some(tx) = &self.local_delivery_channel {
        tx.send(frame).await?;
    }

    // Update metrics
    self.metrics.increment_delivered();

    Ok(())
}
```

---

### 9. Adapter Start/Stop Endpoints ⚠️ QUICK WIN
**Estimated Effort:** 2 hours
**Location:** `crates/myriadnode/src/api.rs`

**TODOs:**
- [ ] Line 309: Implement start_adapter
- [ ] Line 317: Implement stop_adapter

**Implementation:**
```rust
// POST /api/adapters/:id/start
async fn start_adapter(
    State(node): State<Arc<Node>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    node.network_manager.start_adapter(&id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

// POST /api/adapters/:id/stop
async fn stop_adapter(
    State(node): State<Arc<Node>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    node.network_manager.stop_adapter(&id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}
```

---

### 10. Configuration API Endpoints
**Estimated Effort:** 3 hours
**Location:** `crates/myriadnode/src/api.rs`

**TODOs:**
- [ ] Line 550: Return actual config instead of placeholder
- [ ] Line 571: Implement update_config

---

### 11. Heartbeat Enhancements
**Estimated Effort:** 4 hours
**Location:** `crates/myriadnode/src/heartbeat.rs`

**TODOs:**
- [ ] Line 409: Implement geolocation collection
- [ ] Line 419: Broadcast via all eligible adapters
- [ ] Lines 426-427: Track RTT and failure count

---

### 12. i2p Integration Enhancements
**Estimated Effort:** 2 hours
**Location:** `crates/myriadnode/src/api.rs`

**TODOs:**
- [ ] Lines 615-616: Get actual i2p tunnel/peer counts
- [ ] Line 635: Get i2p destination from adapter
- [ ] Line 655: Get i2p tunnel information

---

## Low Priority (Larger Efforts)

### 13. Network Adapter Platform Integration
**Estimated Effort:** 1-2 weeks
**Not blocking Phase 5**

**Bluetooth Classic** (`crates/myriadmesh-network/src/adapters/bluetooth.rs`):
- [ ] SDP service discovery
- [ ] bluez integration (Linux)
- [ ] CoreBluetooth integration (macOS/iOS)
- [ ] Actual RFCOMM socket implementation

**Bluetooth Low Energy** (`crates/myriadmesh-network/src/adapters/bluetooth_le.rs`):
- [ ] Platform BLE stack integration
- [ ] BlueZ GATT implementation
- [ ] CoreBluetooth implementation
- [ ] WinRT implementation

**Cellular** (`crates/myriadmesh-network/src/adapters/cellular.rs`):
- [ ] ModemManager integration (Linux)
- [ ] AT command implementation
- [ ] Network type detection
- [ ] Cost tracking integration

---

### 14. Android JNI Bridge Implementation
**Estimated Effort:** 3-4 weeks
**Not blocking Phase 5**

**Location:** `crates/myriadmesh-android/src/node.rs`

**TODOs (6 total):**
- [ ] Line 12: Add actual MyriadNode instance
- [ ] Line 42: Initialize and start MyriadNode
- [ ] Line 59: Stop MyriadNode
- [ ] Line 79: Send message through MyriadNode
- [ ] Line 88: Get actual node ID
- [ ] Line 95: Get actual status

**Dependencies:**
- Requires physical appliance for testing
- Can continue in parallel with Phase 5

---

### 15. Adapter Reload Enhancements
**Estimated Effort:** 4 hours
**Location:** `crates/myriadmesh-network/src/reload.rs`

**TODOs:**
- [ ] Line 453: Implement binary preservation for rollback
- [ ] Lines 467-470: Clean up preserved binaries
- [ ] Lines 531-535: Binary cleanup implementation

---

## Testing TODOs

### Unit Tests Needed:
- [ ] API endpoint tests (myriadnode/src/api.rs)
- [ ] WebSocket connection tests
- [ ] Authentication/authorization tests
- [ ] Storage/database migration tests
- [ ] DHT lookup algorithm tests
- [ ] Store-and-forward cache tests

### Integration Tests Needed:
- [ ] DHT + routing + network end-to-end
- [ ] Multi-node ledger consensus
- [ ] Message delivery across adapters
- [ ] Failover scenarios

### Other Testing:
- [ ] Measure code coverage with `cargo tarpaulin`
- [ ] Create test vectors for crypto operations
- [ ] Property-based testing with `proptest`
- [ ] Performance benchmarks

---

## Documentation TODOs

- [ ] Add test vector files for crypto
- [ ] Create adapter development guide
- [ ] Add more API usage examples
- [ ] Write deployment guide
- [ ] Create troubleshooting guide

---

## Priority Order for Implementation

**Quick Wins (Start Here - 1-2 days total):**
1. ✅ Metrics persistence (3-4 hours)
2. ✅ Publisher signature verification (1-2 hours)
3. ✅ i2p padding detection (1 hour)
4. ✅ Local message delivery (2-3 hours)
5. ✅ Adapter start/stop endpoints (2 hours)
6. ✅ Message API endpoints (4-6 hours)

**Critical for Phase 5 (3-5 days):**
7. ⏳ Store-and-forward caching (1-2 days)
8. ⏳ DHT iterative lookups (2-3 days)
9. ⏳ Ledger integration (1-2 days)

**Nice to Have:**
10. Configuration API endpoints
11. Heartbeat enhancements
12. i2p integration enhancements
13. Testing improvements

**Future Work:**
14. Network adapter platform integration
15. Android JNI bridge
16. Adapter reload enhancements

---

## Pre-Submission Checklist (CONTRIBUTING.md)

Before committing changes, run:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Clippy on entire workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Run all tests
cargo test --workspace --all-features

# 4. Build all targets
cargo build --workspace --all-targets --all-features
```

---

## Notes

- All TODOs tracked in this document correspond to actual code comments
- Priority based on Phase 5 readiness and implementation speed
- Items 1-6 are quick wins that provide immediate value
- Items 7-9 are critical for Phase 5 success
- Items 14-15 can be done in parallel with Phase 5 work
