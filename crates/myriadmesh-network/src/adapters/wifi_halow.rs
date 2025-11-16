//! Wi-Fi HaLoW (802.11ah) network adapter
//!
//! Provides energy-efficient long-range WiFi connectivity.
//! Operates in sub-1 GHz bands with range up to 1 km.
//!
//! # Features
//!
//! - 802.11ah protocol (HaLoW = High efficiency, Long range, Low power)
//! - TWT (Target Wake Time) for 80%+ power reduction
//! - Sub-1 GHz operation (900 MHz in US)
//! - Range: 1-10 km (vs 100m for traditional WiFi)
//! - Supports thousands of connected devices
//! - Mock implementation (hardware not widely available)

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::{types::AdapterType, Frame, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

type FrameReceiver = Arc<RwLock<Option<mpsc::Receiver<(Address, Frame)>>>>;

/// WiFi HaLoW adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiHalowConfig {
    /// SSID to connect to
    pub ssid: String,
    /// Passphrase (WPA2/WPA3)
    pub password: Option<String>,
    /// Channel number (varies by region, typically 1-59)
    pub channel: u8,
    /// Enable power save mode with Target Wake Time
    pub power_save: bool,
    /// Target Wake Time interval in milliseconds (100-1000ms typical)
    pub twt_interval_ms: u32,
    /// Wireless adapter interface name
    pub interface: String,
    /// MAC address (6 bytes, colon-separated)
    pub mac_address: String,
    /// Operating bandwidth (1, 2, 4, 8, or 16 MHz)
    pub bandwidth_mhz: u8,
}

impl Default for WifiHalowConfig {
    fn default() -> Self {
        Self {
            ssid: "mesh-network".to_string(),
            password: Some("secure-password".to_string()),
            channel: 1,
            power_save: true,
            twt_interval_ms: 500, // Wake every 500ms
            interface: "wlan0".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            bandwidth_mhz: 2, // 2 MHz channel (compromise: range vs speed)
        }
    }
}

impl WifiHalowConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Check channel range
        if self.channel == 0 || self.channel > 59 {
            return Err(NetworkError::Other("Channel must be 1-59".to_string()));
        }

        // Check TWT interval
        if self.power_save && (self.twt_interval_ms < 10 || self.twt_interval_ms > 10000) {
            return Err(NetworkError::Other(
                "TWT interval must be 10-10000ms".to_string(),
            ));
        }

        // Check bandwidth
        if ![1, 2, 4, 8, 16].contains(&self.bandwidth_mhz) {
            return Err(NetworkError::Other(
                "Bandwidth must be 1, 2, 4, 8, or 16 MHz".to_string(),
            ));
        }

        Ok(())
    }

    /// Get theoretical data rate based on bandwidth
    pub fn get_data_rate(&self) -> u32 {
        // Approximate MCS rates for 802.11ah
        match self.bandwidth_mhz {
            1 => 300_000,    // 300 kbps
            2 => 650_000,    // 650 kbps
            4 => 1_300_000,  // 1.3 Mbps
            8 => 3_000_000,  // 3 Mbps
            16 => 7_800_000, // 7.8 Mbps
            _ => 650_000,    // Default to 2 MHz
        }
    }

    /// Calculate power savings from TWT
    pub fn get_power_savings(&self) -> f32 {
        if !self.power_save {
            return 0.0;
        }

        // Power savings = (sleep_time / total_time)
        // Assuming 10ms active window per TWT interval
        let active_ms = 10.0;
        let total_ms = self.twt_interval_ms as f32;
        ((total_ms - active_ms) / total_ms) * 100.0
    }
}

/// Internal WiFi HaLoW state
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct WifiHalowState {
    connected: bool,
    rssi_dbm: Option<i16>,
    link_rate_mbps: f32,
    power_save_active: bool,
    last_beacon: Option<Instant>,
    associated_peers: HashMap<String, Instant>, // MAC -> last seen
}

/// TWT (Target Wake Time) session
#[derive(Debug, Clone)]
struct TwtSession {
    wake_interval_ms: u32,
    wake_duration_ms: u32,
    next_wake: Instant,
    enabled: bool,
}

impl TwtSession {
    fn new(interval_ms: u32) -> Self {
        Self {
            wake_interval_ms: interval_ms,
            wake_duration_ms: 10, // 10ms wake window
            next_wake: Instant::now() + Duration::from_millis(interval_ms as u64),
            enabled: true,
        }
    }

    /// Check if we should be awake now
    fn is_awake(&self) -> bool {
        if !self.enabled {
            return true;
        }

        let now = Instant::now();
        now < self.next_wake + Duration::from_millis(self.wake_duration_ms as u64)
    }

    /// Update wake schedule
    fn update(&mut self) {
        let now = Instant::now();
        if now >= self.next_wake {
            self.next_wake = now + Duration::from_millis(self.wake_interval_ms as u64);
        }
    }
}

/// Mock WiFi network stack
trait NetworkStack: Send + Sync {
    fn connect(&mut self, ssid: &str, password: Option<&str>) -> Result<()>;
    fn disconnect(&mut self) -> Result<()>;
    fn send_frame(&mut self, dest_mac: &str, data: &[u8]) -> Result<()>;
    fn receive_frame(&mut self) -> Result<Option<Vec<u8>>>;
    fn get_rssi(&self) -> Option<i16>;
    fn scan_networks(&self) -> Result<Vec<String>>;
}

/// Mock 802.11ah network stack
struct MockNetworkStack {
    connected: bool,
    ssid: String,
    tx_buffer: Vec<Vec<u8>>,
    rx_buffer: Vec<Vec<u8>>,
}

impl MockNetworkStack {
    fn new() -> Self {
        Self {
            connected: false,
            ssid: String::new(),
            tx_buffer: Vec::new(),
            rx_buffer: Vec::new(),
        }
    }
}

impl NetworkStack for MockNetworkStack {
    fn connect(&mut self, ssid: &str, _password: Option<&str>) -> Result<()> {
        self.ssid = ssid.to_string();
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    fn send_frame(&mut self, _dest_mac: &str, data: &[u8]) -> Result<()> {
        if !self.connected {
            return Err(NetworkError::Other("Not connected".to_string()));
        }
        self.tx_buffer.push(data.to_vec());
        Ok(())
    }

    fn receive_frame(&mut self) -> Result<Option<Vec<u8>>> {
        if self.rx_buffer.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.rx_buffer.remove(0)))
        }
    }

    fn get_rssi(&self) -> Option<i16> {
        if self.connected {
            Some(-70) // Mock RSSI
        } else {
            None
        }
    }

    fn scan_networks(&self) -> Result<Vec<String>> {
        Ok(vec!["mesh-network".to_string(), "halow-test".to_string()])
    }
}

/// WiFi HaLoW adapter
pub struct WifiHalowAdapter {
    config: WifiHalowConfig,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    state: Arc<RwLock<WifiHalowState>>,
    rx: FrameReceiver,
    incoming_tx: mpsc::Sender<(Address, Frame)>,
    rx_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    network_stack: Arc<RwLock<Box<dyn NetworkStack>>>,
    twt_session: Arc<RwLock<Option<TwtSession>>>,
}

impl WifiHalowAdapter {
    pub fn new(config: WifiHalowConfig) -> Self {
        let data_rate = config.get_data_rate();

        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::WiFiHaLoW,
            max_message_size: 1500, // Standard MTU
            typical_latency_ms: 50.0,
            typical_bandwidth_bps: data_rate as u64,
            reliability: 0.97,
            range_meters: 5000.0, // 5 km typical (vs 100m WiFi)
            power_consumption: if config.power_save {
                PowerConsumption::VeryLow // With TWT
            } else {
                PowerConsumption::Low
            },
            cost_per_mb: 0.0,
            supports_broadcast: true,
            supports_multicast: true,
        };

        // RESOURCE M3: Bounded channel to prevent memory exhaustion
        // Ethernet/Wi-Fi: 10,000 capacity (high throughput)
        let (incoming_tx, incoming_rx) = mpsc::channel(10000);

        Self {
            config,
            status: Arc::new(RwLock::new(AdapterStatus::Uninitialized)),
            capabilities,
            state: Arc::new(RwLock::new(WifiHalowState {
                connected: false,
                rssi_dbm: None,
                link_rate_mbps: (data_rate as f32) / 1_000_000.0,
                power_save_active: false,
                last_beacon: None,
                associated_peers: HashMap::new(),
            })),
            rx: Arc::new(RwLock::new(Some(incoming_rx))),
            incoming_tx,
            rx_task: Arc::new(RwLock::new(None)),
            network_stack: Arc::new(RwLock::new(Box::new(MockNetworkStack::new()))),
            twt_session: Arc::new(RwLock::new(None)),
        }
    }

    /// Connect to HaLoW network
    async fn connect_to_network(&self) -> Result<()> {
        let mut stack = self.network_stack.write().await;
        stack.connect(&self.config.ssid, self.config.password.as_deref())?;

        let mut state = self.state.write().await;
        state.connected = true;
        state.rssi_dbm = stack.get_rssi();

        log::info!("Connected to HaLoW network: {}", self.config.ssid);
        Ok(())
    }

    /// Configure Target Wake Time for power savings
    async fn configure_twt(&self) -> Result<()> {
        if !self.config.power_save {
            return Ok(());
        }

        let session = TwtSession::new(self.config.twt_interval_ms);
        *self.twt_session.write().await = Some(session);

        self.state.write().await.power_save_active = true;

        let savings = self.config.get_power_savings();
        log::info!(
            "TWT configured: {}ms interval, {:.1}% power savings",
            self.config.twt_interval_ms,
            savings
        );

        Ok(())
    }
}

#[async_trait::async_trait]
impl NetworkAdapter for WifiHalowAdapter {
    async fn initialize(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::Initializing;

        // Validate configuration
        self.config.validate()?;

        // Connect to network
        self.connect_to_network().await?;

        // Configure TWT if power save enabled
        if self.config.power_save {
            self.configure_twt().await?;
        }

        log::info!(
            "WiFi HaLoW initialized: SSID={}, channel={}, {}MHz BW",
            self.config.ssid,
            self.config.channel,
            self.config.bandwidth_mhz
        );

        *self.status.write().await = AdapterStatus::Ready;
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        if *self.status.read().await != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }

        // Spawn RX task
        let incoming_tx = self.incoming_tx.clone();
        let network_stack = self.network_stack.clone();
        let twt_session = self.twt_session.clone();

        let rx_task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;

                // Check TWT wake schedule
                if let Some(ref mut twt) = *twt_session.write().await {
                    if !twt.is_awake() {
                        twt.update();
                        continue; // Sleeping
                    }
                }

                // Receive frames
                if let Ok(Some(data)) = network_stack.write().await.receive_frame() {
                    if let Ok(frame) = bincode::deserialize::<Frame>(&data) {
                        let addr = Address::WifiHaLow("00:00:00:00:00:00".to_string());
                        let _ = incoming_tx.send((addr, frame));
                    }
                }
            }
        });

        *self.rx_task.write().await = Some(rx_task);
        log::info!("WiFi HaLoW adapter started");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if let Some(h) = self.rx_task.write().await.take() {
            h.abort();
        }

        self.network_stack.write().await.disconnect()?;
        *self.status.write().await = AdapterStatus::ShuttingDown;
        log::info!("WiFi HaLoW adapter stopped");
        Ok(())
    }

    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()> {
        let data = bincode::serialize(frame)
            .map_err(|e| NetworkError::Other(format!("Serialization failed: {}", e)))?;

        // Check TWT wake schedule
        if let Some(ref twt) = *self.twt_session.read().await {
            if !twt.is_awake() {
                // Wait for next wake window
                return Err(NetworkError::Other("TWT sleep active".to_string()));
            }
        }

        // Extract MAC address from destination
        let dest_mac = match destination {
            Address::WifiHaLow(mac) => mac.as_str(),
            _ => "FF:FF:FF:FF:FF:FF", // Broadcast
        };

        self.network_stack
            .write()
            .await
            .send_frame(dest_mac, &data)?;

        Ok(())
    }

    async fn receive(&self, timeout_ms: u64) -> Result<(Address, Frame)> {
        let mut rx_guard = self.rx.write().await;
        if let Some(rx) = rx_guard.as_mut() {
            tokio::select! {
                result = rx.recv() => {
                    result.ok_or(NetworkError::ReceiveFailed("Channel closed".to_string()))
                }
                _ = tokio::time::sleep(Duration::from_millis(timeout_ms)) => {
                    Err(NetworkError::Timeout)
                }
            }
        } else {
            Err(NetworkError::AdapterNotReady)
        }
    }

    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        // Simplified peer discovery via network scan
        let networks = self.network_stack.read().await.scan_networks()?;

        let peers = networks
            .into_iter()
            .map(|ssid| PeerInfo {
                node_id: NodeId::from_bytes([0u8; 64]), // Mock
                address: Address::WifiHaLow(format!("00:00:00:00:00:00@{}", ssid)),
            })
            .collect();

        Ok(peers)
    }

    fn get_status(&self) -> AdapterStatus {
        self.status
            .try_read()
            .map(|s| *s)
            .unwrap_or(AdapterStatus::Uninitialized)
    }

    fn get_capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    async fn test_connection(&self, _destination: &Address) -> Result<TestResults> {
        let rssi = self.network_stack.read().await.get_rssi();

        Ok(TestResults {
            success: rssi.is_some(),
            rtt_ms: Some(50.0), // Typical latency
            error: if rssi.is_none() {
                Some("Not connected".to_string())
            } else {
                None
            },
        })
    }

    fn get_local_address(&self) -> Option<Address> {
        Some(Address::WifiHaLow(self.config.mac_address.clone()))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        // Parse "halow://mac_address@ssid" or just "mac_address"
        let mac = addr_str
            .strip_prefix("halow://")
            .unwrap_or(addr_str)
            .split('@')
            .next()
            .unwrap_or(addr_str);

        Ok(Address::WifiHaLow(mac.to_string()))
    }

    fn supports_address(&self, address: &Address) -> bool {
        matches!(address, Address::WifiHaLow(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wifi_halow_creation() {
        let config = WifiHalowConfig::default();
        let adapter = WifiHalowAdapter::new(config);
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
    }

    #[test]
    fn test_wifi_halow_capabilities() {
        let adapter = WifiHalowAdapter::new(WifiHalowConfig::default());
        let caps = adapter.get_capabilities();

        assert_eq!(caps.adapter_type, AdapterType::WiFiHaLoW);
        assert_eq!(caps.max_message_size, 1500);
        assert_eq!(caps.range_meters, 5000.0);
    }

    #[test]
    fn test_wifi_halow_config_defaults() {
        let config = WifiHalowConfig::default();
        assert_eq!(config.ssid, "mesh-network");
        assert!(config.password.is_some());
        assert!(config.power_save);
        assert_eq!(config.bandwidth_mhz, 2);
    }

    #[test]
    fn test_data_rate_calculation() {
        let config_1mhz = WifiHalowConfig {
            bandwidth_mhz: 1,
            ..Default::default()
        };
        assert_eq!(config_1mhz.get_data_rate(), 300_000);

        let config_16mhz = WifiHalowConfig {
            bandwidth_mhz: 16,
            ..Default::default()
        };
        assert_eq!(config_16mhz.get_data_rate(), 7_800_000);
    }

    #[test]
    fn test_power_savings_calculation() {
        let config = WifiHalowConfig {
            power_save: true,
            twt_interval_ms: 1000,
            ..Default::default()
        };
        let savings = config.get_power_savings();
        assert!(savings > 90.0); // >90% savings with 1000ms interval

        let config_disabled = WifiHalowConfig {
            power_save: false,
            ..Default::default()
        };
        assert_eq!(config_disabled.get_power_savings(), 0.0);
    }

    #[test]
    fn test_config_validation() {
        let valid_config = WifiHalowConfig::default();
        assert!(valid_config.validate().is_ok());

        let invalid_channel = WifiHalowConfig {
            channel: 100, // Invalid
            ..Default::default()
        };
        assert!(invalid_channel.validate().is_err());

        let invalid_bandwidth = WifiHalowConfig {
            bandwidth_mhz: 3, // Invalid (must be 1,2,4,8,16)
            ..Default::default()
        };
        assert!(invalid_bandwidth.validate().is_err());
    }

    #[test]
    fn test_twt_session() {
        let mut twt = TwtSession::new(100);
        assert!(twt.enabled);
        assert_eq!(twt.wake_interval_ms, 100);
        assert_eq!(twt.wake_duration_ms, 10);

        // Should be awake initially
        assert!(twt.is_awake());

        // Update schedule
        twt.update();
    }

    #[test]
    fn test_mock_network_stack() {
        let mut stack = MockNetworkStack::new();

        // Not connected initially
        assert!(!stack.connected);
        assert!(stack.send_frame("00:11:22:33:44:55", b"test").is_err());

        // Connect
        assert!(stack.connect("test-ssid", Some("password")).is_ok());
        assert!(stack.connected);

        // Can send when connected
        assert!(stack.send_frame("00:11:22:33:44:55", b"test").is_ok());
        assert_eq!(stack.tx_buffer.len(), 1);

        // RSSI available when connected
        assert!(stack.get_rssi().is_some());

        // Scan networks
        let networks = stack.scan_networks().unwrap();
        assert!(!networks.is_empty());
    }

    #[test]
    fn test_address_parsing() {
        let adapter = WifiHalowAdapter::new(WifiHalowConfig::default());

        let addr1 = adapter.parse_address("00:11:22:33:44:55").unwrap();
        assert!(matches!(addr1, Address::WifiHaLow(_)));

        let addr2 = adapter
            .parse_address("halow://00:11:22:33:44:55@mesh-network")
            .unwrap();
        assert!(matches!(addr2, Address::WifiHaLow(_)));
    }

    #[tokio::test]
    async fn test_adapter_initialization() {
        let mut adapter = WifiHalowAdapter::new(WifiHalowConfig::default());
        let result = adapter.initialize().await;
        assert!(result.is_ok());
        assert_eq!(adapter.get_status(), AdapterStatus::Ready);
    }

    #[tokio::test]
    async fn test_twt_configuration() {
        let config = WifiHalowConfig {
            power_save: true,
            twt_interval_ms: 500,
            ..Default::default()
        };
        let adapter = WifiHalowAdapter::new(config);

        assert!(adapter.configure_twt().await.is_ok());
        assert!(adapter.twt_session.read().await.is_some());
    }
}
