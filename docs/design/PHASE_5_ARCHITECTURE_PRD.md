# Phase 5 Architecture - Product Requirements & Implementation Details

**Version:** 1.0 (Based on Finalized Design Decisions)
**Date:** 2025-11-15
**Status:** Ready for Implementation

---

## Overview

This document provides detailed architectural specifications and implementation requirements for Phase 5, derived from finalized design decisions.

---

## Part 1: Power Management System

### 1.1 Power Supply Management

**Location:** `crates/myriadmesh-appliance/src/power/`

#### New Module: `power_supply.rs`

```rust
/// Power supply type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PowerSupply {
    /// Mains AC power - always available
    ACMains,

    /// Power over Ethernet
    PoE {
        available_watts: f32,
        reserved_watts: f32,  // For other devices on PoE network
    },

    /// Battery-powered device
    Battery {
        capacity_mwh: u32,
        current_mwh: u32,
        charge_rate_mwh_per_hour: f32,
        discharge_rate_mwh_per_hour: f32,
        low_power_threshold_percent: u8,   // Default: 20%
        critical_threshold_percent: u8,    // Default: 5%
    },
}

pub struct PowerManager {
    supply: Arc<RwLock<PowerSupply>>,
    scaling_table: HashMap<u8, u8>,  // battery% -> max_tx_power_dbm
    adapter_availability: HashMap<AdapterType, Vec<BatteryThreshold>>,
}

#[derive(Clone)]
pub struct BatteryThreshold {
    pub threshold_percent: u8,
    pub action: PowerAction,
}

pub enum PowerAction {
    FullPower,
    ReducePower { reduction_percent: u8 },
    DisableAdapter,
    ListenOnly,
}

impl PowerManager {
    /// Update battery state (called periodically)
    pub async fn update_battery_state(&self, mwh: u32) -> Result<()>;

    /// Get current power budget for adapter
    pub async fn get_power_budget(&self, adapter: AdapterType) -> Result<u32>;

    /// Check if adapter should be active
    pub async fn is_adapter_active(&self, adapter: AdapterType) -> Result<bool>;

    /// Get current tx power reduction factor (0.0 - 1.0)
    pub async fn get_power_scaling(&self) -> f64;

    /// Notify of significant power event
    pub async fn on_power_threshold_crossed(&self, threshold: u8);
}
```

#### Configuration Example

```yaml
# appliance_config.yaml

power:
  supply:
    type: battery
    capacity_mwh: 5000        # 5Ah @ 3.7V
    charge_rate: 2000         # mWh/hour charging
    discharge_rate: 500       # mWh/hour baseline
    low_power_threshold: 20
    critical_threshold: 5

  power_scaling:
    100: 30    # 100% battery = 30 dBm TX
    50: 25     # 50% battery = 25 dBm TX
    20: 20     # 20% battery = 20 dBm TX
    5: 10      # 5% battery = 10 dBm TX

  adapter_availability:
    Cellular:
      - threshold: 20
        action: { type: disable }
    FrsGmrs:
      - threshold: 5
        action: { type: listen_only }
    LoRa:
      - threshold: 5
        action: { type: reduce_power, percent: 50 }
```

---

### 1.2 Data Usage Management

**Location:** `crates/myriadmesh-network/src/data_usage.rs`

```rust
pub struct DataUsagePolicy {
    /// Is data limiting enabled? (default: false)
    pub enabled: bool,

    /// Warn when exceeding this (default: 1000 MB = 1 GB)
    pub warn_threshold_mb: u32,

    /// Hard limit in MB (0 = unlimited)
    pub hard_limit_mb: u32,

    /// Reset period
    pub reset_period: ResetPeriod,

    /// Current period start
    pub period_start: u64,
}

#[derive(Clone, Copy)]
pub enum ResetPeriod {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
}

pub struct DataUsageTracker {
    policy: Arc<RwLock<DataUsagePolicy>>,
    usage_mb: Arc<AtomicU32>,
}

impl DataUsageTracker {
    /// Check if transmission is allowed
    pub async fn check_quota(&self, size_mb: u32) -> Result<QuotaCheck> {
        match quotum_status {
            QuotaStatus::OK => Ok(QuotaCheck::Allow),
            QuotaStatus::Warning => {
                warn!("Data usage approaching limit");
                Ok(QuotaCheck::WarnButAllow)
            }
            QuotaStatus::Limited => {
                error!("Data usage limit exceeded");
                Err(NetworkError::DataLimitExceeded)
            }
        }
    }

    pub async fn add_usage(&self, bytes: u64);
    pub async fn get_usage_mb(&self) -> u32;
    pub async fn get_remaining_mb(&self) -> u32;
    pub async fn reset_if_needed(&self);
}
```

**Integration with Cellular Adapter:**
```rust
impl CellularAdapter {
    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()> {
        // Check data usage before transmission
        let frame_size = frame.to_bytes()?.len();
        self.data_tracker.check_quota((frame_size / 1024 / 1024) as u32).await?;

        // ... send frame ...

        // Track usage
        self.data_tracker.add_usage(frame_size as u64).await;
        Ok(())
    }
}
```

---

## Part 2: License Management System

### 2.1 License Verification

**Location:** `crates/myriadmesh-network/src/license.rs`

```rust
/// License state for radio adapters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LicenseState {
    /// Valid license with optional expiration
    Valid {
        callsign: String,
        license_class: LicenseClass,
        expires_at: Option<u64>,  // Unix timestamp
    },
    /// No license configured
    None,
    /// License has expired
    Expired { callsign: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseClass {
    Amateur(AmateurClass),
    GMRS,
    CB,
}

pub enum AmateurClass {
    Technician,
    General,
    Extra,
}

pub struct LicenseManager {
    state: Arc<RwLock<LicenseState>>,
    fcc_client: Option<FccClient>,  // For online verification
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

pub struct CacheEntry {
    callsign: String,
    valid: bool,
    cached_at: u64,
}

impl LicenseManager {
    /// Set license for this node
    pub async fn set_license(&self, callsign: String, class: LicenseClass) -> Result<()> {
        // Validate with FCC if online
        if let Some(ref fcc) = self.fcc_client {
            fcc.validate_callsign(&callsign).await?;
        }

        let mut state = self.state.write().await;
        *state = LicenseState::Valid {
            callsign,
            license_class: class,
            expires_at: None,  // Amateur licenses don't expire
        };
        Ok(())
    }

    /// Check if transmission is allowed
    pub async fn can_transmit(&self) -> Result<()> {
        let state = self.state.read().await;
        match *state {
            LicenseState::Valid { .. } => Ok(()),
            LicenseState::None => Err(NetworkError::LicenseRequired),
            LicenseState::Expired { ref callsign } => {
                Err(NetworkError::LicenseExpired(callsign.clone()))
            }
        }
    }

    /// Check if receive is allowed (always true)
    pub fn can_receive(&self) -> Result<()> {
        Ok(())  // Always allowed to listen
    }

    /// Get current license info
    pub async fn get_license(&self) -> LicenseState {
        self.state.read().await.clone()
    }
}

pub struct FccClient {
    // Integration with FCC license database
    // Fallback to local cache if offline
}
```

**Integration with Licensed Adapters:**

```rust
// In AprsAdapter
#[async_trait]
impl NetworkAdapter for AprsAdapter {
    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()> {
        // Check license before transmission
        self.license_manager.can_transmit().await?;

        // ... rest of send logic ...
    }

    async fn receive(&self, timeout_ms: u64) -> Result<(Address, Frame)> {
        // License not required for listening
        // ... receive logic ...
    }
}

// In HfRadioAdapter
#[async_trait]
impl NetworkAdapter for HfRadioAdapter {
    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()> {
        // Check license
        self.license_manager.can_transmit().await?;
        // ... transmit ...
    }
}
```

---

## Part 3: Advanced Fragmentation Strategy

### 3.1 Routing-Aware Fragmentation

**Location:** `crates/myriadmesh-routing/src/fragmentation.rs`

```rust
pub struct FragmentationDecision {
    /// Should fragment at router level?
    pub should_fragment: bool,

    /// Reason for decision
    pub reason: FragmentationReason,

    /// Preferred fragment size (if fragmenting)
    pub mtu: usize,
}

pub enum FragmentationReason {
    /// Message exceeds adapter MTU and won't be combined with others
    ExceedsMtu,

    /// Message under MTU, no fragmentation needed
    WithinMtu,

    /// Will be combined with other messages to same dest
    CombiningTransmissions,

    /// Adapter will handle own fragmentation
    AdapterHandled,
}

impl MessageRouter {
    pub async fn decide_fragmentation(
        &self,
        destination: NodeId,
        frame: &Frame,
        adapter: &dyn NetworkAdapter,
    ) -> Result<FragmentationDecision> {
        let mtu = adapter.get_capabilities().max_message_size;

        if frame.size() <= mtu {
            return Ok(FragmentationDecision {
                should_fragment: false,
                reason: FragmentationReason::WithinMtu,
                mtu,
            });
        }

        // Check if this message will be combined with others
        let route = self.dht.find_route(destination).await?;
        let queued_messages = self.get_queued_messages_to_destination(destination).await?;

        if route.would_combine && !queued_messages.is_empty() {
            // Don't fragment - let adapter/DHT handle batching
            return Ok(FragmentationDecision {
                should_fragment: false,
                reason: FragmentationReason::CombiningTransmissions,
                mtu,
            });
        }

        // Fragment at router level
        Ok(FragmentationDecision {
            should_fragment: true,
            reason: FragmentationReason::ExceedsMtu,
            mtu,
        })
    }

    pub async fn send_with_fragmentation(
        &self,
        destination: NodeId,
        frame: Frame,
        adapter_id: AdapterId,
    ) -> Result<()> {
        let adapter = self.adapters.get(adapter_id)?;
        let decision = self.decide_fragmentation(destination, &frame, adapter).await?;

        if decision.should_fragment {
            let fragments = fragment_frame(&frame, decision.mtu)?;
            for frag in fragments {
                // Send each fragment
                adapter.send(destination, &frag).await?;
            }
        } else {
            // Send whole frame
            adapter.send(destination, &frame).await?;
        }

        Ok(())
    }
}

pub fn fragment_frame(frame: &Frame, mtu: usize) -> Result<Vec<Frame>> {
    let serialized = frame.to_bytes()?;
    if serialized.len() <= mtu {
        return Ok(vec![frame.clone()]);
    }

    // Fragment format: [ID: 2B][Num: 1B][Total: 1B][Payload: variable]
    let header_size = 4;
    let payload_size = mtu - header_size;

    let message_id = rand::random::<u16>();
    let total_frags = (serialized.len() + payload_size - 1) / payload_size;

    let mut fragments = Vec::new();
    for frag_num in 0..total_frags {
        let start = frag_num * payload_size;
        let end = std::cmp::min(start + payload_size, serialized.len());

        let mut payload = Vec::with_capacity(mtu);
        payload.extend_from_slice(&message_id.to_be_bytes());
        payload.push(frag_num as u8);
        payload.push(total_frags as u8);
        payload.extend_from_slice(&serialized[start..end]);

        // Create fragment frame
        let frag_frame = Frame {
            is_fragment: true,
            fragment_data: Some(payload),
            ..frame.clone()
        };
        fragments.push(frag_frame);
    }

    Ok(fragments)
}
```

---

## Part 4: Space Weather Integration (HF Adapter)

### 4.1 Space Weather Data Fetching

**Location:** `crates/myriadmesh-network/src/adapters/hf_radio/space_weather.rs`

```rust
use async_trait::async_trait;
use reqwest::Client;

pub struct SpaceWeatherData {
    pub solar_flux_index: u16,      // SFI (0-300)
    pub k_index: u8,                // K-index (0-9)
    pub a_index: u16,               // A-index (0-400)
    pub updated_at: u64,            // Unix timestamp
    pub source: WeatherSource,
}

pub enum WeatherSource {
    NoaaSpc,        // Primary source
    VoacapOnline,   // Secondary source
    LocalCache,     // Fallback
}

#[async_trait]
pub trait SpaceWeatherProvider {
    async fn fetch_current(&self) -> Result<SpaceWeatherData>;
    async fn forecast_hourly(&self) -> Result<Vec<SpaceWeatherData>>;
}

pub struct NoaaSpcProvider {
    client: Client,
    cache: Arc<RwLock<Option<SpaceWeatherData>>>,
    cache_ttl: Duration,
}

impl NoaaSpcProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            cache: Arc::new(RwLock::new(None)),
            cache_ttl: Duration::from_secs(3600),  // 1 hour cache
        }
    }
}

#[async_trait]
impl SpaceWeatherProvider for NoaaSpcProvider {
    async fn fetch_current(&self) -> Result<SpaceWeatherData> {
        // Check cache first
        if let Some(data) = self.cache.read().await.as_ref() {
            if data.updated_at + self.cache_ttl.as_secs() > now() {
                return Ok(data.clone());
            }
        }

        // Fetch from NOAA SPC API
        let response = self.client
            .get("https://services.swpc.noaa.gov/products/space-weather-alerts.json")
            .send()
            .await?;

        let data = parse_noaa_response(response).await?;

        // Update cache
        *self.cache.write().await = Some(data.clone());
        Ok(data)
    }

    async fn forecast_hourly(&self) -> Result<Vec<SpaceWeatherData>> {
        // Fetch forecast from NOAA 3-day forecast
        let response = self.client
            .get("https://services.swpc.noaa.gov/json/forecast/geospace/3-day-forecast.json")
            .send()
            .await?;

        parse_noaa_forecast(response).await
    }
}

pub struct VoacapProvider {
    // Integration with VOACAP online service
}

pub struct LocalCacheProvider {
    cache_file: String,
}

impl LocalCacheProvider {
    pub fn new(cache_file: String) -> Self {
        Self { cache_file }
    }

    /// Update local cache from file
    pub async fn update_from_file(&self) -> Result<SpaceWeatherData> {
        let contents = tokio::fs::read_to_string(&self.cache_file).await?;
        serde_json::from_str(&contents).map_err(|e| NetworkError::DeserializationFailed(e.to_string()).into())
    }
}

pub struct MultiProviderWeather {
    primary: Box<dyn SpaceWeatherProvider>,
    secondary: Box<dyn SpaceWeatherProvider>,
    fallback: Box<dyn SpaceWeatherProvider>,
}

impl MultiProviderWeather {
    pub async fn fetch(&self) -> Result<SpaceWeatherData> {
        // Try primary provider
        match self.primary.fetch_current().await {
            Ok(data) => return Ok(data),
            Err(e) => warn!("Primary provider failed: {}", e),
        }

        // Try secondary
        match self.secondary.fetch_current().await {
            Ok(data) => return Ok(data),
            Err(e) => warn!("Secondary provider failed: {}", e),
        }

        // Use fallback
        self.fallback.fetch_current().await
    }
}
```

### 4.2 Band Selection Based on Space Weather

```rust
pub struct HfBandSelector {
    weather_provider: Arc<MultiProviderWeather>,
    band_suitability: HashMap<u16, BandCharacteristics>,
}

pub struct BandCharacteristics {
    pub frequency_mhz: u16,
    pub name: String,
    pub optimal_sfi_min: u16,
    pub optimal_sfi_max: u16,
    pub max_k_index: u8,
}

impl HfBandSelector {
    pub async fn select_band(&self) -> Result<u16> {
        let weather = self.weather_provider.fetch().await?;

        // Filter bands based on weather conditions
        let suitable: Vec<_> = self.band_suitability
            .iter()
            .filter(|(_, band)| {
                weather.solar_flux_index >= band.optimal_sfi_min &&
                weather.solar_flux_index <= band.optimal_sfi_max &&
                weather.k_index <= band.max_k_index
            })
            .collect();

        if suitable.is_empty() {
            // Default to 20m band as fallback
            return Ok(14_000);  // 14 MHz
        }

        // Return first suitable band (sorted by frequency)
        Ok(suitable[0].0)
    }

    pub async fn get_band_quality(&self, frequency_mhz: u16) -> Result<f64> {
        let weather = self.weather_provider.fetch().await?;
        let band = self.band_suitability.get(&frequency_mhz)
            .ok_or(NetworkError::UnknownBand)?;

        // Quality 0.0 - 1.0 based on how well conditions match band
        let sfi_match = if weather.solar_flux_index >= band.optimal_sfi_min &&
                           weather.solar_flux_index <= band.optimal_sfi_max {
            1.0
        } else {
            0.5
        };

        let k_match = if weather.k_index <= band.max_k_index {
            1.0
        } else {
            0.2
        };

        Ok(sfi_match * 0.6 + k_match * 0.4)
    }
}
```

---

## Part 5: Plugin Architecture

### 5.1 Plugin System Foundation

**Location:** `crates/myriadmesh-core/src/plugin.rs`

```rust
use std::any::Any;

/// Base plugin trait
pub trait MyriadMeshPlugin: Send + Sync {
    fn plugin_name(&self) -> &str;
    fn plugin_version(&self) -> &str;
    fn author(&self) -> &str;
    fn description(&self) -> &str;

    async fn initialize(&mut self, config: PluginConfig) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;

    fn as_any(&self) -> &dyn Any;
}

pub struct PluginConfig {
    pub name: String,
    pub version: String,
    pub config_data: serde_json::Value,
}

/// Adapter plugin - adds network transport capability
pub trait AdapterPlugin: MyriadMeshPlugin + NetworkAdapter {
    fn hardware_requirements(&self) -> Vec<String>;
    fn dependencies(&self) -> Vec<PluginDependency>;
}

pub struct PluginDependency {
    pub name: String,
    pub min_version: String,
}

/// Application plugin - high-level functionality
pub trait ApplicationPlugin: MyriadMeshPlugin {
    fn register_message_handler(&self) -> Option<MessageHandler>;
    fn provide_rest_endpoints(&self) -> Vec<RestEndpoint>;
    fn provide_ui_components(&self) -> Vec<UiComponent>;
}

/// Bridge plugin - connects to external networks
pub trait BridgePlugin: MyriadMeshPlugin {
    fn bridge_name(&self) -> &str;
    fn supported_networks(&self) -> Vec<String>;

    async fn translate_inbound(&self, from_network: &str, data: &[u8]) -> Result<Frame>;
    async fn translate_outbound(&self, frame: &Frame, to_network: &str) -> Result<Vec<u8>>;
}

pub struct RestEndpoint {
    pub path: String,
    pub method: HttpMethod,
    pub handler: Arc<dyn Fn() + Send + Sync>,
}

pub struct MessageHandler {
    pub message_type: u8,
    pub handler: Arc<dyn Fn(&Frame) -> Result<()> + Send + Sync>,
}

pub struct UiComponent {
    pub component_id: String,
    pub title: String,
    pub component_type: ComponentType,
}

pub enum ComponentType {
    Dashboard,
    Settings,
    Status,
    Custom(String),
}

pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
}
```

### 5.2 Plugin Registry

**Location:** `crates/myriadmesh-core/src/plugin_registry.rs`

```rust
pub struct PluginRegistry {
    adapters: Arc<RwLock<HashMap<String, Arc<dyn AdapterPlugin>>>>,
    applications: Arc<RwLock<HashMap<String, Arc<dyn ApplicationPlugin>>>>,
    bridges: Arc<RwLock<HashMap<String, Arc<dyn BridgePlugin>>>>,
    plugin_dir: PathBuf,
}

impl PluginRegistry {
    pub fn new(plugin_dir: PathBuf) -> Self {
        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            applications: Arc::new(RwLock::new(HashMap::new())),
            bridges: Arc::new(RwLock::new(HashMap::new())),
            plugin_dir,
        }
    }

    /// Load plugin from shared library
    pub async fn load_adapter_plugin(&self, plugin_path: &str) -> Result<()> {
        // Load .so/.dylib/.dll
        // Instantiate and validate
        // Register in adapter registry
        unimplemented!("Dynamic plugin loading")
    }

    /// Register built-in core adapter
    pub async fn register_core_adapter(&self, adapter: Arc<dyn AdapterPlugin>) -> Result<()> {
        let name = adapter.plugin_name().to_string();
        self.adapters.write().await.insert(name, adapter);
        Ok(())
    }

    /// List all registered adapters
    pub async fn list_adapters(&self) -> Vec<String> {
        self.adapters.read().await.keys().cloned().collect()
    }

    /// Get adapter plugin by name
    pub async fn get_adapter(&self, name: &str) -> Option<Arc<dyn AdapterPlugin>> {
        self.adapters.read().await.get(name).cloned()
    }

    // Similar methods for applications and bridges...
}
```

---

## Part 6: Interoperability Bridges

### 6.1 Meshtastic Bridge

**Location:** `crates/myriadmesh-network/src/bridges/meshtastic.rs`

```rust
pub struct MeshtasticBridge {
    /// Translation table between protocols
    translator: FrameTranslator,
}

pub struct FrameTranslator;

impl FrameTranslator {
    /// Convert MyriadMesh frame to Meshtastic packet format
    pub fn encode_meshtastic(frame: &Frame) -> Result<Vec<u8>> {
        // Meshtastic packet format:
        // [Header: variable]
        // [Encrypted payload]
        // [CRC: 2 bytes]
        unimplemented!("Meshtastic packet encoding")
    }

    /// Convert Meshtastic packet to MyriadMesh frame
    pub fn decode_meshtastic(data: &[u8]) -> Result<Frame> {
        // Parse Meshtastic header
        // Extract payload
        // Create MyriadMesh frame
        unimplemented!("Meshtastic packet decoding")
    }
}

pub struct MeshtasticRelayNode {
    /// Routes packets between Meshtastic and MyriadMesh networks
    bridge: MeshtasticBridge,
    message_router: Arc<MessageRouter>,
}

impl MeshtasticRelayNode {
    pub async fn relay_from_meshtastic(&self, packet: &[u8]) -> Result<()> {
        let frame = MeshtasticBridge::decode_meshtastic(packet)?;
        self.message_router.relay_message(frame).await?;
        Ok(())
    }

    pub async fn relay_to_meshtastic(&self, frame: &Frame) -> Result<Vec<u8>> {
        MeshtasticBridge::encode_meshtastic(frame)
    }
}
```

### 6.2 APRS-IS Gateway with Advanced Scheduling

**Location:** `crates/myriadmesh-network/src/bridges/aprs_gateway.rs`

```rust
pub struct AprsGateway {
    aprs_is_client: Arc<AprsIsClient>,
    scheduler: Arc<PacketScheduler>,
    message_router: Arc<MessageRouter>,
}

pub struct PacketScheduler {
    /// Avoid transmissions during these periods
    quiet_periods: Arc<RwLock<Vec<QuietPeriod>>>,

    /// Optimal windows for specific callsigns
    optimal_windows: Arc<RwLock<HashMap<String, TimeWindow>>>,

    /// Retry schedule for failed deliveries
    retry_backoff: ExponentialBackoff,

    /// Collision avoidance for shared frequencies
    frequency_slots: Arc<RwLock<HashMap<u32, TimeSlot>>>,
}

pub struct QuietPeriod {
    pub start_hour: u8,
    pub end_hour: u8,
    pub adapters: Vec<AdapterType>,
}

pub struct TimeWindow {
    pub start_hour: u8,
    pub end_hour: u8,
    pub priority: Priority,
}

pub struct TimeSlot {
    pub frequency_hz: u32,
    pub slot_start_sec: u8,
    pub slot_duration_sec: u8,
    pub reserved_for: Option<String>,  // Callsign if reserved
}

impl PacketScheduler {
    pub async fn schedule_transmission(
        &self,
        from_callsign: &str,
        to_callsign: &str,
        priority: Priority,
    ) -> Result<TransmissionWindow> {
        // Find optimal time slot
        // Avoid quiet periods
        // Avoid frequency conflicts
        // Consider callsign-specific windows
        unimplemented!("Transmission scheduling")
    }

    pub async fn can_transmit_now(&self, frequency_hz: u32) -> Result<bool> {
        // Check if frequency is available
        let slots = self.frequency_slots.read().await;
        Ok(slots.get(&frequency_hz).map_or(true, |slot| {
            // Check if current time is within slot
            let now_sec = (now() % 3600) as u8;
            now_sec >= slot.slot_start_sec &&
            now_sec < (slot.slot_start_sec + slot.slot_duration_sec)
        }))
    }

    pub async fn register_quiet_period(
        &self,
        start_hour: u8,
        end_hour: u8,
        adapters: Vec<AdapterType>,
    ) -> Result<()> {
        self.quiet_periods.write().await.push(QuietPeriod {
            start_hour,
            end_hour,
            adapters,
        });
        Ok(())
    }
}

pub struct ExponentialBackoff {
    initial_delay_ms: u32,
    max_delay_ms: u32,
    current_attempt: u32,
}

impl ExponentialBackoff {
    pub fn next_retry_delay(&mut self) -> Duration {
        let delay = (self.initial_delay_ms as f64 * 2_f64.powi(self.current_attempt as i32))
            .min(self.max_delay_ms as f64) as u64;
        self.current_attempt += 1;
        Duration::from_millis(delay)
    }
}

impl AprsGateway {
    pub async fn relay_from_aprs_is(&self, data: &[u8]) -> Result<()> {
        // Parse APRS-IS packet
        // Convert to MyriadMesh frame
        // Route via message router
        unimplemented!("APRS-IS inbound relay")
    }

    pub async fn relay_to_aprs_is(&self, frame: &Frame) -> Result<()> {
        // Check if transmission is scheduled
        let window = self.scheduler.schedule_transmission(
            &frame.source_callsign(),
            &frame.dest_callsign(),
            frame.priority(),
        ).await?;

        // Wait until scheduled time
        tokio::time::sleep(window.wait_duration).await;

        // Send to APRS-IS network
        self.aprs_is_client.send(frame).await?;
        Ok(())
    }
}
```

---

## Part 7: Documentation Architecture

### 7.1 Documentation Structure

```
docs/
├── guides/
│   ├── adapters/
│   │   ├── lora/
│   │   │   ├── 01-quickstart.md           (Beginner)
│   │   │   ├── 02-configuration.md        (Intermediate)
│   │   │   ├── 03-troubleshooting.md      (Advanced)
│   │   │   └── vendor/
│   │   │       ├── heltec-wireless-stick.md
│   │   │       └── ttgo-lora.md
│   │   ├── aprs/
│   │   │   └── ... (same structure)
│   │   └── ... (for each adapter)
│   ├── concepts/
│   │   ├── mesh-networking.md
│   │   ├── adapter-selection.md
│   │   ├── power-management.md
│   │   └── licensing.md
│   └── troubleshooting/
│       └── common-issues.md
│
├── api/
│   ├── adapters.md                        (Auto-generated)
│   ├── routing.md                         (Auto-generated)
│   ├── message-format.md                  (Manual)
│   └── examples/
│       ├── basic-messaging.rs
│       ├── adapter-selection.rs
│       └── custom-routing.rs
│
├── hardware/
│   ├── lora-setup.md
│   ├── aprs-tnc-setup.md
│   ├── hf-radio-cat-control.md
│   └── power-supplies.md
│
└── plugin-development/
    ├── plugin-api.md
    ├── adapter-plugin.md
    ├── application-plugin.md
    └── examples/
        └── custom-adapter.rs
```

### 7.2 Multi-Level Documentation Pattern

Each adapter guide follows this pattern:

**Quickstart (Beginner, 5 min read)**
```
# LoRaWAN Quick Start

## What You Need
- [ ] LoRa device (Heltec/TTGO)
- [ ] USB cable
- [ ] 5 minutes

## Basic Setup
1. Plug in device
2. Run: `myriadnode --setup-lora`
3. Done!

## Send First Message
[Simple example with defaults]
```

**Configuration (Intermediate, 30 min)**
```
# LoRaWAN Configuration Guide

## Understanding the Parameters
- Spreading Factor (SF)
- Bandwidth
- Transmit Power

[Detailed explanation of each]

## Customizing Your Setup
[Configuration examples]

## Testing Your Configuration
[Validation steps]
```

**Advanced (30+ min)**
```
# LoRaWAN Advanced Topics

## Performance Tuning
## Duty Cycle Optimization
## Long-Range Considerations
## Interference Mitigation

[In-depth technical discussion]
```

---

## Implementation Timeline - Updated

### Phase 5a: Foundation (Months 14-15)

**Week 1-2: Power Management System**
- Implement PowerManager
- Add to appliance stack
- Integrate with adapters
- Tests: 100% coverage

**Week 3-4: License Management System**
- Implement LicenseManager
- APRS/HF integration
- FCC validation
- Tests: 100% coverage

**Week 5: Plugin Architecture Foundation**
- Implement plugin traits
- Create plugin registry
- Plugin loading system
- Tests: 80% coverage

**Week 6: Documentation Infrastructure**
- Set up multi-tier guide structure
- Create templates
- Auto-API generation setup

### Phase 5b: Core Adapters (Months 15-16)

**Parallel work across all adapters:**

**LoRaWAN (Highest Priority)**
- Full implementation
- Hardware testing (if available)
- Mock adapter for CI/CD
- Comprehensive documentation (all tiers + vendor guides)

**APRS (High Priority)**
- Full AX.25/KISS implementation
- License checking
- Documentation
- Mock TNC for testing

**FRS/GMRS**
- Full modem implementation
- PTT control
- Documentation

**Others (Lower Priority)**
- Full implementations
- Mocks where hardware unavailable
- Documentation

### Phase 5c: Advanced Features (Months 16-17)

- HF Radio implementation + space weather
- WiFi HaLoW (or mock)
- Dial-up (legacy)
- Plugin system testing
- Advanced documentation

### Phase 5d: Bridges & Interop (Months 17-18)

- Meshtastic bridge
- APRS-IS gateway with scheduling
- Documentation completion
- Community testing

---

## Success Metrics

**Adapter Coverage:**
- ✅ 6/6 adapters with full implementations
- ✅ 100% unit test coverage for adapters
- ✅ Integration tests passing
- ✅ Mock adapters for unavailable hardware

**Power Management:**
- ✅ Adaptive power scaling working
- ✅ Battery monitoring accurate
- ✅ Data usage tracking correct
- ✅ Tests: 95%+ coverage

**License System:**
- ✅ Transmission blocking for unlicensed
- ✅ Receive always allowed
- ✅ FCC validation (if online)
- ✅ Fallback cache working

**Fragmentation:**
- ✅ Routing-aware decisions
- ✅ Both adapter and router handling
- ✅ Reassembly reliable
- ✅ All MTU sizes supported

**Space Weather (HF):**
- ✅ NOAA data fetching
- ✅ Band selection working
- ✅ Fallback cache functional
- ✅ Quality estimation accurate

**Plugin System:**
- ✅ Core plugins registered
- ✅ Plugin loading functional
- ✅ Plugin lifecycle management
- ✅ Plugin tests: 80%+ coverage

**Bridges:**
- ✅ Meshtastic packet translation
- ✅ APRS-IS gateway functional
- ✅ Scheduling working
- ✅ Relay tests: 90%+ coverage

**Documentation:**
- ✅ Beginner guides (all adapters)
- ✅ Intermediate guides (all adapters)
- ✅ Advanced guides (all adapters)
- ✅ Vendor-specific guides (stretch goal)
- ✅ API docs (auto-generated + examples)
- ✅ Hardware setup guides

---

**Status:** Phase 5 Architecture Fully Specified - Ready for Implementation

This PRD provides all technical details needed for Phase 5 implementation teams.
