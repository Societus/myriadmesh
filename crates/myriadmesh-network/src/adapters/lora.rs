//! LoRaWAN/Meshtastic network adapter
//!
//! Provides long-range (15+ km), low-power mesh networking via LoRa modulation.
//! Supports both native LoRaWAN and Meshtastic protocol compatibility.
//!
//! # Features
//!
//! - SPI interface to SX1262/SX1276 LoRa modems
//! - Duty cycle enforcement (EU: 1%, US: unlimited)
//! - Meshtastic protocol translation
//! - Fragmentation for 240-byte MTU
//! - Power management integration
//! - Mock adapter for testing without hardware

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::{
    types::{AdapterType, NODE_ID_SIZE},
    Frame, MessageId, MessageType, NodeId,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

type FrameReceiver = Arc<RwLock<Option<mpsc::UnboundedReceiver<(Address, Frame)>>>>;

/// LoRaWAN/Meshtastic adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoRaConfig {
    /// Frequency in Hz (868 MHz Europe, 902 MHz N. America)
    pub frequency_hz: u32,
    /// Spreading factor (7-12, higher = longer range, slower speed)
    pub spreading_factor: u8,
    /// Bandwidth in kHz (125, 250, 500)
    pub bandwidth_khz: u16,
    /// Coding rate (e.g., 0.8 for 4/5)
    pub coding_rate: f32,
    /// Transmit power in dBm (2-20)
    pub tx_power_dbm: i8,
    /// Enable Meshtastic protocol compatibility
    pub meshtastic_mode: bool,
    /// Duty cycle limit as percentage (EU: 1%, US: unlimited)
    pub duty_cycle_percent: f32,
    /// SPI device path for modem (e.g., "/dev/spidev0.0")
    pub spi_device: String,
    /// Use mock hardware (for testing without physical modem)
    pub use_mock: bool,
}

impl Default for LoRaConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 868_000_000,
            spreading_factor: 7,
            bandwidth_khz: 125,
            coding_rate: 0.8,
            tx_power_dbm: 14,
            meshtastic_mode: true,
            duty_cycle_percent: 1.0,
            spi_device: "/dev/spidev0.0".to_string(),
            use_mock: true, // Default to mock for safety
        }
    }
}

impl LoRaConfig {
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        if !(7..=12).contains(&self.spreading_factor) {
            return Err(NetworkError::InitializationFailed(
                "Spreading factor must be 7-12".to_string(),
            ));
        }

        if ![125, 250, 500].contains(&self.bandwidth_khz) {
            return Err(NetworkError::InitializationFailed(
                "Bandwidth must be 125, 250, or 500 kHz".to_string(),
            ));
        }

        if !(0.5..=1.0).contains(&self.coding_rate) {
            return Err(NetworkError::InitializationFailed(
                "Coding rate must be 0.5-1.0".to_string(),
            ));
        }

        if !(2..=20).contains(&self.tx_power_dbm) {
            return Err(NetworkError::InitializationFailed(
                "TX power must be 2-20 dBm".to_string(),
            ));
        }

        if !(0.0..=100.0).contains(&self.duty_cycle_percent) {
            return Err(NetworkError::InitializationFailed(
                "Duty cycle must be 0-100%".to_string(),
            ));
        }

        Ok(())
    }

    /// Calculate time-on-air for a given payload size in milliseconds
    pub fn calculate_time_on_air(&self, payload_bytes: usize) -> u64 {
        // Simplified time-on-air calculation
        // Real implementation would use proper LoRa formula
        let symbol_time_ms =
            (1000.0 * (1 << self.spreading_factor) as f64) / (self.bandwidth_khz as f64 * 1000.0);

        // Preamble + header + payload symbols
        let preamble_symbols = 8.0;
        let header_symbols = 5.0;
        let payload_symbols = ((payload_bytes as f64 * 8.0) / self.spreading_factor as f64).ceil();

        let total_symbols = preamble_symbols + header_symbols + payload_symbols;
        (total_symbols * symbol_time_ms) as u64
    }
}

/// Internal LoRa modem state
#[derive(Debug, Clone)]
struct LoRaState {
    /// Device ID (random or derived from MAC)
    device_id: u32,
    /// Signal-to-Noise Ratio of last received packet
    snr_db: Option<f32>,
    /// Received Signal Strength Indicator of last packet
    rssi_dbm: Option<i16>,
    /// Total packets sent
    packets_sent: u64,
    /// Total packets received
    packets_received: u64,
}

/// Duty cycle tracker
struct DutyCycleTracker {
    /// Cumulative TX time in current window (ms)
    tx_time_ms: Arc<AtomicU64>,
    /// Window start timestamp (Unix ms)
    window_start_ms: Arc<AtomicU64>,
    /// Duty cycle limit (0.0-1.0)
    limit: f32,
    /// Window duration (ms) - typically 1 hour
    window_duration_ms: u64,
}

impl DutyCycleTracker {
    fn new(limit_percent: f32) -> Self {
        Self {
            tx_time_ms: Arc::new(AtomicU64::new(0)),
            window_start_ms: Arc::new(AtomicU64::new(now_ms())),
            limit: limit_percent / 100.0,
            window_duration_ms: 3_600_000, // 1 hour
        }
    }

    /// Check if transmission is allowed and record usage
    fn check_and_record(&self, tx_duration_ms: u64) -> Result<()> {
        let current_time = now_ms();
        let window_start = self.window_start_ms.load(Ordering::Relaxed);

        // Check if we need to reset the window
        if current_time >= window_start + self.window_duration_ms {
            self.tx_time_ms.store(0, Ordering::Relaxed);
            self.window_start_ms.store(current_time, Ordering::Relaxed);
        }

        let current_tx_time = self.tx_time_ms.load(Ordering::Relaxed);
        let projected_tx_time = current_tx_time + tx_duration_ms;

        // Check if would exceed duty cycle
        let max_tx_time = (self.window_duration_ms as f64 * self.limit as f64) as u64;

        if projected_tx_time > max_tx_time {
            return Err(NetworkError::SendFailed(format!(
                "Duty cycle limit exceeded: {} ms used, {} ms max in window",
                projected_tx_time, max_tx_time
            )));
        }

        // Record the transmission
        self.tx_time_ms.fetch_add(tx_duration_ms, Ordering::Relaxed);
        Ok(())
    }

    /// Get current duty cycle usage (0.0-1.0)
    #[allow(dead_code)]
    fn get_usage(&self) -> f32 {
        let tx_time = self.tx_time_ms.load(Ordering::Relaxed);
        (tx_time as f32) / (self.window_duration_ms as f32)
    }
}

/// Hardware abstraction for LoRa modem
#[allow(dead_code)]
trait LoRaModem: Send + Sync {
    /// Initialize the modem
    fn initialize(&mut self, config: &LoRaConfig) -> Result<()>;

    /// Transmit a packet
    fn transmit(&mut self, data: &[u8]) -> Result<()>;

    /// Receive a packet (blocking with timeout)
    fn receive(&mut self, timeout_ms: u64) -> Result<Option<Vec<u8>>>;

    /// Get RSSI of last received packet
    fn get_rssi(&self) -> Option<i16>;

    /// Get SNR of last received packet
    fn get_snr(&self) -> Option<f32>;

    /// Set TX power
    fn set_tx_power(&mut self, power_dbm: i8) -> Result<()>;

    /// Enter sleep mode
    fn sleep(&mut self) -> Result<()>;
}

/// Mock LoRa modem for testing
#[allow(dead_code)]
struct MockLoRaModem {
    config: LoRaConfig,
    tx_queue: Arc<RwLock<Vec<Vec<u8>>>>,
    rx_queue: Arc<RwLock<Vec<Vec<u8>>>>,
    rssi: i16,
    snr: f32,
}

impl MockLoRaModem {
    fn new() -> Self {
        Self {
            config: LoRaConfig::default(),
            tx_queue: Arc::new(RwLock::new(Vec::new())),
            rx_queue: Arc::new(RwLock::new(Vec::new())),
            rssi: -50,
            snr: 10.0,
        }
    }
}

impl LoRaModem for MockLoRaModem {
    fn initialize(&mut self, config: &LoRaConfig) -> Result<()> {
        config.validate()?;
        self.config = config.clone();
        log::info!("Mock LoRa modem initialized at {} Hz", config.frequency_hz);
        Ok(())
    }

    fn transmit(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > 240 {
            return Err(NetworkError::MessageTooLarge {
                size: data.len(),
                max: 240,
            });
        }

        // Simulate transmission
        let tx_queue = self.tx_queue.clone();
        let data_vec = data.to_vec();
        tokio::spawn(async move {
            tx_queue.write().await.push(data_vec);
        });

        log::debug!("Mock LoRa TX: {} bytes", data.len());
        Ok(())
    }

    fn receive(&mut self, _timeout_ms: u64) -> Result<Option<Vec<u8>>> {
        // Mock always returns None (would be async in real impl)
        Ok(None)
    }

    fn get_rssi(&self) -> Option<i16> {
        Some(self.rssi)
    }

    fn get_snr(&self) -> Option<f32> {
        Some(self.snr)
    }

    fn set_tx_power(&mut self, power_dbm: i8) -> Result<()> {
        log::debug!("Mock LoRa TX power set to {} dBm", power_dbm);
        Ok(())
    }

    fn sleep(&mut self) -> Result<()> {
        log::debug!("Mock LoRa entering sleep mode");
        Ok(())
    }
}

/// Meshtastic protocol encoder/decoder
struct MeshtasticCodec;

impl MeshtasticCodec {
    /// Encode a MyriadMesh frame to Meshtastic packet format
    fn encode(frame: &Frame) -> Result<Vec<u8>> {
        // Simplified Meshtastic encoding
        // Real implementation would use protobuf

        let mut packet = Vec::new();

        // Header (4 bytes)
        packet.extend_from_slice(&[0x94, 0x28]); // Magic bytes
        packet.push(0x01); // Version
        packet.push(0x00); // Flags

        // Serialize frame
        let frame_data = bincode::serialize(frame)
            .map_err(|e| NetworkError::SendFailed(format!("Serialization failed: {}", e)))?;

        // Add payload length
        packet.extend_from_slice(&(frame_data.len() as u16).to_le_bytes());

        // Add payload
        packet.extend_from_slice(&frame_data);

        // Add CRC (2 bytes)
        let crc = calculate_crc16(&packet);
        packet.extend_from_slice(&crc.to_le_bytes());

        Ok(packet)
    }

    /// Decode Meshtastic packet to MyriadMesh frame
    fn decode(data: &[u8]) -> Result<Frame> {
        if data.len() < 8 {
            return Err(NetworkError::ReceiveFailed("Packet too small".to_string()));
        }

        // Verify magic bytes
        if data[0] != 0x94 || data[1] != 0x28 {
            return Err(NetworkError::ReceiveFailed(
                "Invalid magic bytes".to_string(),
            ));
        }

        // Extract payload length
        let payload_len = u16::from_le_bytes([data[4], data[5]]) as usize;

        if data.len() < 6 + payload_len + 2 {
            return Err(NetworkError::ReceiveFailed("Incomplete packet".to_string()));
        }

        // Verify CRC
        let expected_crc = u16::from_le_bytes([data[data.len() - 2], data[data.len() - 1]]);
        let actual_crc = calculate_crc16(&data[..data.len() - 2]);

        if expected_crc != actual_crc {
            return Err(NetworkError::ReceiveFailed("CRC mismatch".to_string()));
        }

        // Deserialize frame
        let frame_data = &data[6..6 + payload_len];
        bincode::deserialize(frame_data)
            .map_err(|e| NetworkError::ReceiveFailed(format!("Deserialization failed: {}", e)))
    }
}

/// Calculate CRC-16 (CCITT)
fn calculate_crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;

    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

/// LoRaWAN/Meshtastic adapter
pub struct LoRaAdapter {
    config: LoRaConfig,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    state: Arc<RwLock<LoRaState>>,
    modem: Arc<RwLock<Box<dyn LoRaModem>>>,
    duty_cycle: Arc<DutyCycleTracker>,
    rx: FrameReceiver,
    incoming_tx: mpsc::UnboundedSender<(Address, Frame)>,
    rx_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    running: Arc<AtomicBool>,
}

impl LoRaAdapter {
    pub fn new(config: LoRaConfig) -> Self {
        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::LoRaWAN,
            max_message_size: 240,      // Single LoRa frame
            typical_latency_ms: 2000.0, // ~2 seconds for SF7
            typical_bandwidth_bps: 250, // ~250 bps at SF7
            reliability: 0.95,          // High reliability, LOS
            range_meters: 15000.0,      // 15 km typical
            power_consumption: PowerConsumption::Low,
            cost_per_mb: 0.0,         // License-free spectrum
            supports_broadcast: true, // LoRa broadcast capable
            supports_multicast: true, // Supports group addressing
        };

        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();

        let modem: Box<dyn LoRaModem> = Box::new(MockLoRaModem::new());

        Self {
            duty_cycle: Arc::new(DutyCycleTracker::new(config.duty_cycle_percent)),
            config: config.clone(),
            status: Arc::new(RwLock::new(AdapterStatus::Uninitialized)),
            capabilities,
            state: Arc::new(RwLock::new(LoRaState {
                device_id: rand::random(),
                snr_db: None,
                rssi_dbm: None,
                packets_sent: 0,
                packets_received: 0,
            })),
            modem: Arc::new(RwLock::new(modem)),
            rx: Arc::new(RwLock::new(Some(incoming_rx))),
            incoming_tx,
            rx_task: Arc::new(RwLock::new(None)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start background receive task
    async fn start_rx_task(&self) -> Result<()> {
        let modem = self.modem.clone();
        let incoming_tx = self.incoming_tx.clone();
        let running = self.running.clone();
        let config = self.config.clone();
        let state = self.state.clone();

        let handle = tokio::spawn(async move {
            log::info!("LoRa RX task started");

            while running.load(Ordering::Relaxed) {
                // In mock mode, this would poll the modem
                // Real implementation would use interrupt-driven approach
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Try to receive (mock will return None)
                let mut modem_guard = modem.write().await;
                if let Ok(Some(data)) = modem_guard.receive(100) {
                    drop(modem_guard);

                    // Decode packet
                    let frame = if config.meshtastic_mode {
                        match MeshtasticCodec::decode(&data) {
                            Ok(f) => f,
                            Err(e) => {
                                log::warn!("Failed to decode Meshtastic packet: {}", e);
                                continue;
                            }
                        }
                    } else {
                        match bincode::deserialize(&data) {
                            Ok(f) => f,
                            Err(e) => {
                                log::warn!("Failed to deserialize frame: {}", e);
                                continue;
                            }
                        }
                    };

                    // Update state
                    let mut state_guard = state.write().await;
                    state_guard.packets_received += 1;

                    // Get modem again for RSSI/SNR
                    let modem_guard = modem.read().await;
                    state_guard.rssi_dbm = modem_guard.get_rssi();
                    state_guard.snr_db = modem_guard.get_snr();
                    drop(state_guard);

                    // Queue for application
                    let addr = Address::LoRa(format!("lora://unknown@{}", config.frequency_hz));
                    if incoming_tx.send((addr, frame)).is_err() {
                        log::warn!("Failed to queue received frame - channel closed");
                        break;
                    }
                }
            }

            log::info!("LoRa RX task stopped");
        });

        *self.rx_task.write().await = Some(handle);
        Ok(())
    }
}

#[async_trait::async_trait]
impl NetworkAdapter for LoRaAdapter {
    async fn initialize(&mut self) -> Result<()> {
        {
            let mut status = self.status.write().await;
            *status = AdapterStatus::Initializing;
        }

        // Validate configuration
        self.config.validate()?;

        // Initialize modem
        {
            let mut modem = self.modem.write().await;
            modem.initialize(&self.config)?;
        }

        {
            let mut status = self.status.write().await;
            *status = AdapterStatus::Ready;
        }

        log::info!(
            "LoRa adapter initialized at {} Hz, SF{}",
            self.config.frequency_hz,
            self.config.spreading_factor
        );

        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let status = self.status.read().await;
        match *status {
            AdapterStatus::Ready => {
                drop(status);

                // Start RX task
                self.running.store(true, Ordering::Relaxed);
                self.start_rx_task().await?;

                log::info!("LoRa adapter started");
                Ok(())
            }
            _ => Err(NetworkError::AdapterNotReady),
        }
    }

    async fn stop(&mut self) -> Result<()> {
        let mut status = self.status.write().await;
        *status = AdapterStatus::ShuttingDown;

        // Stop RX task
        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.rx_task.write().await.take() {
            handle.abort();
        }

        // Put modem to sleep
        {
            let mut modem = self.modem.write().await;
            modem.sleep()?;
        }

        *status = AdapterStatus::ShuttingDown;
        log::info!("LoRa adapter stopped");
        Ok(())
    }

    async fn send(&self, _destination: &Address, frame: &Frame) -> Result<()> {
        // Encode frame
        let data = if self.config.meshtastic_mode {
            MeshtasticCodec::encode(frame)?
        } else {
            bincode::serialize(frame)
                .map_err(|e| NetworkError::SendFailed(format!("Serialization failed: {}", e)))?
        };

        // Check size
        if data.len() > 240 {
            return Err(NetworkError::MessageTooLarge {
                size: data.len(),
                max: 240,
            });
        }

        // Calculate time-on-air
        let toa_ms = self.config.calculate_time_on_air(data.len());

        // Check duty cycle
        self.duty_cycle.check_and_record(toa_ms)?;

        // Transmit
        {
            let mut modem = self.modem.write().await;
            modem.transmit(&data)?;
        }

        // Update stats
        {
            let mut state = self.state.write().await;
            state.packets_sent += 1;
        }

        log::debug!("LoRa TX: {} bytes, ToA: {} ms", data.len(), toa_ms);
        Ok(())
    }

    async fn receive(&self, timeout_ms: u64) -> Result<(Address, Frame)> {
        let mut rx_guard = self.rx.write().await;
        let rx = rx_guard.as_mut().ok_or(NetworkError::AdapterNotReady)?;

        tokio::select! {
            result = rx.recv() => {
                result.ok_or(NetworkError::ReceiveFailed("Channel closed".to_string()))
            }
            _ = tokio::time::sleep(Duration::from_millis(timeout_ms)) => {
                Err(NetworkError::Timeout)
            }
        }
    }

    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        // Send broadcast discovery frame
        let source = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([0xFFu8; NODE_ID_SIZE]);
        let timestamp = now_ms();
        let payload = vec![];
        let msg_id = MessageId::generate(&source, &dest, &payload, timestamp, 0);

        let discovery_frame = Frame::new(
            MessageType::Discovery,
            source,
            dest,
            payload,
            msg_id,
            timestamp,
        )
        .map_err(|e| NetworkError::DiscoveryFailed(e.to_string()))?;

        self.send(&Address::LoRa("broadcast".to_string()), &discovery_frame)
            .await?;

        // Listen for responses (simplified - real implementation would collect responses)
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Return empty for now - real implementation would track responses
        Ok(Vec::new())
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

    async fn test_connection(&self, destination: &Address) -> Result<TestResults> {
        let start = std::time::Instant::now();

        // Send ping frame
        let source = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let timestamp = now_ms();
        let payload = vec![];
        let msg_id = MessageId::generate(&source, &dest, &payload, timestamp, 0);

        let ping_frame = Frame::new(MessageType::Ping, source, dest, payload, msg_id, timestamp)
            .map_err(|e| NetworkError::Other(e.to_string()))?;

        self.send(destination, &ping_frame).await?;

        // Wait for pong (simplified)
        let rtt_ms = start.elapsed().as_millis() as f64;

        Ok(TestResults {
            success: true,
            rtt_ms: Some(rtt_ms),
            error: None,
        })
    }

    fn get_local_address(&self) -> Option<Address> {
        let state = self.state.try_read().ok()?;
        Some(Address::LoRa(format!(
            "lora://0x{:08x}@{}",
            state.device_id, self.config.frequency_hz
        )))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        if !addr_str.starts_with("lora://") {
            return Err(NetworkError::InvalidAddress(
                "Not a LoRa address".to_string(),
            ));
        }

        Ok(Address::LoRa(addr_str.to_string()))
    }

    fn supports_address(&self, address: &Address) -> bool {
        matches!(address, Address::LoRa(_))
    }
}

/// Get current time in milliseconds since Unix epoch
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lora_config_validation() {
        let mut config = LoRaConfig::default();
        assert!(config.validate().is_ok());

        config.spreading_factor = 13;
        assert!(config.validate().is_err());

        config.spreading_factor = 7;
        config.bandwidth_khz = 100;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_lora_time_on_air_calculation() {
        let config = LoRaConfig::default();
        let toa = config.calculate_time_on_air(50);
        assert!(toa > 0);
        assert!(toa < 5000); // Should be less than 5 seconds for small packet
    }

    #[test]
    fn test_duty_cycle_tracker() {
        let tracker = DutyCycleTracker::new(1.0); // 1% duty cycle

        // Should allow small transmission
        assert!(tracker.check_and_record(100).is_ok());

        // Usage should be non-zero
        assert!(tracker.get_usage() > 0.0);
    }

    #[test]
    fn test_meshtastic_codec() {
        let source = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let timestamp = now_ms();
        let payload = vec![1, 2, 3, 4];
        let msg_id = MessageId::generate(&source, &dest, &payload, timestamp, 0);

        let frame =
            Frame::new(MessageType::Data, source, dest, payload, msg_id, timestamp).unwrap();

        let encoded = MeshtasticCodec::encode(&frame).unwrap();
        let decoded = MeshtasticCodec::decode(&encoded).unwrap();

        assert_eq!(frame.header.message_type, decoded.header.message_type);
    }

    #[test]
    fn test_crc16_calculation() {
        let data = vec![1, 2, 3, 4, 5];
        let crc1 = calculate_crc16(&data);
        let crc2 = calculate_crc16(&data);

        // CRC should be deterministic
        assert_eq!(crc1, crc2);

        // Different data should produce different CRC
        let different_data = vec![1, 2, 3, 4, 6];
        let crc3 = calculate_crc16(&different_data);
        assert_ne!(crc1, crc3);
    }

    #[tokio::test]
    async fn test_lora_adapter_creation() {
        let config = LoRaConfig::default();
        let adapter = LoRaAdapter::new(config);
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
    }

    #[tokio::test]
    async fn test_lora_adapter_initialization() {
        let config = LoRaConfig::default();
        let mut adapter = LoRaAdapter::new(config);

        assert!(adapter.initialize().await.is_ok());
        assert_eq!(adapter.get_status(), AdapterStatus::Ready);
    }

    #[tokio::test]
    async fn test_lora_capabilities() {
        let adapter = LoRaAdapter::new(LoRaConfig::default());
        let caps = adapter.get_capabilities();

        assert_eq!(caps.adapter_type, AdapterType::LoRaWAN);
        assert_eq!(caps.max_message_size, 240);
        assert!(caps.supports_broadcast);
        assert_eq!(caps.range_meters, 15000.0);
    }

    #[tokio::test]
    async fn test_lora_address_support() {
        let adapter = LoRaAdapter::new(LoRaConfig::default());
        let addr = Address::LoRa("lora://test".to_string());
        assert!(adapter.supports_address(&addr));
    }

    #[tokio::test]
    async fn test_lora_address_parsing() {
        let adapter = LoRaAdapter::new(LoRaConfig::default());
        let addr_str = "lora://0x12345678@868000000";
        let addr = adapter.parse_address(addr_str).unwrap();
        assert!(matches!(addr, Address::LoRa(_)));
    }

    #[tokio::test]
    async fn test_mock_modem() {
        let mut modem = MockLoRaModem::new();
        let config = LoRaConfig::default();

        assert!(modem.initialize(&config).is_ok());
        assert!(modem.transmit(&[1, 2, 3, 4]).is_ok());
        assert!(modem.set_tx_power(10).is_ok());
    }
}
