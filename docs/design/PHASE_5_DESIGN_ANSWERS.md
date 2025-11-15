# Phase 5 Design Clarifications - Final Decisions

**Date:** 2025-11-15
**Status:** FINALIZED - Ready for Implementation

---

## Summary of Design Decisions

### Question 1: Adapter Implementation Scope
**DECISION: All 6 adapters require full implementations by end of Phase 5**

**Implication:**
- LoRaWAN: Full SPI + modem driver
- WiFi HaLoW: Full 802.11ah implementation (or simulator if hardware unavailable)
- APRS: Full AX.25 + KISS + TNC interface
- FRS/GMRS: Full AFSK/FreeDV modem + PTT control
- HF Radio: Full CAT control + digital modes
- Dial-up: Full Hayes AT + PPP implementation

**Timeline Impact:** Requires 5-6 months intensive implementation effort

---

### Question 2: License Verification Strategy
**DECISION: Block transmission, allow listen-only when no valid license**

**Implementation Approach:**
- Add `license_state` field to adapter configuration
- States: `Valid(callsign)`, `None`, `Expired`
- Transmission methods check state and return `Err(LicenseRequired)` if invalid
- Receive/listen operations are always allowed
- Create license management API to set/update callsign and validate

**New Components Needed:**
```rust
pub enum LicenseState {
    Valid { callsign: String, expires_at: Option<u64> },
    None,
    Expired,
}

pub trait LicensedAdapter {
    fn set_license(&mut self, callsign: String) -> Result<()>;
    fn get_license_state(&self) -> LicenseState;
    fn validate_license(&self) -> Result<()>;
}
```

**Affected Adapters:** APRS, HF Radio
**Configuration:** Per-node license management via node config file

---

### Question 3: Fragmentation Strategy
**DECISION: Hybrid approach with routing-aware logic**

**Rules:**
- **Small messages (<MTU):** Adapter handles directly
- **Large messages (>MTU) without combining:** Router fragments before sending
- **Exception:** When routing rules would combine transmissions to same dest node, DO NOT fragment at router level - let adapter handle (enables aggregation)

**Implementation Location:**
- Message Router: Check routing decision BEFORE fragmentation
- Adapter: Implement reassembly with timeout (60s default)
- Router: Only fragment if message will be sent alone to destination

**Code Pattern:**
```rust
// In MessageRouter::send()
let route_decision = self.dht.find_route(destination)?;

if route_decision.would_combine_with_other_messages {
    // Don't fragment, let adapter handle
    self.adapters.send(&adapter_id, destination, frame).await?
} else if frame.size() > adapter.mtu() {
    // Fragment and send fragments
    let fragments = fragment(frame, adapter.mtu())?;
    for frag in fragments {
        self.adapters.send(&adapter_id, destination, &frag).await?
    }
}
```

---

### Question 4: Power Management & Data Usage

**DECISION: Three-part approach**

#### 4a. Power Supply Management (NEW - Appliance Stack)

**Add to Appliance/Device Config:**
```rust
pub enum PowerSupply {
    ACMains,           // Desktop, plugged in
    PoE,              // Ethernet powered
    Battery {
        capacity_mwh: u32,
        current_mwh: u32,
        low_power_threshold_percent: u8,  // Default: 20%
        critical_threshold_percent: u8,   // Default: 5%
    },
}

pub struct PowerScaling {
    /// Max transmit power at different battery levels
    pub power_table: HashMap<u8, u8>,  // percent -> dBm
    /// Enable/disable expensive adapters based on battery
    pub adapter_availability: HashMap<AdapterType, BatteryThreshold>,
}
```

**Adaptive Behavior:**
- AC Mains: Full power always
- PoE: Dynamic based on available power budget
- Battery <20%: Reduce TX power by 50%, disable high-power adapters
- Battery <5%: Listen-only mode, disable TX entirely

#### 4b. Data Usage Tracking (Cellular/Dial-up)

**Configuration:**
```rust
pub struct DataUsagePolicy {
    /// Enable/disable data limit enforcement (default: false)
    pub enabled: bool,
    /// Warn at this many MB (default: 1000 = 1GB)
    pub warn_threshold_mb: u32,
    /// Hard limit in MB (default: unlimited)
    pub hard_limit_mb: u32,
    /// Reset period: Daily/Monthly/Quarterly
    pub reset_period: ResetPeriod,
}
```

**Behavior:**
- Warn at 1GB (configurable)
- Operator sets hard limits
- Off by default (no enforcement)
- Cellular adapter tracks usage and reports to network manager
- Option to throttle/block by priority level

---

### Question 5: HF Radio Propagation Awareness
**DECISION: Yes, include space weather integration**

**Implementation:**
```rust
pub struct SpaceWeatherData {
    pub solar_flux_index: u16,      // SFI (0-300)
    pub k_index: u8,                // K-index (0-9)
    pub a_index: u16,               // A-index (0-400)
    pub updated_at: u64,
}

pub trait HfRadioAdapter {
    async fn fetch_space_weather(&mut self) -> Result<SpaceWeatherData>;
    async fn auto_select_band(&self, data: &SpaceWeatherData) -> Result<f32>;
    fn estimate_propagation_quality(&self, dest_freq: f32, data: &SpaceWeatherData) -> f64;
}
```

**Data Sources:**
- NOAA Space Weather Prediction Center API (primary)
- Local cached database (fallback)
- VOACAP online prediction service (optional)

**Integration with Routing:**
- HF adapter provides propagation quality metric to router
- Router uses metric for hop selection
- Suggested bands displayed to operator

---

### Question 6: Hardware Availability Strategy
**DECISION: Build everything now, fix when hardware available**

**Approach:**
- Implement all 6 adapters fully per specification
- Use mock adapters for unavailable hardware (WiFi HaLoW, etc.)
- Create hardware abstraction layer:
  ```rust
  #[cfg(feature = "wifi-halow-hardware")]
  use crate::adapters::WifiHalowHardware;

  #[cfg(not(feature = "wifi-halow-hardware"))]
  use crate::adapters::WifiHalowMock as WifiHalowHardware;
  ```

**Testing Strategy:**
- Unit tests use mocks
- Integration tests available for both hardware and mocks
- CI/CD runs with mocks
- Hardware tests run on separate branch/platform when available

---

### Question 7: Documentation Strategy
**DECISION: Painstakingly comprehensive, multi-level, wiki-compatible**

**Documentation Tiers:**

#### Tier 1: Quick Start (5 minutes)
- Basic setup for each adapter
- Minimal configuration
- Common use cases

#### Tier 2: Practical Guides (30 minutes)
- Step-by-step setup by skill level: Beginner, Intermediate, Advanced
- Troubleshooting sections
- Configuration deep-dives

#### Tier 3: Vendor-Specific (Stretch Goal)
- Popular hardware models:
  - LoRa: Heltec Wireless Stick, TTGO LoRa
  - FRS/GMRS: Baofeng UV-5R, TYT MD-390
  - HF: Yaesu FT-991A, Icom IC-7300
  - APRS: TNC-X, TNC232, Direwolf software TNC
  - WiFi HaLoW: Qualcomm 802.11ah dev board (if available)

#### Tier 4: API Reference (Wiki-Ready)
- OpenAPI spec for all endpoints
- Markdown format for FlatDocs/ReadTheDocs
- Schema diagrams
- Code examples in multiple languages

**Deliverables:**
- `/docs/guides/adapters/` - Practical guides by tier
- `/docs/api/` - Auto-generated from code comments
- `/docs/hardware/` - Vendor-specific guides
- `/docs/troubleshooting/` - Common issues and solutions

---

### Question 8: Legacy Technology & Deprecation Policy
**DECISION: Support indefinitely, no deprecation, modular plugin system**

**Rationale:** Emergency communication is core mission

**Policy:**
- All adapters remain supported as long as hardware exists
- If technology becomes obsolete, efforts to provide hardware-level support
- Modular plugin architecture allows:
  - Easy addition of new adapters in future
  - Community contributions without core changes
  - Hardware-specific implementations without bloating core

**Plugin Architecture:**
```rust
pub trait AdapterPlugin: NetworkAdapter {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn dependencies(&self) -> Vec<String>;
    fn capabilities(&self) -> Vec<String>;
}

pub struct AdapterRegistry {
    core_adapters: HashMap<String, Box<dyn NetworkAdapter>>,
    plugins: HashMap<String, Box<dyn AdapterPlugin>>,
}

impl AdapterRegistry {
    pub fn register_plugin(&mut self, plugin: Box<dyn AdapterPlugin>) -> Result<()>;
    pub fn list_plugins(&self) -> Vec<&str>;
    pub fn enable_plugin(&mut self, name: &str) -> Result<()>;
    pub fn disable_plugin(&mut self, name: &str) -> Result<()>;
}
```

**Future Technologies:** Easily added as plugins when hardware available
- Satellite (Iridium, Starlink, etc.)
- Mesh networks (Zigbee, Matter)
- New radio technologies

---

### Question 9: Interoperability - Full Platform Integration
**DECISION: Maximum interoperability with relay and scheduling support**

#### 9a. Meshtastic Interoperability

**Implementation:**
- MyriadMesh nodes can relay Meshtastic packets
- Meshtastic nodes can relay MyriadMesh packets
- Protocol translation layer for packet formats
- Discovery mechanism to identify peer networks

```rust
pub mod meshtastic_bridge {
    pub struct MeshtasticBridge {
        // Translates between Meshtastic and MyriadMesh frames
    }

    impl MeshtasticBridge {
        pub fn encode_meshtastic(&self, frame: &MyriadMeshFrame) -> Vec<u8>;
        pub fn decode_meshtastic(&self, data: &[u8]) -> Result<MyriadMeshFrame>;
    }
}
```

#### 9b. APRS-IS Integration

**Implementation:**
- Licensed operators can participate via unified platform
- Nodes function as repeaters
- Advanced scheduling for packet radio

```rust
pub struct AprsGateway {
    pub aprs_is_connection: AprsIsClient,
    pub scheduling: PacketScheduler,
}

pub struct PacketScheduler {
    /// Schedule transmissions to avoid conflicts
    pub quiet_periods: Vec<(TimeRange, Vec<AdapterType>)>,
    /// Preferred transmission windows
    pub optimal_windows: HashMap<Callsign, TimeRange>,
    /// Retry schedule for failed deliveries
    pub retry_backoff: ExponentialBackoff,
}
```

#### 9c. Multi-Platform Relay Support

```rust
pub enum RelayTarget {
    MyriadMesh(NodeId),
    Meshtastic(MeshtasticId),
    APRS(CallSign),
}

pub struct RelayDecision {
    pub targets: Vec<RelayTarget>,
    pub schedule: RelaySchedule,
    pub priority: Priority,
}
```

**Advanced Scheduling for Packet Radio:**
- Collision avoidance on shared frequencies
- Time-slotted transmission for APRS
- Intelligent retry scheduling
- Load balancing across repeaters

---

### Question 10: Plugin Architecture & Protocol Focus
**DECISION: Core plugin system with protocol as centerpiece**

**Architecture Layers:**

#### Layer 1: Protocol (Core Focus)
- MyriadMesh protocol (transport-agnostic)
- Message framing and routing
- Adapter abstraction
- **No dependencies on specific hardware/software**

#### Layer 2: Core Adapters
- 6 Phase 5 adapters (built-in)
- Ethernet, Bluetooth, Cellular (Phase 1-3)
- Well-tested and maintained by core team

#### Layer 3: Plugin System
- Community adapters
- Third-party integrations
- Application-specific extensions

```rust
pub trait MyriadMeshPlugin {
    fn plugin_name(&self) -> &str;
    fn plugin_version(&self) -> &str;
    fn initialize(&mut self, config: PluginConfig) -> Result<()>;
}

pub trait AdapterPlugin: MyriadMeshPlugin + NetworkAdapter {
    fn adapter_type(&self) -> AdapterType;
    fn hardware_requirements(&self) -> Vec<String>;
}

pub trait ApplicationPlugin: MyriadMeshPlugin {
    /// Higher-level functionality using mesh network
    fn register_message_handler(&self) -> MessageHandler;
    fn provide_ui_components(&self) -> Vec<UiComponent>;
}

pub trait BridgePlugin: MyriadMeshPlugin {
    /// Bridge to external networks (LORA-WAN, LoRaWAN compat, etc.)
    fn translate_message(&self, from_network: &str, data: &[u8]) -> Result<Frame>;
}
```

#### Layer 4: Applications
- MyriadNode (reference implementation)
- Third-party apps using mesh protocol
- IoT applications native to mesh

**Benefits of this architecture:**
- Protocol remains stable and independent
- Core team maintains proven adapters
- Community can extend with plugins
- Applications can use mesh as native transport
- No deprecation needed - just add new plugins

**Plugin Distribution:**
```
myriadmesh-plugins/
├── community/
│   ├── satellite-adapter/
│   ├── zigbee-bridge/
│   ├── mobile-app-extension/
├── official/
│   ├── meshtastic-compat/
│   ├── aprs-gateway/
```

---

## Architectural Changes Required

### 1. Power Management System (Appliance Stack)
**New Files:**
- `crates/myriadmesh-appliance/src/power.rs`
  - PowerSupply enum and management
  - Adaptive power scaling
  - Battery monitoring

**Changes to Adapters:**
- Each adapter tracks power consumption
- Respect power limits set by appliance
- Report power state to power manager

### 2. License Management System
**New Files:**
- `crates/myriadmesh-network/src/license.rs`
  - LicenseState enum
  - License validation
  - FCC/amateur radio integration

**Changes to Adapters:**
- APRS: Check license before transmit
- HF Radio: Check license before transmit
- Both: Allow receive without license

### 3. Plugin Architecture
**New Files:**
- `crates/myriadmesh-core/src/plugin.rs`
  - Plugin trait definitions
  - Plugin registry
  - Plugin lifecycle management

**Changes:**
- Adapter trait becomes base of plugin system
- Registry system for plugin loading
- Plugin configuration and management

### 4. Interoperability Bridges
**New Modules:**
- `crates/myriadmesh-network/src/bridges/meshtastic.rs`
- `crates/myriadmesh-network/src/bridges/aprs.rs`
- `crates/myriadmesh-network/src/bridges/generic.rs`

**New Files:**
- `crates/myriadmesh-routing/src/scheduling.rs` (packet radio scheduling)

### 5. Documentation System
**New Directories:**
- `docs/guides/` - Multi-tier guides
- `docs/hardware/` - Vendor-specific
- `docs/api/` - Auto-generated API docs
- `docs/troubleshooting/` - Common issues

---

## Implementation Roadmap - Updated for Decisions

### Phase 5a: Foundation (Months 14-15)
1. Implement power management system
2. Implement license management system
3. Create plugin architecture
4. Set up documentation infrastructure

### Phase 5b: Core Adapters (Months 15-16)
1. LoRaWAN (highest priority, common use case)
2. APRS (with license checking)
3. FRS/GMRS (with power management)
4. Implement mock adapters for unavailable hardware

### Phase 5c: Advanced Features (Months 16-17)
1. HF Radio with space weather integration
2. WiFi HaLoW (or mock if hardware unavailable)
3. Dial-up (legacy emergency support)
4. Plugin system full implementation

### Phase 5d: Bridges & Interop (Months 17-18)
1. Meshtastic bridge
2. APRS-IS integration with advanced scheduling
3. Packet radio scheduling system
4. Documentation completion

---

## Success Criteria - Updated

Phase 5 is complete when:

✅ All 6 adapters fully implemented
✅ Power management system operational with adaptive scaling
✅ License checking (transmit-only) implemented for APRS/HF
✅ Hybrid fragmentation strategy working with routing awareness
✅ Space weather integration for HF adapter
✅ Comprehensive documentation (3+ tiers, vendor-specific guides)
✅ Plugin architecture fully functional
✅ Meshtastic relay support operational
✅ APRS-IS integration with scheduling
✅ Mock adapters for unavailable hardware
✅ 10+ total adapter types (with plugin system ready for more)
✅ All tests passing (unit, integration, hardware when available)

---

## Key Decisions Summary Table

| Area | Decision | Impact |
|------|----------|--------|
| Scope | All 6 adapters, full impl | +3 months development |
| License | Block TX, allow RX | New license system needed |
| Fragmentation | Hybrid + routing-aware | More complex routing logic |
| Power | Supply-aware adaptive | New appliance subsystem |
| Data Usage | Warn 1GB, hard limits | Cellular monitoring needed |
| HF Weather | Space weather API | External dependency, fallback DB |
| Hardware | Build now, fix later | Mocks for unavailable HW |
| Documentation | Multi-tier + vendor | Significant documentation effort |
| Legacy | Support indefinitely | Plugin system essential |
| Interop | Full bridges + scheduling | Complex relay implementation |
| Extensibility | Core plugin system | Architectural foundation change |

---

## Next Implementation Steps

1. ✅ **Decisions documented** (this file)
2. **Create architectural PRD** with specific component specs
3. **Update stub adapters** to reflect new decisions
4. **Implement power management system** (Phase 5a foundation)
5. **Implement license management system** (Phase 5a foundation)
6. **Create plugin architecture** (Phase 5a foundation)
7. **Begin Phase 5b: Core adapter implementations**

---

**Framework Status:** ✅ Clarifications Complete - Ready for Detailed Design
**Expected Phase 5 Start:** Immediate (foundation work in parallel with final design)
