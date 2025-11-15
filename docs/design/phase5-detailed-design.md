# Phase 5: Specialized Adapters - Detailed Design Document

**Version:** 1.0 (Draft)
**Date:** 2025-11-15
**Status:** Framework & Clarification Phase

## Executive Summary

Phase 5 extends MyriadMesh with six specialized network adapters targeting low-bandwidth, long-range, and amateur radio communication scenarios. This phase prioritizes radio-based adapters for resilient last-mile connectivity and emergency communications.

**Phase 5 Goals:**
- Implement 6 new network adapter types
- Support both licensed (cellular, shortwave) and unlicensed (LoRa, Bluetooth) spectrum
- Enable operation in bandwidth-constrained and power-limited environments
- Maintain compatibility with existing Phase 1-4 infrastructure

**Target Completion:** Months 14-18 of project timeline

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Adapter Specifications](#adapter-specifications)
3. [Implementation Patterns](#implementation-patterns)
4. [Fragmentation Strategy](#fragmentation-strategy)
5. [Power Management](#power-management)
6. [Testing Strategy](#testing-strategy)
7. [Timeline](#timeline)
8. [Design Questions](#design-questions)

---

## Architecture Overview

### Phase 5 Adapter Stack

```
┌─────────────────────────────────────────────────────────┐
│                   Application Layer                      │
│              (Phases 3-4: MyriadNode)                   │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│              Message Router (Phase 2)                    │
│         Adapter Selection & Routing Engine              │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│         Adapter Manager (Network Abstraction)           │
│  ┌──────────────────────────────────────────────────┐   │
│  │ Phase 1-4 Adapters      │   Phase 5 New          │   │
│  ├─────────────────────────┼──────────────────────┤   │
│  │ • Ethernet              │ • LoRaWAN/Meshtastic  │   │
│  │ • Bluetooth Classic     │ • Wi-Fi HaLoW         │   │
│  │ • Bluetooth LE          │ • APRS                │   │
│  │ • Cellular              │ • FRS/GMRS            │   │
│  │ • i2p                   │ • CB/Shortwave        │   │
│  │                         │ • Dial-up/PPPoE       │   │
│  └─────────────────────────┴──────────────────────┘   │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│            Physical Transport Drivers                    │
│  (SPI, Serial, USB, RF Interfaces)                      │
└──────────────────────────────────────────────────────────┘
```

### Adapter Capability Matrix

| Adapter | Range | Bandwidth | Latency | Power | Cost | Licensed | Spectrum |
|---------|-------|-----------|---------|-------|------|----------|----------|
| LoRa | 15+ km | 50 bps-50 kbps | 1-10s | Low | Low | No | ISM 902/868 |
| WiFi HaLoW | 1-10 km | 150 kbps-19 Mbps | 50-100ms | Med | Med | No | 802.11ah |
| APRS | 30+ km | 1200 bps | 2-5s | Low | Low | Yes* | Ham Radio |
| FRS/GMRS | 0.5-25 km | 4.8 kbps (digital) | 100ms | Low | Low | Partial | UHF 462-467 |
| CB/HF | 100+ km | 300 bps-2400 bps | 2-10s | Med | Med | Yes* | 27 MHz / HF |
| Dial-up | Varies | 2.4-56 kbps | 100-500ms | Low | High | Yes | PSTN/GSM |

*Licensed adapters require operator license (Ham Radio: General or Extra; CB: No license; HF: Amateur Radio)

---

## Adapter Specifications

### 5.1 LoRaWAN/Meshtastic Adapter

**Purpose:** Long-range, low-power mesh networking for remote and IoT applications

#### Configuration

```rust
pub struct LoRaConfig {
    /// Frequency in Hz (868 MHz Europe, 902 MHz N. America)
    pub frequency_hz: u32,
    /// Spreading factor (7-12, higher = longer range, slower)
    pub spreading_factor: u8,
    /// Bandwidth in kHz (125, 250, 500)
    pub bandwidth_khz: u16,
    /// Coding rate (4/5 to 4/8)
    pub coding_rate: f32,
    /// Transmit power in dBm (2-20)
    pub tx_power_dbm: i8,
    /// Enable Meshtastic compatibility
    pub meshtastic_mode: bool,
    /// Duty cycle limit (1-100%, EU: 1%)
    pub duty_cycle_percent: f32,
}
```

#### Address Format

```
lora://<device_id>@<frequency_hz>
Example: lora://0x12345678@868000000
```

#### Implementation Notes

- **Hardware:** SPI interface to SX1262/SX1276 modem
- **Fragmentation:** Messages split into 240-byte LoRa packets (1664 bytes max per datagram)
- **Meshtastic:** Protocol translation for interop with Meshtastic mesh devices
- **Duty Cycle:** Enforce regulatory limits (EU: 1%, US: unlimited in ISM)
- **Key Challenge:** Low bandwidth (~50 bps at SF12) requires aggressive message prioritization

#### Privacy Level

- **0.90** (Very High) - RF-only, no central infrastructure, immune to network surveillance
- Consideration: Large message padding overhead unaffordable at low SF

---

### 5.2 Wi-Fi HaLoW (802.11ah) Adapter

**Purpose:** Energy-efficient long-range WiFi for IoT and mesh networks

#### Configuration

```rust
pub struct WifiHalowConfig {
    /// SSID to connect to
    pub ssid: String,
    /// Passphrase
    pub password: Option<String>,
    /// Channel (1-6 for 1MHz, 7-13 for 2MHz, etc.)
    pub channel: u8,
    /// Enable power save mode
    pub power_save: bool,
    /// Target Wake Time interval (ms)
    pub twt_interval_ms: u32,
}
```

#### Address Format

```
halow://<mac_address>@<ssid>
Example: halow://00:11:22:33:44:55@office-mesh
```

#### Implementation Notes

- **Hardware:** 802.11ah compatible WiFi adapter (rare, mostly development boards)
- **Power Efficiency:** Target Wake Time (TWT) reduces power consumption by 80%+
- **Range:** 1-10 km with reduced bandwidth
- **Challenge:** Very limited hardware availability; may require software emulation

#### Privacy Level

- **0.65** (Medium-High) - Can be isolated to local network, but SSID broadcast exposes network presence

---

### 5.3 Amateur Packet Radio (APRS) Adapter

**Purpose:** Worldwide connectivity via amateur radio network (requires HAM license)

#### Configuration

```rust
pub struct AprsConfig {
    /// APRS-IS server hostname
    pub aprs_is_server: String,
    /// APRS-IS port (default 14580)
    pub aprs_is_port: u16,
    /// Callsign (e.g., "N0CALL-1")
    pub callsign: String,
    /// APRS passcode (hash of callsign)
    pub passcode: u16,
    /// TNC device path (e.g., "/dev/ttyUSB0")
    pub tnc_device: String,
    /// Enable APRS-IS gateway relay
    pub use_internet_gateway: bool,
    /// Require valid FCC license
    pub license_check: bool,
}
```

#### Address Format

```
aprs://<callsign>@<aprs-is-server>
Example: aprs://N0CALL-1@noam.aprs2.net
```

#### Implementation Notes

- **Protocol:** AX.25 over KISS (TNC interface)
- **Transport:** Radio to TNC, or APRS-IS network
- **Digipeaters:** Support packet relay via digipeater network
- **Licensing:** Requires Amateur Radio license (Technician or higher in US)
- **Key Challenge:** License verification; APRS-IS requires authentication

#### Privacy Level

- **0.45** (Medium-Low) - Callsign is broadcast; APRS-IS network logs all packets
- **Mitigation:** Use `-n` suffix for data-only (non-position) packets

---

### 5.4 FRS/GMRS Radio Adapter

**Purpose:** Local unlicensed or licensed UHF radio for mesh networks

#### Configuration

```rust
pub struct FrsGmrsConfig {
    /// Frequency in Hz (FRS: 462.5625-467.7125 MHz)
    pub frequency_hz: f32,
    /// Modulation: FM, AFSK, FreeDV
    pub modulation: ModulationType,
    /// Power level in watts (FRS max: 0.5W, GMRS max: 5W)
    pub tx_power_watts: f32,
    /// Enable CTCSS (Continuous Tone Coded Squelch System)
    pub ctcss_enabled: bool,
    /// CTCSS tone frequency in Hz
    pub ctcss_frequency_hz: Option<f32>,
    /// PTT (Push-to-Talk) GPIO pin (if using radio module)
    pub ptt_gpio_pin: Option<u8>,
}

pub enum ModulationType {
    FM,
    AFSK,
    FreeDV, // Codec2-based digital
}
```

#### Address Format

```
frsgmrs://<frequency_hz>@<channel>
Example: frsgmrs://462562500@channel-1
```

#### Implementation Notes

- **FRS:** Family Radio Service (license-free in US, 14 channels)
- **GMRS:** General Mobile Radio Service (license required in US, 16 channels)
- **Modem:** Software AFSK (1200 bps) or FreeDV (1600 bps with Codec2)
- **Hardware:** Serial UART to radio module (Baofeng, Yaesu, etc.)
- **Challenge:** Integrating software modems; handling PTT control

#### Privacy Level

- **0.60** (Medium) - Transmissions are broadcast on public frequencies
- **Mitigation:** Encrypt payloads (E2E encryption via protocol layer)

---

### 5.5 CB/Shortwave Radio Adapter

**Purpose:** Long-distance communication via HF amateur radio (requires HAM license)

#### Configuration

```rust
pub struct HfRadioConfig {
    /// Radio model (e.g., "FT-991A", "IC-7300")
    pub radio_model: String,
    /// CAT (Computer-Aided Transceiver) serial device
    pub cat_device: String,
    /// Baud rate for CAT (9600, 19200, 38400)
    pub cat_baud_rate: u32,
    /// Frequency in Hz (3.5-29.7 MHz for amateur bands)
    pub frequency_hz: f32,
    /// Mode: SSB, FSK, FT8, PSK31, RTTY
    pub digital_mode: DigitalMode,
    /// Power level in watts (0-100)
    pub tx_power_watts: f32,
    /// Enable automatic band switching
    pub auto_band_switching: bool,
}

pub enum DigitalMode {
    PSK31,
    RTTY,
    FT8,  // FT8 requires external decoder (WSJT-X)
    Packet,
}
```

#### Address Format

```
hf://<callsign>@<frequency_mhz>
Example: hf://N0CALL@7.040
```

#### Implementation Notes

- **CAT Control:** Hamlib-compatible (rigctl) for frequency/mode control
- **Digital Modes:** PSK31 (31 baud), RTTY (45/75 baud), FT8 (requires external decoder)
- **Propagation:** Time-of-day and solar cycle dependent (K-index, SFI)
- **Error Correction:** Convolutional codes or ARQ protocols
- **Challenge:** Software modem integration; complex CAT control; propagation-aware routing

#### Privacy Level

- **0.50** (Medium) - Shortwave transmissions widely monitored; callsign broadcast
- **Mitigation:** Use cryptographic encryption; maintain secure codebooks

---

### 5.6 Dial-up/PPPoE Adapter

**Purpose:** Legacy PSTN or GSM-SMS based emergency fallback

#### Configuration

```rust
pub struct DialupConfig {
    /// Modem type: Serial Hayes, USB, GSM (AT-command compatible)
    pub modem_type: ModemType,
    /// Serial device path
    pub device_path: String,
    /// Baud rate
    pub baud_rate: u32,
    /// Phone number to dial (PSTN) or modem ID (GSM)
    pub phone_number: String,
    /// PPP username and password
    pub ppp_username: String,
    pub ppp_password: String,
    /// Auto-dial timeout (seconds)
    pub auto_dial_timeout_secs: u32,
}

pub enum ModemType {
    SerialHayes,      // Traditional modem
    UsbModem,         // USB modem device
    GsmModule,        // GSM/SMS modem (SIM800, etc.)
}
```

#### Address Format

```
dialup://<phone_number>@<isp>
Example: dialup://5551234567@aol.com
```

#### Implementation Notes

- **PSTN:** Traditional dial-up modems (V.92, now obsolete)
- **GSM SMS:** SMS-based transport as fallback (extremely low bandwidth)
- **Protocol:** Hayes AT command set (original Unix modem standard)
- **Challenge:** Obsolete technology; maintaining compatibility

#### Privacy Level

- **0.40** (Low) - ISP can observe all traffic; phone records available
- **Mitigation:** Full E2E encryption at protocol layer

---

## Implementation Patterns

### Standard Adapter Structure

All Phase 5 adapters follow this template:

```rust
// File: crates/myriadmesh-network/src/adapters/{adapter_name}.rs

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::Result;
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::{types::AdapterType, Frame};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

// 1. Configuration struct with Default impl
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {AdapterName}Config {
    // Fields specific to adapter
}

impl Default for {AdapterName}Config {
    fn default() -> Self {
        // Sensible defaults for this adapter type
    }
}

// 2. Internal state struct (if needed)
#[derive(Debug, Clone)]
struct {AdapterName}State {
    // Mutable state during operation
}

// 3. Adapter struct using Arc<RwLock<>> for async safety
pub struct {AdapterName}Adapter {
    config: {AdapterName}Config,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    state: Arc<RwLock<{AdapterName}State>>,
    rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<(Address, Frame)>>>>,
    incoming_tx: mpsc::UnboundedSender<(Address, Frame)>,
}

impl {AdapterName}Adapter {
    pub fn new(config: {AdapterName}Config) -> Self {
        // Initialize capabilities and channels
    }

    // 4. Helper methods for platform-specific operations
    async fn establish_connection(&mut self) -> Result<()> {
        // TODO: Implement hardware initialization
        unimplemented!("Phase 5 stub")
    }
}

// 5. NetworkAdapter trait implementation
#[async_trait::async_trait]
impl NetworkAdapter for {AdapterName}Adapter {
    async fn initialize(&mut self) -> Result<()> {
        // TODO: Implement
        unimplemented!("Phase 5 stub")
    }

    async fn start(&mut self) -> Result<()> {
        // TODO: Implement
        unimplemented!("Phase 5 stub")
    }

    // ... other trait methods
}

// 6. Unit tests
#[cfg(test)]
mod tests {
    // TODO: Add tests
}
```

### Fragmentation & MTU Handling

Since many Phase 5 adapters have strict MTU limits:

```rust
/// Fragment large messages for low-bandwidth adapters
fn fragment_message(frame: &Frame, mtu: usize) -> Result<Vec<Vec<u8>>> {
    let frame_bytes = frame.to_bytes()?;

    if frame_bytes.len() <= mtu {
        return Ok(vec![frame_bytes]);
    }

    // Fragment format:
    // [Fragment Header: 4 bytes]
    //   - Message ID (2 bytes) - same across all fragments
    //   - Fragment number (1 byte)
    //   - Total fragments (1 byte)
    // [Fragment payload: mtu-4 bytes]

    let payload_size = mtu - 4;
    let total_frags = (frame_bytes.len() + payload_size - 1) / payload_size;
    let mut fragments = Vec::new();

    for frag_num in 0..total_frags as u8 {
        let start = (frag_num as usize) * payload_size;
        let end = std::cmp::min(start + payload_size, frame_bytes.len());

        let mut fragment = Vec::with_capacity(mtu);
        // Add header
        fragment.extend_from_slice(&[(frag_num >> 8) as u8, (frag_num & 0xFF) as u8]);
        fragment.push(frag_num);
        fragment.push(total_frags as u8);
        // Add payload
        fragment.extend_from_slice(&frame_bytes[start..end]);

        fragments.push(fragment);
    }

    Ok(fragments)
}
```

### Power Management for Battery-Powered Adapters

```rust
/// Power state transitions for battery-constrained adapters
#[derive(Debug, Clone, Copy)]
pub enum PowerState {
    /// Actively sending/receiving
    Active,
    /// Low power listening (heartbeat only)
    Idle,
    /// Deep sleep (disabled unless woken by interrupt)
    Sleep,
}

/// Adapter should implement power awareness
pub trait PowerAware {
    async fn set_power_state(&mut self, state: PowerState) -> Result<()>;
    fn estimate_power_consumption_mw(&self) -> u32;
}
```

---

## Fragmentation Strategy

### Adapter MTU Mapping

| Adapter | MTU | Fragmentation | Notes |
|---------|-----|----------------|-------|
| LoRaWAN | 240 | 7-frame max | ~1.7 KB max message |
| WiFi HaLoW | 1500 | Rare | Standard IP fragmentation |
| APRS | 256 | AX.25 UI frames | Digipeater limits |
| FRS/GMRS | 64 | 20+ frames needed | Very restrictive |
| CB/HF | 128 | 10+ frames needed | Error correction overhead |
| Dial-up | 1500 | Rare | PPP frames |

### Fragment Reassembly

Implement at the message router level:

```rust
struct FragmentReassembler {
    /// Pending fragments by message ID
    pending: HashMap<u16, Vec<Option<Vec<u8>>>>,
    /// Expiration times
    expiration: HashMap<u16, Instant>,
}

impl FragmentReassembler {
    fn add_fragment(&mut self, msg_id: u16, frag_num: u8, total: u8, data: Vec<u8>) -> Option<Vec<u8>> {
        // Allocate space if first fragment
        if !self.pending.contains_key(&msg_id) {
            self.pending.insert(msg_id, vec![None; total as usize]);
            self.expiration.insert(msg_id, Instant::now() + Duration::from_secs(60));
        }

        // Store fragment
        let frags = &mut self.pending[&msg_id];
        if (frag_num as usize) < frags.len() {
            frags[frag_num as usize] = Some(data);
        }

        // Check if complete
        if frags.iter().all(|f| f.is_some()) {
            let mut result = Vec::new();
            for frag in frags {
                result.extend_from_slice(frag.as_ref().unwrap());
            }
            self.pending.remove(&msg_id);
            self.expiration.remove(&msg_id);
            return Some(result);
        }

        None
    }
}
```

---

## Power Management

### Battery-Aware Routing

Low-power adapters should be deprioritized when alternatives exist:

```rust
fn calculate_adapter_score_battery_aware(
    adapter: &dyn NetworkAdapter,
    message: &Frame,
) -> f64 {
    let capabilities = adapter.get_capabilities();

    // Reduce score if high power consumption
    let power_penalty = match capabilities.power_consumption {
        PowerConsumption::None => 0.0,
        PowerConsumption::Low => -0.05,
        PowerConsumption::Medium => -0.15,
        PowerConsumption::High => -0.30,
    };

    // ... existing scoring logic + power_penalty
}
```

### Idle Power Reduction

For battery devices, implement power states:

```rust
pub trait BatteryOptimized {
    /// Reduce power consumption during idle periods
    async fn enter_low_power_mode(&mut self) -> Result<()>;

    /// Resume full operation
    async fn exit_low_power_mode(&mut self) -> Result<()>;

    /// Estimated battery life in hours at current usage
    fn estimate_battery_hours(&self) -> u32;
}
```

---

## Testing Strategy

### Phase 5 Testing Framework

#### 1. Unit Tests (per adapter)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = {AdapterName}Config::default();
        let adapter = {AdapterName}Adapter::new(config);
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
    }

    #[tokio::test]
    async fn test_capability_reporting() {
        let adapter = {AdapterName}Adapter::new(Default::default());
        let caps = adapter.get_capabilities();
        assert!(caps.max_message_size > 0);
    }

    #[tokio::test]
    async fn test_address_parsing() {
        let adapter = {AdapterName}Adapter::new(Default::default());
        let addr = adapter.parse_address("protocol://example").unwrap();
        assert!(adapter.supports_address(&addr));
    }
}
```

#### 2. Integration Tests

- Multi-hop message routing through Phase 5 adapters
- Fragmentation and reassembly across low-MTU adapters
- Power state transitions and battery life estimation
- License verification for regulated adapters (APRS, HF)

#### 3. Hardware Testing (Requires Actual Hardware)

- Physical layer testing with real radio modules
- Propagation testing (especially HF)
- Power consumption profiling
- Interference susceptibility

#### 4. Simulation & Emulation

For adapters without hardware:

```rust
/// Mock adapter for testing without hardware
#[cfg(test)]
pub struct {AdapterName}MockAdapter {
    // Simulated behavior
}
```

---

## Timeline

### Month 14: LoRaWAN/Meshtastic Adapter
- **Week 1-2:** Hardware interface (SPI driver)
- **Week 2-3:** LoRa modem configuration
- **Week 3-4:** Fragmentation and reassembly
- **Week 4:** Integration tests

### Month 15: WiFi HaLoW & APRS Adapters
- **Week 1-2:** WiFi HaLoW (if hardware available, else mock)
- **Week 2-3:** APRS TNC interface and KISS protocol
- **Week 3-4:** License verification framework
- **Week 4:** Integration testing

### Month 16: FRS/GMRS & CB/HF Radio Adapters
- **Week 1-2:** FRS/GMRS modem and PTT control
- **Week 2-3:** CB/HF CAT control (Hamlib integration)
- **Week 3-4:** Digital mode support (PSK31, FT8)
- **Week 4:** Integration testing

### Month 17: Dial-up/PPPoE Adapter
- **Week 1-2:** Hayes modem support (legacy)
- **Week 2-3:** GSM SMS fallback mode
- **Week 3-4:** PPP protocol implementation
- **Week 4:** Integration testing

### Month 18: Cross-Adapter Optimization & Documentation
- **Week 1-2:** Performance optimization across all Phase 5 adapters
- **Week 2-3:** Comprehensive adapter documentation and examples
- **Week 3-4:** Final security review and hardening

---

## Success Criteria

Phase 5 is complete when:

- ✅ All 6 adapters have working implementations (stubs OK initially)
- ✅ Each adapter passes unit tests (>80% coverage)
- ✅ Multi-hop routing works across different adapter types
- ✅ Fragmentation works for low-MTU adapters (LoRa, APRS, FRS/GMRS)
- ✅ Power consumption estimated for battery adapters
- ✅ License verification implemented for APRS and HF
- ✅ Hardware compatibility documented
- ✅ Full integration tests passing
- ✅ 10+ total adapter types supported (includes Phases 1-4)

---

## Design Decisions - FINALIZED

All design clarifications have been completed. See `PHASE_5_DESIGN_ANSWERS.md` for detailed rationale.

**Key Decisions Implemented:**

1. ✅ **All 6 adapters require full implementations** by end of Phase 5
2. ✅ **License strategy:** Block transmission, allow listen-only when unlicensed
3. ✅ **Fragmentation:** Hybrid approach with routing-aware logic
4. ✅ **Power management:** Adaptive power scaling per supply type (AC/PoE/Battery)
5. ✅ **HF Propagation:** Full space weather integration from NOAA/VOACAP
6. ✅ **Hardware strategy:** Build everything now, fix when hardware available
7. ✅ **Documentation:** Multi-tier (Beginner/Intermediate/Advanced + Vendor-specific)
8. ✅ **Legacy support:** Indefinite, with modular plugin architecture
9. ✅ **Interoperability:** Full bridges to Meshtastic/APRS with advanced scheduling
10. ✅ **Extensibility:** Core plugin system for future technologies

---

## Next Steps

1. **Review this framework** and provide feedback
2. **Answer design questions** (separate document)
3. **Prioritize adapters** based on use case needs
4. **Begin implementation** with LoRaWAN (most common Phase 5 use case)
5. **Establish hardware test lab** for radio-based adapters

---

## Appendix: Adapter Configuration Example

```yaml
# config.toml for Phase 5 adapters

[adapters]

# LoRaWAN/Meshtastic
[adapters.lora]
enabled = true
frequency_hz = 868000000
spreading_factor = 7
bandwidth_khz = 125
coding_rate = 0.8
tx_power_dbm = 14
meshtastic_mode = true
duty_cycle_percent = 1.0  # EU regulation

# WiFi HaLoW (if available)
[adapters.wifi_halow]
enabled = false  # Requires compatible hardware
ssid = "mesh-network"
password = "secure-password"
channel = 1
power_save = true

# APRS (requires HAM license)
[adapters.aprs]
enabled = true
callsign = "N0CALL-1"
passcode = 12345
tnc_device = "/dev/ttyUSB0"
use_internet_gateway = true
license_check = true

# FRS/GMRS
[adapters.frsgmrs]
enabled = true
frequency_hz = 462562500  # FRS channel 1
modulation = "FreeDV"
tx_power_watts = 0.5
ctcss_enabled = true
ctcss_frequency_hz = 67.0

# CB/HF Radio (requires HAM license)
[adapters.hf_radio]
enabled = false  # Requires CAT-capable radio
radio_model = "FT-991A"
cat_device = "/dev/ttyUSB0"
frequency_hz = 7040000  # 40m band
digital_mode = "PSK31"
tx_power_watts = 10

# Dial-up (legacy/emergency fallback)
[adapters.dialup]
enabled = false
modem_type = "GsmModule"
device_path = "/dev/ttyUSB1"
phone_number = "0800123456"
auto_dial_timeout_secs = 300
```

---

**Document Status:** Framework published for review and clarification phase
**Next Phase:** Implementation begins after design questions answered
