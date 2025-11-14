# Android-Appliance Architecture Design
## Phase 4.5: Android Application with Hardware Appliance Integration

**Version:** 1.0
**Date:** 2025-11-14
**Status:** Design Phase

---

## Executive Summary

This document describes the architecture for MyriadMesh's Android application with deep integration to hardware appliance nodes. The design enables Android devices to leverage home/office appliance nodes as persistent gateways, providing message caching, routing preferences, and network persistence when mobile devices are offline or power-constrained.

### Key Features
- **Appliance Discovery**: Automatic discovery of nearby appliance nodes via mDNS and DHT
- **Secure Pairing**: QR code-based pairing with mutual authentication
- **Message Caching**: Appliance stores messages for mobile devices when offline
- **Priority Configuration**: Mobile app configures routing priorities and QoS preferences
- **Seamless Handoff**: Automatic routing through appliance when mobile adapters are unavailable
- **Battery Optimization**: Appliance handles heavy mesh operations to preserve mobile battery

---

## 1. System Architecture

### 1.1 Component Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Android Application                      │
│  ┌────────────┐  ┌─────────────┐  ┌───────────────────┐    │
│  │ UI Layer   │  │ Service     │  │ MyriadNode Core   │    │
│  │ (Kotlin)   │←→│ (Kotlin)    │←→│ (Rust via JNI)    │    │
│  └────────────┘  └─────────────┘  └───────────────────┘    │
│         │              │                     │               │
│         └──────────────┴─────────────────────┘               │
│                        │                                     │
└────────────────────────┼─────────────────────────────────────┘
                         │ Appliance Protocol
                         │ (WebSocket + REST API)
                         │
┌────────────────────────┼─────────────────────────────────────┐
│                        ▼                                     │
│              Hardware Appliance Node                         │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  MyriadNode (Appliance Mode)                         │   │
│  │  ┌─────────────┐  ┌──────────────┐  ┌────────────┐  │   │
│  │  │ Appliance   │  │ Message      │  │ Config     │  │   │
│  │  │ Manager     │  │ Cache        │  │ Sync       │  │   │
│  │  └─────────────┘  └──────────────┘  └────────────┘  │   │
│  │  ┌──────────────────────────────────────────────┐   │   │
│  │  │ Standard MyriadNode Components               │   │   │
│  │  │ (DHT, Routing, Adapters, Ledger, etc.)       │   │   │
│  │  └──────────────────────────────────────────────┘   │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 1.2 Appliance Node Roles

An appliance node operates in a special "appliance mode" providing:

1. **Gateway Services**: Always-on mesh connectivity for mobile devices
2. **Message Store-and-Forward**: Caches messages for paired mobile devices
3. **Routing Proxy**: Routes messages on behalf of mobile devices
4. **Configuration Persistence**: Stores and syncs mobile device preferences
5. **Network Aggregation**: Combines multiple network adapters for reliability

### 1.3 Mobile Device Roles

The Android application provides:

1. **User Interface**: Native Android UI for messaging and configuration
2. **Local Mesh**: Direct mesh connectivity via WiFi Direct, Bluetooth
3. **Appliance Management**: Discovery, pairing, and configuration of appliances
4. **Offline Operation**: Functions independently when no appliance available
5. **Power Management**: Offloads heavy operations to appliance

---

## 2. Appliance Discovery Protocol

### 2.1 Discovery Mechanisms

#### 2.1.1 Local Network Discovery (mDNS/DNS-SD)

**Service Type**: `_myriadmesh-appliance._tcp.local`

**TXT Records**:
```
node_id=<hex-encoded-node-id>
version=1.0.0
capabilities=cache,relay,bridge
pairing_available=true
max_paired_devices=10
current_paired=3
```

**Discovery Flow**:
```
Android App                     Appliance Node
    │                                 │
    │──── mDNS Query ────────────────→│
    │     _myriadmesh-appliance._tcp  │
    │                                 │
    │←─── mDNS Response ──────────────│
    │     TXT: node_id, capabilities  │
    │                                 │
    │──── HTTP GET /api/appliance/info│
    │                                 │
    │←─── JSON: Full capabilities ────│
    │                                 │
```

#### 2.1.2 DHT-Based Discovery

For discovering appliances across the internet:

1. Appliance publishes availability to DHT with key: `appliance:<node_id>`
2. Android app queries DHT for known appliance node IDs
3. Establishes connection via i2p or direct IP

### 2.2 Appliance Information API

**Endpoint**: `GET /api/appliance/info`

**Response**:
```json
{
  "node_id": "a1b2c3d4...",
  "node_name": "home-gateway",
  "version": "1.0.0",
  "appliance_mode": true,
  "capabilities": {
    "message_caching": true,
    "relay": true,
    "bridge": true,
    "max_cache_messages": 10000,
    "max_paired_devices": 10
  },
  "pairing": {
    "available": true,
    "methods": ["qr_code", "pin"],
    "requires_approval": true
  },
  "stats": {
    "uptime_secs": 864000,
    "paired_devices": 3,
    "cached_messages": 42,
    "adapters_online": 3
  }
}
```

---

## 3. Pairing Protocol

### 3.1 QR Code Pairing (Recommended)

**Flow**:
```
Android App                     Appliance Node
    │                                 │
    │──── POST /api/appliance/pair/request
    │     { device_id, public_key }   │
    │                                 │
    │←─── 201 Created ────────────────│
    │     { pairing_token, challenge }│
    │                                 │
    │     [User scans QR code         │
    │      displayed on appliance]    │
    │                                 │
    │──── POST /api/appliance/pair/complete
    │     { pairing_token, signature }│
    │                                 │
    │←─── 200 OK ─────────────────────│
    │     { session_token, config }   │
    │                                 │
```

**QR Code Contents**:
```json
{
  "pairing_token": "abc123...",
  "node_id": "a1b2c3d4...",
  "timestamp": 1700000000,
  "signature": "sig..."
}
```

### 3.2 Security Model

1. **Mutual Authentication**: Both devices verify each other's identity using Ed25519 signatures
2. **Session Tokens**: JWT tokens for ongoing API authentication
3. **Token Rotation**: Sessions expire after 30 days, require re-pairing
4. **Revocation**: Either device can revoke pairing at any time

### 3.3 Pairing Storage

**Appliance Side** (`appliance_pairings.json`):
```json
{
  "pairings": [
    {
      "device_id": "mobile-abc123",
      "device_node_id": "m1n2o3...",
      "public_key": "pubkey...",
      "paired_at": 1700000000,
      "last_seen": 1700086400,
      "session_token_hash": "hash...",
      "preferences": {
        "message_caching": true,
        "cache_priority": "high",
        "routing_preference": "privacy"
      }
    }
  ]
}
```

---

## 4. Message Caching System

### 4.1 Cache Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Appliance Message Cache                    │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐  │
│  │ Priority     │  │ Normal       │  │ Low         │  │
│  │ Queue        │  │ Queue        │  │ Queue       │  │
│  │ (1000 msgs)  │  │ (5000 msgs)  │  │ (4000 msgs) │  │
│  └──────────────┘  └──────────────┘  └─────────────┘  │
│         │                 │                  │         │
│         └─────────────────┴──────────────────┘         │
│                           │                            │
│                    ┌──────▼──────┐                     │
│                    │ LRU Evictor │                     │
│                    └─────────────┘                     │
│                                                         │
│  Storage: SQLite database                              │
│  Location: {data_dir}/appliance/cache.db               │
└─────────────────────────────────────────────────────────┘
```

### 4.2 Cache Database Schema

```sql
CREATE TABLE message_cache (
    message_id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,           -- Paired device this message is for
    direction TEXT NOT NULL,           -- 'inbound' or 'outbound'
    priority INTEGER NOT NULL,         -- 0=low, 1=normal, 2=high, 3=urgent
    payload BLOB NOT NULL,
    metadata TEXT,                     -- JSON metadata
    received_at INTEGER NOT NULL,      -- Unix timestamp
    expires_at INTEGER NOT NULL,       -- Unix timestamp
    delivered BOOLEAN DEFAULT FALSE,
    delivery_attempts INTEGER DEFAULT 0,
    FOREIGN KEY (device_id) REFERENCES paired_devices(device_id)
);

CREATE INDEX idx_device_priority ON message_cache(device_id, priority DESC, received_at);
CREATE INDEX idx_expires ON message_cache(expires_at);
CREATE INDEX idx_delivery ON message_cache(device_id, delivered, priority);
```

### 4.3 Cache Management Policies

**Priority Levels**:
- **Urgent (3)**: Real-time alerts, emergency messages - 7 day TTL
- **High (2)**: Important communications - 14 day TTL
- **Normal (1)**: Standard messages - 7 day TTL
- **Low (0)**: Bulk/promotional - 3 day TTL

**Eviction Policy**:
1. Expire messages past TTL
2. If cache full (10,000 messages total):
   - Evict delivered messages first
   - Within undelivered: LRU eviction starting with lowest priority

**Delivery Retry**:
- Attempt delivery when device connects
- Retry failed deliveries every 5 minutes (up to 12 attempts)
- After 12 failures, mark as undeliverable but keep until TTL

### 4.4 Cache API Endpoints

**Store Message**: `POST /api/appliance/cache/store`
```json
{
  "device_id": "mobile-abc123",
  "direction": "inbound",
  "priority": 2,
  "payload": "base64...",
  "metadata": {
    "from": "sender-node-id",
    "message_type": "chat"
  },
  "ttl_days": 7
}
```

**Retrieve Messages**: `GET /api/appliance/cache/retrieve?device_id=mobile-abc123&limit=100`

**Mark Delivered**: `POST /api/appliance/cache/delivered`
```json
{
  "message_ids": ["msg1", "msg2", "msg3"]
}
```

**Cache Statistics**: `GET /api/appliance/cache/stats?device_id=mobile-abc123`
```json
{
  "device_id": "mobile-abc123",
  "total_cached": 42,
  "by_priority": {
    "urgent": 2,
    "high": 10,
    "normal": 25,
    "low": 5
  },
  "undelivered": 8,
  "oldest_message_age_secs": 86400
}
```

---

## 5. Configuration Synchronization

### 5.1 Configuration Categories

#### 5.1.1 Routing Preferences
```json
{
  "routing": {
    "default_policy": "privacy",     // privacy, performance, reliability, balanced
    "adapter_priority": ["i2p", "wifi", "cellular"],
    "qos_class_default": "normal",
    "multipath_enabled": true,
    "geographic_routing_enabled": false
  }
}
```

#### 5.1.2 Message Preferences
```json
{
  "messages": {
    "cache_on_appliance": true,
    "cache_priority_default": "normal",
    "auto_forward_to_appliance": true,
    "store_and_forward": true,
    "ttl_days": 7
  }
}
```

#### 5.1.3 Power Management
```json
{
  "power": {
    "offload_dht_to_appliance": true,
    "offload_ledger_sync": true,
    "mobile_heartbeat_interval": 300,  // Longer interval on mobile
    "appliance_as_proxy": true         // Route through appliance to save power
  }
}
```

#### 5.1.4 Privacy & Security
```json
{
  "privacy": {
    "always_use_i2p_via_appliance": false,
    "clearnet_allowed_on_mobile": true,
    "require_appliance_for_sensitive": true,
    "trusted_nodes_only": false
  }
}
```

### 5.2 Configuration Sync Protocol

**Upload Configuration**: `PUT /api/appliance/config`
```json
{
  "device_id": "mobile-abc123",
  "config_version": 5,
  "config": { /* full config object */ },
  "timestamp": 1700086400
}
```

**Download Configuration**: `GET /api/appliance/config?device_id=mobile-abc123`

**Configuration Change Notifications**: WebSocket `/ws/appliance/config-sync`
```json
{
  "event": "config_updated",
  "config_version": 6,
  "changed_keys": ["routing.default_policy", "messages.ttl_days"]
}
```

---

## 6. Routing Through Appliance

### 6.1 Routing Decision Tree

```
Mobile wants to send message
    │
    ├─ Is appliance paired and online?
    │   │
    │   NO─→ Use direct mobile adapters
    │   │
    │   YES
    │   │
    │   ├─ Is message priority HIGH or URGENT?
    │   │   │
    │   │   YES─→ Send via both appliance AND mobile adapters (redundancy)
    │   │   │
    │   │   NO
    │   │   │
    │   │   ├─ Is mobile on cellular with data limits?
    │   │   │   │
    │   │   │   YES─→ Route through appliance only
    │   │   │   │
    │   │   │   NO
    │   │   │   │
    │   │   │   ├─ Does user prefer privacy routing?
    │   │   │   │   │
    │   │   │   │   YES─→ Route through appliance's i2p
    │   │   │   │   │
    │   │   │   │   NO─→ Use best available adapter (mobile or appliance)
```

### 6.2 Appliance as Message Proxy

**Send Message via Appliance**: `POST /api/appliance/send`
```json
{
  "device_id": "mobile-abc123",
  "message": {
    "destination": "recipient-node-id",
    "payload": "encrypted-payload...",
    "priority": 2,
    "routing_hints": {
      "prefer_i2p": true,
      "max_hops": 5
    }
  }
}
```

**Response**:
```json
{
  "message_id": "msg_abc123",
  "status": "queued",
  "routing_path": "appliance -> i2p -> mesh",
  "estimated_delivery": 1700086500
}
```

---

## 7. Android Application Architecture

### 7.1 Technology Stack

- **Language**: Kotlin
- **UI**: Jetpack Compose
- **Architecture**: MVVM with Clean Architecture
- **DI**: Hilt
- **Networking**: Retrofit + OkHttp
- **WebSocket**: OkHttp WebSocket
- **Database**: Room
- **Background Work**: WorkManager + Foreground Service
- **Rust Integration**: JNI via `jni` crate

### 7.2 Module Structure

```
app/
├── core/               # Core Rust bindings
│   ├── jni/           # JNI bridge layer
│   └── rust/          # Compiled Rust library
├── data/              # Data layer
│   ├── repository/    # Repository implementations
│   ├── local/         # Room database
│   └── remote/        # Retrofit API clients
├── domain/            # Business logic
│   ├── model/         # Domain models
│   ├── usecase/       # Use cases
│   └── repository/    # Repository interfaces
├── presentation/      # UI layer
│   ├── appliance/     # Appliance management screens
│   ├── messages/      # Messaging UI
│   ├── settings/      # Settings and preferences
│   └── dashboard/     # Main dashboard
└── service/           # Background services
    ├── MyriadMeshService.kt
    ├── ApplianceSyncWorker.kt
    └── MessageSyncWorker.kt
```

### 7.3 Key Components

#### 7.3.1 MyriadNode JNI Bridge

**Rust Side** (`android/jni.rs`):
```rust
#[no_mangle]
pub extern "C" fn Java_com_myriadmesh_core_MyriadNode_nativeInit(
    env: JNIEnv,
    _class: JClass,
    config_path: JString,
) -> jlong {
    // Initialize MyriadNode
    // Return pointer as jlong
}

#[no_mangle]
pub extern "C" fn Java_com_myriadmesh_core_MyriadNode_nativeStart(
    env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
) -> jboolean {
    // Start node
}

#[no_mangle]
pub extern "C" fn Java_com_myriadmesh_core_MyriadNode_nativeSendMessage(
    env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
    destination: JString,
    payload: jbyteArray,
) -> jboolean {
    // Send message
}
```

**Kotlin Side** (`MyriadNode.kt`):
```kotlin
class MyriadNode private constructor(private val nodePtr: Long) {

    companion object {
        init {
            System.loadLibrary("myriadmesh_android")
        }

        @JvmStatic
        external fun nativeInit(configPath: String): Long

        @JvmStatic
        external fun nativeStart(nodePtr: Long): Boolean

        @JvmStatic
        external fun nativeSendMessage(
            nodePtr: Long,
            destination: String,
            payload: ByteArray
        ): Boolean
    }

    fun start(): Boolean = nativeStart(nodePtr)

    fun sendMessage(destination: String, payload: ByteArray): Boolean =
        nativeSendMessage(nodePtr, destination, payload)
}
```

#### 7.3.2 Appliance Manager

```kotlin
class ApplianceManager @Inject constructor(
    private val applianceApi: ApplianceApi,
    private val applianceDao: ApplianceDao,
    private val discoveryService: DiscoveryService
) {

    suspend fun discoverAppliances(): List<ApplianceInfo> {
        // mDNS discovery
        return discoveryService.discover()
    }

    suspend fun pairWithAppliance(
        applianceId: String,
        publicKey: ByteArray
    ): PairingResult {
        // Pairing flow
    }

    suspend fun syncConfiguration(
        applianceId: String,
        config: ApplianceConfig
    ): Result<Unit> {
        // Config sync
    }

    suspend fun retrieveCachedMessages(
        applianceId: String
    ): List<CachedMessage> {
        // Message retrieval
    }
}
```

#### 7.3.3 Background Service

```kotlin
class MyriadMeshService : LifecycleService() {

    private lateinit var myriadNode: MyriadNode
    private lateinit var applianceManager: ApplianceManager

    override fun onCreate() {
        super.onCreate()

        // Initialize node
        myriadNode = MyriadNode.initialize(configPath)
        myriadNode.start()

        // Start appliance sync
        startApplianceSync()

        // Show persistent notification
        startForeground(NOTIFICATION_ID, buildNotification())
    }

    private fun startApplianceSync() {
        lifecycleScope.launch {
            // Periodic sync with paired appliances
            while (isActive) {
                syncWithAppliances()
                delay(30_000) // Every 30 seconds
            }
        }
    }

    private suspend fun syncWithAppliances() {
        val appliances = applianceManager.getPairedAppliances()
        appliances.forEach { appliance ->
            // Retrieve cached messages
            val messages = applianceManager.retrieveCachedMessages(appliance.id)
            messages.forEach { deliverMessage(it) }
        }
    }
}
```

---

## 8. Power Management & Battery Optimization

### 8.1 Offloading Strategy

**Heavy operations offloaded to appliance**:
1. DHT participation and maintenance
2. Blockchain ledger synchronization
3. Message routing for non-urgent traffic
4. Heartbeat broadcasting (reduced frequency on mobile)
5. Backup/relay for incoming messages

**Mobile device retains**:
1. Direct peer-to-peer connections (WiFi Direct, Bluetooth)
2. Urgent message handling
3. UI and local crypto operations
4. Low-power adapter monitoring

### 8.2 Power Profiles

**High Performance Mode**:
- Full mesh participation
- All adapters active
- Normal heartbeat interval (60s)
- DHT active

**Balanced Mode** (Default with appliance):
- Appliance handles DHT and routing
- WiFi/Bluetooth active for local mesh
- Extended heartbeat interval (300s)
- Cellular only when needed

**Power Saver Mode**:
- Appliance handles all routing
- Only WiFi for local connections
- Heartbeat via appliance only
- Cellular disabled for mesh

### 8.3 Android Battery APIs

```kotlin
class PowerManager @Inject constructor(
    private val context: Context,
    private val batteryManager: BatteryManager
) {

    fun getCurrentProfile(): PowerProfile {
        val batteryPct = batteryManager.getIntProperty(
            BatteryManager.BATTERY_PROPERTY_CAPACITY
        )
        val isCharging = batteryManager.isCharging

        return when {
            isCharging -> PowerProfile.HIGH_PERFORMANCE
            batteryPct > 50 -> PowerProfile.BALANCED
            else -> PowerProfile.POWER_SAVER
        }
    }

    fun applyProfile(profile: PowerProfile) {
        when (profile) {
            PowerProfile.HIGH_PERFORMANCE -> {
                // Enable all adapters
                // Normal intervals
            }
            PowerProfile.BALANCED -> {
                // Selective adapters
                // Extended intervals
            }
            PowerProfile.POWER_SAVER -> {
                // Minimal adapters
                // Appliance-only routing
            }
        }
    }
}
```

---

## 9. Implementation Phases

### Phase 1: Appliance Mode Foundation (Week 1)
- [ ] Add appliance mode configuration to MyriadNode
- [ ] Implement appliance discovery API
- [ ] Create message cache database and management
- [ ] Implement pairing protocol (backend)

### Phase 2: Android Project Setup (Week 2)
- [ ] Create Android project structure
- [ ] Set up Rust cross-compilation for Android
- [ ] Implement JNI bridge
- [ ] Basic Android UI with Jetpack Compose

### Phase 3: Appliance Integration (Week 3-4)
- [ ] Android appliance discovery
- [ ] Pairing UI and flow
- [ ] Configuration sync implementation
- [ ] Message cache retrieval

### Phase 4: Routing & Power (Week 5-6)
- [ ] Routing through appliance
- [ ] Power management profiles
- [ ] Android background service
- [ ] Battery optimization

### Phase 5: Android Adapters (Week 7-8)
- [ ] WiFi Direct adapter
- [ ] Bluetooth Classic adapter
- [ ] Bluetooth LE adapter
- [ ] Cellular adapter

### Phase 6: Testing & Polish (Week 9)
- [ ] Integration testing
- [ ] Performance optimization
- [ ] UI/UX refinement
- [ ] Documentation

---

## 10. Security Considerations

### 10.1 Threat Model

**Threats**:
1. Rogue appliance impersonation
2. Man-in-the-middle during pairing
3. Session token theft
4. Unauthorized message access
5. Configuration tampering

**Mitigations**:
1. Mutual Ed25519 authentication during pairing
2. Visual verification via QR code
3. Short-lived session tokens with rotation
4. End-to-end encryption for all messages
5. Signed configuration updates

### 10.2 Android Security

- Store session tokens in EncryptedSharedPreferences
- Use Android Keystore for crypto keys
- Secure all network communications with TLS 1.3
- Implement certificate pinning for appliance connections
- Use SafetyNet attestation for app integrity

---

## 11. Testing Strategy

### 11.1 Unit Tests
- Message cache logic
- Pairing protocol
- Configuration sync
- Power management

### 11.2 Integration Tests
- Android <-> Appliance communication
- Message delivery via appliance
- Failover scenarios
- Multi-appliance scenarios

### 11.3 Performance Tests
- Message cache performance (10k messages)
- Battery drain with various profiles
- Network efficiency (bandwidth usage)
- Latency with appliance routing

### 11.4 User Acceptance Tests
- Pairing flow usability
- Message delivery reliability
- Configuration changes take effect
- Power savings measurable

---

## 12. Future Enhancements

### 12.1 Multi-Appliance Support
- Pair with multiple appliances (home, office, vehicle)
- Automatic selection based on location
- Seamless handoff between appliances

### 12.2 Appliance Clusters
- Multiple appliances form a cluster
- Load balancing across cluster
- High availability for cached messages

### 12.3 Advanced Caching
- Intelligent pre-caching based on patterns
- Content-based message deduplication
- Compression for large messages

### 12.4 iOS Support
- iOS app with similar appliance integration
- Cross-platform pairing protocol compatibility

---

## Appendices

### A. API Reference
See generated API documentation at `/docs/api/appliance.md`

### B. Configuration Schema
See JSON schema at `/schemas/appliance-config.schema.json`

### C. WebSocket Protocol
See protocol specification at `/docs/protocols/appliance-ws.md`

---

**Document End**
