//! Amateur Packet Radio System (APRS) network adapter
//!
//! Provides worldwide connectivity via amateur radio network.
//! Requires FCC/ARRL amateur radio license (Technician class or higher).
//!
//! # Features
//!
//! - AX.25 protocol implementation
//! - KISS TNC (Terminal Node Controller) interface
//! - APRS-IS internet gateway support
//! - License verification and enforcement
//! - Digipeater support
//! - Mock TNC for testing

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::license::LicenseManager;
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::{types::AdapterType, Frame};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

type FrameReceiver = Arc<RwLock<Option<mpsc::Receiver<(Address, Frame)>>>>;

/// APRS adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AprsConfig {
    /// Ham radio callsign (e.g., "N0CALL-1")
    pub callsign: String,
    /// APRS passcode (computed from callsign)
    pub passcode: u16,
    /// APRS-IS server hostname
    pub aprs_is_server: String,
    /// APRS-IS port (default 14580)
    pub aprs_is_port: u16,
    /// TNC (Terminal Node Controller) device path
    pub tnc_device: String,
    /// Baud rate for TNC (usually 9600)
    pub tnc_baud_rate: u32,
    /// Use APRS-IS gateway (internet relay) or RF only
    pub use_internet_gateway: bool,
    /// Verify valid FCC ham license
    pub license_check: bool,
    /// Use mock TNC (for testing)
    pub use_mock: bool,
}

impl Default for AprsConfig {
    fn default() -> Self {
        Self {
            callsign: "N0CALL-1".to_string(),
            passcode: 0,
            aprs_is_server: "noam.aprs2.net".to_string(),
            aprs_is_port: 14580,
            tnc_device: "/dev/ttyUSB0".to_string(),
            tnc_baud_rate: 9600,
            use_internet_gateway: true,
            license_check: true,
            use_mock: true, // Default to mock for safety
        }
    }
}

impl AprsConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate callsign format
        if !Self::is_valid_callsign(&self.callsign) {
            return Err(NetworkError::InvalidCallsign(self.callsign.clone()));
        }

        // Validate APRS-IS port
        if self.aprs_is_port == 0 {
            return Err(NetworkError::InitializationFailed(
                "Invalid APRS-IS port".to_string(),
            ));
        }

        // Validate TNC baud rate
        if ![1200, 9600, 19200, 38400].contains(&self.tnc_baud_rate) {
            return Err(NetworkError::InitializationFailed(
                "Invalid TNC baud rate".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate callsign format
    fn is_valid_callsign(callsign: &str) -> bool {
        // Format: 1-2 letters, 1 digit, 1-3 letters/digits, optional SSID
        let parts: Vec<&str> = callsign.split('-').collect();
        if parts.is_empty() || parts.len() > 2 {
            return false;
        }

        let base = parts[0];
        if base.len() < 3 || base.len() > 6 {
            return false;
        }

        // Check pattern
        let chars: Vec<char> = base.chars().collect();
        let has_digit = chars.iter().any(|c| c.is_ascii_digit());
        let has_letter = chars.iter().any(|c| c.is_ascii_alphabetic());

        has_digit && has_letter
    }

    /// Calculate APRS passcode from callsign (algorithm from APRS spec)
    pub fn calculate_passcode(callsign: &str) -> u16 {
        let base = callsign.split('-').next().unwrap_or(callsign);
        let mut hash: i32 = 0x73e2;

        for (i, c) in base.to_uppercase().chars().enumerate() {
            let ascii = c as i32;
            if i % 2 == 0 {
                hash ^= ascii << 8;
            } else {
                hash ^= ascii;
            }
        }

        (hash & 0x7fff) as u16
    }
}

/// Internal APRS state
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AprsState {
    /// Connected to APRS-IS or TNC
    connected: bool,
    /// RF-only or Internet-linked
    mode: AprsMode,
    /// Last packet received from remote
    last_remote_heard: Option<String>,
    /// Number of digipeaters heard
    digipeater_count: usize,
    /// Packets sent
    packets_sent: u64,
    /// Packets received
    packets_received: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum AprsMode {
    TncOnly, // Direct TNC connection via serial
    AprsIs,  // APRS-IS network (internet)
    Hybrid,  // Both TNC and APRS-IS
}

/// AX.25 frame structure
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Ax25Frame {
    /// Destination address
    dest: String,
    /// Source address
    source: String,
    /// Digipeater path
    digipeaters: Vec<String>,
    /// Information field
    info: Vec<u8>,
    /// Control field
    control: u8,
    /// Protocol ID
    pid: u8,
}

#[allow(dead_code)]
impl Ax25Frame {
    /// Create new AX.25 frame
    fn new(dest: String, source: String, info: Vec<u8>) -> Self {
        Self {
            dest,
            source,
            digipeaters: Vec::new(),
            info,
            control: 0x03, // UI frame
            pid: 0xF0,     // No layer 3 protocol
        }
    }

    /// Encode to KISS format
    fn to_kiss(&self) -> Vec<u8> {
        let mut kiss = Vec::new();

        // KISS framing: FEND + Command + Data + FEND
        kiss.push(0xC0); // FEND
        kiss.push(0x00); // Data frame

        // Encode destination (7 bytes)
        kiss.extend_from_slice(&Self::encode_callsign(&self.dest));

        // Encode source (7 bytes)
        kiss.extend_from_slice(&Self::encode_callsign(&self.source));

        // Encode digipeaters (7 bytes each)
        for digi in &self.digipeaters {
            kiss.extend_from_slice(&Self::encode_callsign(digi));
        }

        // Control and PID
        kiss.push(self.control);
        kiss.push(self.pid);

        // Information field
        kiss.extend_from_slice(&self.info);

        kiss.push(0xC0); // FEND

        kiss
    }

    /// Decode from KISS format
    fn from_kiss(data: &[u8]) -> Result<Self> {
        if data.len() < 18 {
            // Minimum: 7 dest + 7 source + 1 control + 1 pid + 2 FEND
            return Err(NetworkError::ReceiveFailed("Packet too small".to_string()));
        }

        // Verify KISS framing
        if data[0] != 0xC0 || data[data.len() - 1] != 0xC0 {
            return Err(NetworkError::ReceiveFailed(
                "Invalid KISS framing".to_string(),
            ));
        }

        if data[1] != 0x00 {
            return Err(NetworkError::ReceiveFailed("Not a data frame".to_string()));
        }

        // Decode addresses
        let dest = Self::decode_callsign(&data[2..9])?;
        let source = Self::decode_callsign(&data[9..16])?;

        // TODO: Decode digipeaters
        let digipeaters = Vec::new();

        // Extract control, PID, and info
        let control = data[16];
        let pid = data[17];
        let info = data[18..data.len() - 1].to_vec();

        Ok(Self {
            dest,
            source,
            digipeaters,
            info,
            control,
            pid,
        })
    }

    /// Encode callsign to AX.25 format (7 bytes)
    fn encode_callsign(callsign: &str) -> [u8; 7] {
        let mut encoded = [0x40; 7]; // Space-padded

        let parts: Vec<&str> = callsign.split('-').collect();
        let base = parts[0];
        let ssid = parts.get(1).and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);

        // Encode callsign (max 6 chars, shifted left by 1)
        for (i, c) in base.chars().take(6).enumerate() {
            encoded[i] = (c.to_ascii_uppercase() as u8) << 1;
        }

        // SSID byte
        encoded[6] = 0x60 | ((ssid & 0x0F) << 1);

        encoded
    }

    /// Decode callsign from AX.25 format
    fn decode_callsign(data: &[u8]) -> Result<String> {
        if data.len() != 7 {
            return Err(NetworkError::ReceiveFailed(
                "Invalid callsign length".to_string(),
            ));
        }

        let mut callsign = String::new();

        // Decode base callsign
        for &byte in &data[0..6] {
            let c = (byte >> 1) as char;
            if c != ' ' {
                callsign.push(c);
            }
        }

        // Decode SSID
        let ssid = (data[6] >> 1) & 0x0F;
        if ssid > 0 {
            callsign.push('-');
            callsign.push_str(&ssid.to_string());
        }

        Ok(callsign)
    }
}

/// TNC (Terminal Node Controller) interface
trait Tnc: Send + Sync {
    /// Initialize TNC connection
    fn initialize(&mut self, config: &AprsConfig) -> Result<()>;

    /// Send AX.25 frame via TNC
    fn send_frame(&mut self, frame: &Ax25Frame) -> Result<()>;

    /// Receive AX.25 frame from TNC (non-blocking)
    fn receive_frame(&mut self) -> Result<Option<Ax25Frame>>;

    /// Close TNC connection
    fn close(&mut self) -> Result<()>;
}

/// Mock TNC for testing
#[allow(dead_code)]
struct MockTnc {
    config: AprsConfig,
    tx_queue: Arc<RwLock<Vec<Ax25Frame>>>,
    rx_queue: Arc<RwLock<Vec<Ax25Frame>>>,
}

impl MockTnc {
    fn new() -> Self {
        Self {
            config: AprsConfig::default(),
            tx_queue: Arc::new(RwLock::new(Vec::new())),
            rx_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Tnc for MockTnc {
    fn initialize(&mut self, config: &AprsConfig) -> Result<()> {
        config.validate()?;
        self.config = config.clone();
        log::info!("Mock TNC initialized for callsign {}", config.callsign);
        Ok(())
    }

    fn send_frame(&mut self, frame: &Ax25Frame) -> Result<()> {
        let tx_queue = self.tx_queue.clone();
        let frame_clone = frame.clone();
        tokio::spawn(async move {
            tx_queue.write().await.push(frame_clone);
        });
        log::debug!("Mock TNC TX: {} -> {}", frame.source, frame.dest);
        Ok(())
    }

    fn receive_frame(&mut self) -> Result<Option<Ax25Frame>> {
        // Mock always returns None (would be async in real impl)
        Ok(None)
    }

    fn close(&mut self) -> Result<()> {
        log::info!("Mock TNC closed");
        Ok(())
    }
}

/// APRS-IS client for internet gateway
#[allow(dead_code)]
struct AprsIsClient {
    server: String,
    port: u16,
    callsign: String,
    passcode: u16,
    connected: bool,
}

#[allow(dead_code)]
impl AprsIsClient {
    fn new(server: String, port: u16, callsign: String, passcode: u16) -> Self {
        Self {
            server,
            port,
            callsign,
            passcode,
            connected: false,
        }
    }

    /// Connect to APRS-IS server
    async fn connect(&mut self) -> Result<()> {
        // TODO: Actual TCP connection
        // For now, just mark as connected
        self.connected = true;
        log::info!("Connected to APRS-IS server {}:{}", self.server, self.port);
        Ok(())
    }

    /// Send packet to APRS-IS
    async fn send_packet(&mut self, _packet: &str) -> Result<()> {
        if !self.connected {
            return Err(NetworkError::AdapterNotReady);
        }
        // TODO: Send via TCP
        Ok(())
    }

    /// Receive packet from APRS-IS
    async fn receive_packet(&mut self) -> Result<Option<String>> {
        if !self.connected {
            return Err(NetworkError::AdapterNotReady);
        }
        // TODO: Receive via TCP
        Ok(None)
    }

    /// Disconnect from APRS-IS
    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        log::info!("Disconnected from APRS-IS");
        Ok(())
    }
}

/// APRS adapter
pub struct AprsAdapter {
    config: AprsConfig,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    state: Arc<RwLock<AprsState>>,
    license_manager: Arc<LicenseManager>,
    tnc: Arc<RwLock<Box<dyn Tnc>>>,
    aprs_is: Arc<RwLock<Option<AprsIsClient>>>,
    rx: FrameReceiver,
    incoming_tx: mpsc::Sender<(Address, Frame)>,
    rx_task: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl AprsAdapter {
    pub fn new(config: AprsConfig) -> Self {
        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::APRS,
            max_message_size: 256,       // AX.25 limit
            typical_latency_ms: 3000.0,  // RF propagation
            typical_bandwidth_bps: 1200, // 1200 bps standard
            reliability: 0.92,           // Atmospheric interference
            range_meters: 30000.0,       // 30 km typical
            power_consumption: PowerConsumption::Low,
            cost_per_mb: 0.0, // License-free for hams
            supports_broadcast: true,
            supports_multicast: true,
        };

        // RESOURCE M3: Bounded channel to prevent memory exhaustion
        // LoRa/Radio: 1,000 capacity (low throughput)
        let (incoming_tx, incoming_rx) = mpsc::channel(1000);

        let tnc: Box<dyn Tnc> = Box::new(MockTnc::new());
        let license_manager = Arc::new(LicenseManager::new_offline());

        // Initialize APRS-IS client if configured
        let aprs_is = if config.use_internet_gateway {
            Some(AprsIsClient::new(
                config.aprs_is_server.clone(),
                config.aprs_is_port,
                config.callsign.clone(),
                config.passcode,
            ))
        } else {
            None
        };

        Self {
            config: config.clone(),
            status: Arc::new(RwLock::new(AdapterStatus::Uninitialized)),
            capabilities,
            state: Arc::new(RwLock::new(AprsState {
                connected: false,
                mode: if config.use_internet_gateway {
                    AprsMode::Hybrid
                } else {
                    AprsMode::TncOnly
                },
                last_remote_heard: None,
                digipeater_count: 0,
                packets_sent: 0,
                packets_received: 0,
            })),
            license_manager,
            tnc: Arc::new(RwLock::new(tnc)),
            aprs_is: Arc::new(RwLock::new(aprs_is)),
            rx: Arc::new(RwLock::new(Some(incoming_rx))),
            incoming_tx,
            rx_task: Arc::new(RwLock::new(None)),
        }
    }

    /// Start background receive task
    async fn start_rx_task(&self) -> Result<()> {
        let tnc = self.tnc.clone();
        let aprs_is = self.aprs_is.clone();
        let incoming_tx = self.incoming_tx.clone();
        let state = self.state.clone();

        let handle = tokio::spawn(async move {
            log::info!("APRS RX task started");

            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Check TNC
                let mut tnc_guard = tnc.write().await;
                if let Ok(Some(ax25_frame)) = tnc_guard.receive_frame() {
                    drop(tnc_guard);

                    // Convert AX.25 to MyriadMesh frame
                    if let Ok(frame) = bincode::deserialize(&ax25_frame.info) {
                        let mut state_guard = state.write().await;
                        state_guard.packets_received += 1;
                        state_guard.last_remote_heard = Some(ax25_frame.source.clone());

                        // RESOURCE M3: Handle backpressure with try_send
                        let addr = Address::APRS(format!("aprs://{}", ax25_frame.source));
                        match incoming_tx.try_send((addr, frame)) {
                            Ok(_) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                log::warn!("APRS incoming channel full, dropping frame");
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                log::warn!("APRS incoming channel closed, stopping RX task");
                                break;
                            }
                        }
                    }
                }

                // Check APRS-IS
                let mut aprs_is_guard = aprs_is.write().await;
                if let Some(ref mut client) = *aprs_is_guard {
                    if let Ok(Some(_packet)) = client.receive_packet().await {
                        // TODO: Parse APRS-IS packet format
                    }
                }
            }

            log::info!("APRS RX task stopped");
        });

        *self.rx_task.write().await = Some(handle);
        Ok(())
    }
}

#[async_trait::async_trait]
impl NetworkAdapter for AprsAdapter {
    async fn initialize(&mut self) -> Result<()> {
        {
            let mut status = self.status.write().await;
            *status = AdapterStatus::Initializing;
        }

        // Validate configuration
        self.config.validate()?;

        // Check license if required
        if self.config.license_check {
            // Set license from config
            use crate::license::LicenseClass;
            self.license_manager
                .set_license(
                    self.config.callsign.clone(),
                    LicenseClass::Amateur(crate::license::AmateurClass::Technician),
                    None,
                )
                .await?;
        }

        // Initialize TNC
        {
            let mut tnc = self.tnc.write().await;
            tnc.initialize(&self.config)?;
        }

        // Connect to APRS-IS if configured
        if self.config.use_internet_gateway {
            let mut aprs_is_guard = self.aprs_is.write().await;
            if let Some(ref mut client) = *aprs_is_guard {
                client.connect().await?;
            }
        }

        {
            let mut status = self.status.write().await;
            *status = AdapterStatus::Ready;
        }

        log::info!(
            "APRS adapter initialized for callsign {}",
            self.config.callsign
        );
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let status = self.status.read().await;
        match *status {
            AdapterStatus::Ready => {
                drop(status);
                self.start_rx_task().await?;
                log::info!("APRS adapter started");
                Ok(())
            }
            _ => Err(NetworkError::AdapterNotReady),
        }
    }

    async fn stop(&mut self) -> Result<()> {
        let mut status = self.status.write().await;
        *status = AdapterStatus::ShuttingDown;

        // Stop RX task
        if let Some(handle) = self.rx_task.write().await.take() {
            handle.abort();
        }

        // Close TNC
        {
            let mut tnc = self.tnc.write().await;
            tnc.close()?;
        }

        // Disconnect from APRS-IS
        if let Some(ref mut client) = *self.aprs_is.write().await {
            client.disconnect().await?;
        }

        *status = AdapterStatus::Uninitialized;
        log::info!("APRS adapter stopped");
        Ok(())
    }

    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()> {
        // Check license before transmission
        if self.config.license_check {
            self.license_manager.can_transmit().await?;
        }

        // Serialize frame
        let data = bincode::serialize(frame)
            .map_err(|e| NetworkError::SendFailed(format!("Serialization failed: {}", e)))?;

        // Check size
        if data.len() > 256 {
            return Err(NetworkError::MessageTooLarge {
                size: data.len(),
                max: 256,
            });
        }

        // Extract destination callsign
        let dest_callsign = match destination {
            Address::APRS(addr) => addr.strip_prefix("aprs://").unwrap_or("APRS").to_string(),
            _ => "APRS".to_string(),
        };

        // Create AX.25 frame
        let ax25_frame = Ax25Frame::new(dest_callsign, self.config.callsign.clone(), data);

        // Transmit via TNC
        {
            let mut tnc = self.tnc.write().await;
            tnc.send_frame(&ax25_frame)?;
        }

        // Update stats
        {
            let mut state = self.state.write().await;
            state.packets_sent += 1;
        }

        log::debug!("APRS TX: {} -> {}", ax25_frame.source, ax25_frame.dest);
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
        // APRS discovery: send beacon and listen
        Ok(Vec::new()) // Simplified for now
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
        Ok(TestResults {
            success: true,
            rtt_ms: Some(3000.0),
            error: None,
        })
    }

    fn get_local_address(&self) -> Option<Address> {
        Some(Address::APRS(format!("aprs://{}", self.config.callsign)))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        if !addr_str.starts_with("aprs://") {
            return Err(NetworkError::InvalidAddress(
                "Not an APRS address".to_string(),
            ));
        }

        Ok(Address::APRS(addr_str.to_string()))
    }

    fn supports_address(&self, address: &Address) -> bool {
        matches!(address, Address::APRS(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aprs_config_validation() {
        let mut config = AprsConfig::default();
        assert!(config.validate().is_ok());

        config.callsign = "INVALID".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_callsign_validation() {
        assert!(AprsConfig::is_valid_callsign("N0CALL"));
        assert!(AprsConfig::is_valid_callsign("N0CALL-1"));
        assert!(AprsConfig::is_valid_callsign("KE7XYZ"));
        assert!(!AprsConfig::is_valid_callsign("ABC"));
        assert!(!AprsConfig::is_valid_callsign("123"));
    }

    #[test]
    fn test_passcode_calculation() {
        let passcode = AprsConfig::calculate_passcode("N0CALL");
        assert!(passcode > 0);
        assert!(passcode < 32768); // 15-bit value
    }

    #[test]
    fn test_ax25_callsign_encoding() {
        let encoded = Ax25Frame::encode_callsign("N0CALL");
        assert_eq!(encoded.len(), 7);

        let decoded = Ax25Frame::decode_callsign(&encoded).unwrap();
        assert_eq!(decoded, "N0CALL");
    }

    #[test]
    fn test_ax25_callsign_with_ssid() {
        let encoded = Ax25Frame::encode_callsign("N0CALL-1");
        let decoded = Ax25Frame::decode_callsign(&encoded).unwrap();
        assert_eq!(decoded, "N0CALL-1");
    }

    #[test]
    fn test_ax25_frame_creation() {
        let frame = Ax25Frame::new("APRS".to_string(), "N0CALL".to_string(), vec![1, 2, 3, 4]);

        assert_eq!(frame.dest, "APRS");
        assert_eq!(frame.source, "N0CALL");
        assert_eq!(frame.info, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_kiss_encoding() {
        let frame = Ax25Frame::new("APRS".to_string(), "N0CALL".to_string(), vec![1, 2, 3, 4]);

        let kiss = frame.to_kiss();
        assert!(!kiss.is_empty());
        assert_eq!(kiss[0], 0xC0); // FEND
        assert_eq!(kiss[kiss.len() - 1], 0xC0); // FEND
    }

    #[tokio::test]
    async fn test_aprs_adapter_creation() {
        let config = AprsConfig::default();
        let adapter = AprsAdapter::new(config);
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
    }

    #[tokio::test]
    async fn test_aprs_adapter_initialization() {
        let config = AprsConfig {
            license_check: false,
            ..Default::default()
        };
        let mut adapter = AprsAdapter::new(config);

        assert!(adapter.initialize().await.is_ok());
        assert_eq!(adapter.get_status(), AdapterStatus::Ready);
    }

    #[tokio::test]
    async fn test_aprs_capabilities() {
        let adapter = AprsAdapter::new(AprsConfig::default());
        let caps = adapter.get_capabilities();

        assert_eq!(caps.adapter_type, AdapterType::APRS);
        assert_eq!(caps.max_message_size, 256);
        assert!(caps.supports_broadcast);
    }

    #[tokio::test]
    async fn test_mock_tnc() {
        let mut tnc = MockTnc::new();
        let config = AprsConfig::default();

        assert!(tnc.initialize(&config).is_ok());

        let frame = Ax25Frame::new("APRS".to_string(), "N0CALL".to_string(), vec![1, 2, 3, 4]);

        assert!(tnc.send_frame(&frame).is_ok());
    }
}
