//! Dial-up/PPPoE network adapter
//!
//! Provides legacy dial-up modem and GSM-SMS fallback connectivity.
//! Last-resort emergency communication when modern networks unavailable.

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::{types::AdapterType, Frame};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

type FrameReceiver = Arc<RwLock<Option<mpsc::UnboundedReceiver<(Address, Frame)>>>>;

/// Dial-up adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialupConfig {
    /// Type of modem
    pub modem_type: ModemType,
    /// Serial device path
    pub device_path: String,
    /// Baud rate
    pub baud_rate: u32,
    /// Phone number to dial (PSTN) or device ID (GSM)
    pub phone_number: String,
    /// ISP name or APN
    pub isp_name: String,
    /// PPP username
    pub ppp_username: String,
    /// PPP password
    pub ppp_password: String,
    /// Dial timeout in seconds
    pub dial_timeout_secs: u32,
    /// Idle timeout before hanging up (0 = never)
    pub idle_timeout_secs: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModemType {
    /// Traditional dial-up modem (Hayes AT command compatible)
    SerialHayes,
    /// USB modem device
    UsbModem,
    /// GSM/SMS modem (SIM800, SIM900, etc.)
    GsmModule,
}

impl Default for DialupConfig {
    fn default() -> Self {
        Self {
            modem_type: ModemType::GsmModule,
            device_path: "/dev/ttyUSB0".to_string(),
            phone_number: "0800123456".to_string(),
            isp_name: "default-isp".to_string(),
            ppp_username: "user".to_string(),
            ppp_password: "password".to_string(),
            baud_rate: 9600,
            dial_timeout_secs: 300,
            idle_timeout_secs: 0,
        }
    }
}

/// Internal dial-up state
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DialupState {
    /// Connected to ISP or network
    connected: bool,
    /// IP address assigned by ISP
    ip_address: Option<String>,
    /// Data transmitted
    bytes_sent: u64,
    /// Data received
    bytes_received: u64,
    /// Call duration (seconds)
    call_duration_secs: u64,
    /// Signal quality (for GSM)
    signal_quality: Option<u8>,
}

/// Modem controller abstraction
#[allow(dead_code)]
trait ModemController: Send + Sync {
    /// Send AT command and wait for response
    fn send_at_command(&mut self, command: &str, timeout_ms: u64) -> Result<String>;

    /// Read unsolicited response
    fn read_response(&mut self, timeout_ms: u64) -> Result<String>;

    /// Set DTR (Data Terminal Ready) signal
    fn set_dtr(&mut self, active: bool) -> Result<()>;

    /// Get signal quality (0-31, 99 = unknown)
    fn get_signal_quality(&self) -> Option<u8>;
}

/// Mock modem controller for testing
struct MockModemController {
    responses: HashMap<String, String>,
    signal_quality: u8,
    connected: bool,
}

impl MockModemController {
    fn new() -> Self {
        let mut responses = HashMap::new();

        // Standard AT commands
        responses.insert("AT".to_string(), "OK".to_string());
        responses.insert("ATE0".to_string(), "OK".to_string());
        responses.insert("ATV1".to_string(), "OK".to_string());
        responses.insert("ATZ".to_string(), "OK".to_string());
        responses.insert("ATH".to_string(), "OK".to_string());

        // Dial command
        responses.insert("ATDT".to_string(), "CONNECT 56000".to_string());

        // GSM commands
        responses.insert("AT+CGACT=1,1".to_string(), "OK".to_string());
        responses.insert("AT+CIFSR".to_string(), "192.168.1.100".to_string());
        responses.insert("AT+CSQ".to_string(), "+CSQ: 25,0\r\nOK".to_string());

        // SMS commands
        responses.insert("AT+CMGF=1".to_string(), "OK".to_string());
        responses.insert("AT+CMGS".to_string(), "> ".to_string());

        Self {
            responses,
            signal_quality: 25,
            connected: false,
        }
    }
}

impl ModemController for MockModemController {
    fn send_at_command(&mut self, command: &str, _timeout_ms: u64) -> Result<String> {
        // Extract base command (without parameters)
        let base_cmd = command.split(['=', ' ']).next().unwrap_or(command);

        // Handle dial commands specially
        if command.starts_with("ATDT") {
            self.connected = true;
            return Ok("CONNECT 56000".to_string());
        }

        if let Some(response) = self.responses.get(base_cmd) {
            Ok(response.clone())
        } else {
            Ok("OK".to_string())
        }
    }

    fn read_response(&mut self, _timeout_ms: u64) -> Result<String> {
        Ok("".to_string())
    }

    fn set_dtr(&mut self, _active: bool) -> Result<()> {
        Ok(())
    }

    fn get_signal_quality(&self) -> Option<u8> {
        Some(self.signal_quality)
    }
}

/// PPP (Point-to-Point Protocol) session
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PppState {
    Idle,
    LcpNegotiating,
    Authenticating,
    IpcpNegotiating,
    Established,
}

#[allow(dead_code)]
struct PppSession {
    state: PppState,
    local_ip: Option<String>,
    remote_ip: Option<String>,
    mtu: u16,
}

impl PppSession {
    fn new() -> Self {
        Self {
            state: PppState::Idle,
            local_ip: None,
            remote_ip: None,
            mtu: 1500,
        }
    }

    /// Negotiate LCP (Link Control Protocol)
    async fn negotiate_lcp(&mut self) -> Result<()> {
        self.state = PppState::LcpNegotiating;

        // In real implementation:
        // 1. Send LCP Configure-Request
        // 2. Receive LCP Configure-Ack/Nak/Reject
        // 3. Negotiate options (MRU, auth protocol, etc.)

        // Mock: instant success
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(())
    }

    /// Authenticate with PAP (Password Authentication Protocol)
    async fn authenticate_pap(&mut self, username: &str, password: &str) -> Result<()> {
        self.state = PppState::Authenticating;

        // In real implementation:
        // 1. Send PAP Authenticate-Request with username/password
        // 2. Wait for PAP Authenticate-Ack/Nak

        // Mock: validate credentials
        if !username.is_empty() && !password.is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(())
        } else {
            Err(NetworkError::Other("Authentication failed".to_string()))
        }
    }

    /// Negotiate IPCP (IP Control Protocol)
    async fn negotiate_ipcp(&mut self) -> Result<()> {
        self.state = PppState::IpcpNegotiating;

        // In real implementation:
        // 1. Send IPCP Configure-Request
        // 2. Request IP address from server
        // 3. Configure DNS servers

        // Mock: assign IP
        self.local_ip = Some("192.168.1.100".to_string());
        self.remote_ip = Some("192.168.1.1".to_string());

        tokio::time::sleep(Duration::from_millis(10)).await;
        self.state = PppState::Established;
        Ok(())
    }

    /// Encapsulate data in PPP frame
    fn encapsulate(&self, data: &[u8]) -> Vec<u8> {
        // PPP frame format:
        // Flag (0x7E) | Address (0xFF) | Control (0x03) | Protocol (0x0021 for IP) | Data | FCS | Flag

        let mut frame = Vec::with_capacity(data.len() + 8);
        frame.push(0x7E); // Flag
        frame.push(0xFF); // Address (all stations)
        frame.push(0x03); // Control (unnumbered information)
        frame.push(0x00); // Protocol high byte (IP)
        frame.push(0x21); // Protocol low byte
        frame.extend_from_slice(data);

        // Calculate FCS (Frame Check Sequence) - simplified
        let fcs = self.calculate_fcs(data);
        frame.push((fcs >> 8) as u8);
        frame.push((fcs & 0xFF) as u8);
        frame.push(0x7E); // Closing flag

        frame
    }

    /// Decapsulate PPP frame
    #[allow(dead_code)]
    fn decapsulate(&self, frame: &[u8]) -> Result<Vec<u8>> {
        if frame.len() < 8 {
            return Err(NetworkError::Other("Invalid PPP frame".to_string()));
        }

        // Check flags
        if frame[0] != 0x7E || frame[frame.len() - 1] != 0x7E {
            return Err(NetworkError::Other("Invalid PPP flags".to_string()));
        }

        // Extract data (skip flag, address, control, protocol, FCS, flag)
        let data = &frame[5..frame.len() - 3];
        Ok(data.to_vec())
    }

    /// Calculate Frame Check Sequence (simplified CRC-16)
    fn calculate_fcs(&self, data: &[u8]) -> u16 {
        let mut fcs: u16 = 0xFFFF;

        for &byte in data {
            fcs ^= byte as u16;
            for _ in 0..8 {
                if fcs & 1 != 0 {
                    fcs = (fcs >> 1) ^ 0x8408;
                } else {
                    fcs >>= 1;
                }
            }
        }

        !fcs
    }
}

/// SMS codec for GSM modems
struct SmsCodec;

impl SmsCodec {
    /// Encode text to GSM 7-bit encoding
    #[allow(dead_code)]
    fn encode_gsm7(text: &str) -> Vec<u8> {
        // Simplified: just use ASCII bytes (real impl would use GSM 7-bit alphabet)
        text.as_bytes().to_vec()
    }

    /// Decode GSM 7-bit encoding to text
    #[allow(dead_code)]
    fn decode_gsm7(data: &[u8]) -> String {
        // Simplified: just convert from ASCII
        String::from_utf8_lossy(data).to_string()
    }

    /// Create SMS PDU (Protocol Data Unit)
    #[allow(dead_code)]
    fn create_pdu(destination: &str, message: &str) -> Vec<u8> {
        // Real implementation would create full PDU with:
        // - SMSC (SMS Center) address
        // - PDU type
        // - Destination address
        // - Protocol identifier
        // - Data coding scheme
        // - User data

        // Simplified mock
        let mut pdu = Vec::new();
        pdu.extend_from_slice(destination.as_bytes());
        pdu.push(0x00); // Separator
        pdu.extend_from_slice(&Self::encode_gsm7(message));
        pdu
    }

    /// Parse received SMS PDU
    #[allow(dead_code)]
    fn parse_pdu(_pdu: &[u8]) -> Result<(String, String)> {
        // Real implementation would parse full PDU
        // For now, return mock data
        Ok(("+1234567890".to_string(), "Test message".to_string()))
    }
}

/// Dial-up adapter
pub struct DialupAdapter {
    config: DialupConfig,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    state: Arc<RwLock<DialupState>>,
    modem: Arc<RwLock<Box<dyn ModemController>>>,
    ppp_session: Arc<RwLock<Option<PppSession>>>,
    rx: FrameReceiver,
    incoming_tx: mpsc::UnboundedSender<(Address, Frame)>,
    call_start_time: Arc<RwLock<Option<Instant>>>,
}

impl DialupAdapter {
    pub fn new(config: DialupConfig) -> Self {
        let (typical_bandwidth_bps, typical_latency_ms, power_consumption) = match config.modem_type
        {
            ModemType::SerialHayes => (2400, 300.0, PowerConsumption::Low), // V.92 modem
            ModemType::UsbModem => (56000, 200.0, PowerConsumption::Low),   // V.92 USB
            ModemType::GsmModule => (115200, 500.0, PowerConsumption::Medium), // GSM/LTE
        };

        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::Dialup,
            max_message_size: 1500,
            typical_latency_ms,
            typical_bandwidth_bps,
            reliability: 0.85, // Variable connection quality
            range_meters: 0.0, // Wide area
            power_consumption,
            cost_per_mb: 0.05, // Typically costs money
            supports_broadcast: false,
            supports_multicast: false,
        };

        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();

        // Create mock modem controller
        let modem: Box<dyn ModemController> = Box::new(MockModemController::new());

        Self {
            config,
            status: Arc::new(RwLock::new(AdapterStatus::Uninitialized)),
            capabilities,
            state: Arc::new(RwLock::new(DialupState {
                connected: false,
                ip_address: None,
                bytes_sent: 0,
                bytes_received: 0,
                call_duration_secs: 0,
                signal_quality: None,
            })),
            modem: Arc::new(RwLock::new(modem)),
            ppp_session: Arc::new(RwLock::new(None)),
            rx: Arc::new(RwLock::new(Some(incoming_rx))),
            incoming_tx,
            call_start_time: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize modem with AT commands
    async fn initialize_modem(&mut self) -> Result<()> {
        let mut modem = self.modem.write().await;

        // Reset modem
        modem.send_at_command("ATZ", 1000)?;
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Disable echo
        modem.send_at_command("ATE0", 1000)?;

        // Enable verbose responses
        modem.send_at_command("ATV1", 1000)?;

        // Test connection
        let response = modem.send_at_command("AT", 1000)?;
        if !response.contains("OK") {
            return Err(NetworkError::InitializationFailed(
                "Modem not responding".to_string(),
            ));
        }

        // Get signal quality for GSM
        if self.config.modem_type == ModemType::GsmModule {
            let signal = modem.get_signal_quality();
            let mut state = self.state.write().await;
            state.signal_quality = signal;
        }

        Ok(())
    }

    /// Dial phone number or connect to GSM network
    async fn dial(&mut self) -> Result<()> {
        let mut modem = self.modem.write().await;

        match self.config.modem_type {
            ModemType::SerialHayes | ModemType::UsbModem => {
                // Dial using tone dialing
                let dial_cmd = format!("ATDT{}", self.config.phone_number);
                let response = modem
                    .send_at_command(&dial_cmd, self.config.dial_timeout_secs as u64 * 1000)?;

                if !response.contains("CONNECT") {
                    return Err(NetworkError::Other("Dial failed".to_string()));
                }

                // Record call start time
                let mut call_time = self.call_start_time.write().await;
                *call_time = Some(Instant::now());
            }
            ModemType::GsmModule => {
                // Activate PDP context
                modem.send_at_command("AT+CGACT=1,1", 5000)?;

                // Get IP address
                let response = modem.send_at_command("AT+CIFSR", 2000)?;

                // Extract IP address from response
                let ip = response
                    .lines()
                    .find(|line| line.chars().next().is_some_and(|c| c.is_numeric()))
                    .unwrap_or("192.168.1.100")
                    .to_string();

                let mut state = self.state.write().await;
                state.ip_address = Some(ip);
                state.connected = true;
            }
        }

        Ok(())
    }

    /// Establish PPP connection with ISP
    async fn establish_ppp(&mut self) -> Result<()> {
        let mut ppp = PppSession::new();

        // LCP negotiation
        ppp.negotiate_lcp().await?;

        // PAP authentication
        ppp.authenticate_pap(&self.config.ppp_username, &self.config.ppp_password)
            .await?;

        // IPCP configuration
        ppp.negotiate_ipcp().await?;

        // Store IP address
        if let Some(ip) = &ppp.local_ip {
            let mut state = self.state.write().await;
            state.ip_address = Some(ip.clone());
            state.connected = true;
        }

        // Save session
        let mut session = self.ppp_session.write().await;
        *session = Some(ppp);

        Ok(())
    }

    /// Disconnect and hang up
    async fn hang_up(&self) -> Result<()> {
        // Send escape sequence (wait 1 second before and after)
        tokio::time::sleep(Duration::from_secs(1)).await;

        let mut modem = self.modem.write().await;

        // Send "+++" to enter command mode
        modem.send_at_command("+++", 1000).ok();

        tokio::time::sleep(Duration::from_secs(1)).await;

        // Send hang up command
        modem.send_at_command("ATH", 1000)?;

        // Update state
        let mut state = self.state.write().await;
        state.connected = false;
        state.ip_address = None;

        // Clear PPP session
        let mut session = self.ppp_session.write().await;
        *session = None;

        Ok(())
    }

    /// Send SMS via GSM modem (fallback for very low bandwidth)
    #[allow(dead_code)]
    async fn send_sms(&self, destination: &str, message: &str) -> Result<()> {
        if self.config.modem_type != ModemType::GsmModule {
            return Err(NetworkError::Other(
                "SMS only supported on GSM modems".to_string(),
            ));
        }

        let mut modem = self.modem.write().await;

        // Set text mode
        modem.send_at_command("AT+CMGF=1", 1000)?;

        // Start SMS
        let sms_cmd = format!("AT+CMGS=\"{}\"", destination);
        let response = modem.send_at_command(&sms_cmd, 1000)?;

        if !response.contains(">") {
            return Err(NetworkError::SendFailed(
                "SMS prompt not received".to_string(),
            ));
        }

        // Send message (in real implementation, send Ctrl+Z after message)
        let _pdu = SmsCodec::create_pdu(destination, message);

        Ok(())
    }

    /// Receive SMS (very limited bandwidth, emergency fallback)
    #[allow(dead_code)]
    async fn receive_sms(&self) -> Result<(String, String)> {
        if self.config.modem_type != ModemType::GsmModule {
            return Err(NetworkError::Other(
                "SMS only supported on GSM modems".to_string(),
            ));
        }

        // In real implementation:
        // - Set SMS text mode
        // - Read all messages with AT+CMGL
        // - Parse PDU or text

        SmsCodec::parse_pdu(&[])
    }

    /// Get connection duration in seconds
    #[allow(dead_code)]
    async fn get_call_duration(&self) -> u64 {
        if let Some(start_time) = *self.call_start_time.read().await {
            start_time.elapsed().as_secs()
        } else {
            0
        }
    }
}

#[async_trait::async_trait]
impl NetworkAdapter for DialupAdapter {
    async fn initialize(&mut self) -> Result<()> {
        {
            let mut status = self.status.write().await;
            *status = AdapterStatus::Initializing;
        }

        // Initialize modem
        if let Err(e) = self.initialize_modem().await {
            let mut status = self.status.write().await;
            *status = AdapterStatus::Error;
            return Err(e);
        }

        // Try to establish connection
        if let Err(e) = self.dial().await {
            let mut status = self.status.write().await;
            *status = AdapterStatus::Error;
            return Err(e);
        }

        // Establish PPP if needed (PSTN modems)
        if self.config.modem_type != ModemType::GsmModule {
            if let Err(e) = self.establish_ppp().await {
                let mut status = self.status.write().await;
                *status = AdapterStatus::Error;
                return Err(e);
            }
        }

        let mut status = self.status.write().await;
        *status = AdapterStatus::Ready;
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let status = self.status.read().await;
        match *status {
            AdapterStatus::Ready => {
                // Spawn RX task to monitor incoming data
                let _incoming_tx = self.incoming_tx.clone();
                let state = self.state.clone();
                let ppp_session = self.ppp_session.clone();

                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(Duration::from_millis(100)).await;

                        // Check if connected
                        let is_connected = state.read().await.connected;
                        if !is_connected {
                            break;
                        }

                        // In real implementation:
                        // - Read data from serial port
                        // - Decapsulate PPP frame
                        // - Parse frame and send to incoming_tx

                        // Mock: simulate occasional frame reception
                        if let Some(ref session) = *ppp_session.read().await {
                            if session.state == PppState::Established {
                                // Mock frame reception (very rare)
                            }
                        }
                    }
                });

                Ok(())
            }
            _ => Err(NetworkError::AdapterNotReady),
        }
    }

    async fn stop(&mut self) -> Result<()> {
        let mut status = self.status.write().await;
        *status = AdapterStatus::ShuttingDown;

        // Hang up connection
        let _ = self.hang_up().await;

        *status = AdapterStatus::Uninitialized;
        Ok(())
    }

    async fn send(&self, _destination: &Address, frame: &Frame) -> Result<()> {
        let state = self.state.read().await;
        if !state.connected {
            return Err(NetworkError::AdapterNotReady);
        }

        // Serialize frame
        let data =
            bincode::serialize(frame).map_err(|e| NetworkError::SendFailed(e.to_string()))?;

        // Encapsulate in PPP if session exists
        let ppp_session = self.ppp_session.read().await;
        let packet = if let Some(ref session) = *ppp_session {
            if session.state == PppState::Established {
                session.encapsulate(&data)
            } else {
                data
            }
        } else {
            data
        };

        // In real implementation: send packet over serial connection
        // For now, just update stats
        drop(state);
        let mut state = self.state.write().await;
        state.bytes_sent += packet.len() as u64;

        Ok(())
    }

    async fn receive(&self, timeout_ms: u64) -> Result<(Address, Frame)> {
        let timeout = Duration::from_millis(timeout_ms);
        let mut rx_guard = self.rx.write().await;

        if let Some(ref mut rx) = *rx_guard {
            match tokio::time::timeout(timeout, rx.recv()).await {
                Ok(Some((addr, frame))) => {
                    // Update stats
                    let mut state = self.state.write().await;
                    state.bytes_received += 1500; // Approximate
                    Ok((addr, frame))
                }
                Ok(None) => Err(NetworkError::ReceiveFailed("Channel closed".to_string())),
                Err(_) => Err(NetworkError::Timeout),
            }
        } else {
            Err(NetworkError::AdapterNotReady)
        }
    }

    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        // Dial-up is point-to-point, no peer discovery
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

    async fn test_connection(&self, _destination: &Address) -> Result<TestResults> {
        let state = self.state.read().await;

        let rtt_ms = if state.connected {
            Some(self.capabilities.typical_latency_ms)
        } else {
            None
        };

        let error = if !state.connected {
            Some("Not connected".to_string())
        } else {
            None
        };

        Ok(TestResults {
            success: state.connected,
            rtt_ms,
            error,
        })
    }

    fn get_local_address(&self) -> Option<Address> {
        let state = self.state.try_read().ok()?;
        state
            .ip_address
            .as_ref()
            .map(|ip| Address::Dialup(ip.clone()))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        // Format: "dialup://phone_number@isp" or just phone number
        if let Some(stripped) = addr_str.strip_prefix("dialup://") {
            let phone = stripped.split('@').next().unwrap_or(stripped);
            Ok(Address::Dialup(phone.to_string()))
        } else {
            Ok(Address::Dialup(addr_str.to_string()))
        }
    }

    fn supports_address(&self, address: &Address) -> bool {
        matches!(address, Address::Dialup(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dialup_creation() {
        let config = DialupConfig::default();
        let adapter = DialupAdapter::new(config);
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
    }

    #[tokio::test]
    async fn test_dialup_capabilities() {
        let adapter = DialupAdapter::new(DialupConfig::default());
        let caps = adapter.get_capabilities();

        assert_eq!(caps.adapter_type, AdapterType::Dialup);
        assert_eq!(caps.max_message_size, 1500);
        assert!(!caps.supports_broadcast);
    }

    #[tokio::test]
    async fn test_modem_types() {
        let config_hayes = DialupConfig {
            modem_type: ModemType::SerialHayes,
            ..Default::default()
        };
        let adapter = DialupAdapter::new(config_hayes);
        assert_eq!(adapter.get_capabilities().typical_bandwidth_bps, 2400);

        let config_usb = DialupConfig {
            modem_type: ModemType::UsbModem,
            ..Default::default()
        };
        let adapter = DialupAdapter::new(config_usb);
        assert_eq!(adapter.get_capabilities().typical_bandwidth_bps, 56000);

        let config_gsm = DialupConfig {
            modem_type: ModemType::GsmModule,
            ..Default::default()
        };
        let adapter = DialupAdapter::new(config_gsm);
        assert_eq!(adapter.get_capabilities().typical_bandwidth_bps, 115200);
    }

    #[tokio::test]
    async fn test_mock_modem_controller() {
        let mut modem = MockModemController::new();

        let response = modem.send_at_command("AT", 1000).unwrap();
        assert_eq!(response, "OK");

        let response = modem.send_at_command("ATE0", 1000).unwrap();
        assert_eq!(response, "OK");

        let signal = modem.get_signal_quality();
        assert!(signal.is_some());
        assert_eq!(signal.unwrap(), 25);
    }

    #[tokio::test]
    async fn test_ppp_session() {
        let mut ppp = PppSession::new();

        assert_eq!(ppp.state, PppState::Idle);

        ppp.negotiate_lcp().await.unwrap();
        ppp.authenticate_pap("user", "pass").await.unwrap();
        ppp.negotiate_ipcp().await.unwrap();

        assert_eq!(ppp.state, PppState::Established);
        assert!(ppp.local_ip.is_some());

        // Test encapsulation/decapsulation
        let data = b"Hello, PPP!";
        let frame = ppp.encapsulate(data);
        assert!(frame.len() > data.len()); // Added headers
        assert_eq!(frame[0], 0x7E); // Flag
        assert_eq!(frame[frame.len() - 1], 0x7E); // Closing flag

        let decapsulated = ppp.decapsulate(&frame).unwrap();
        assert_eq!(decapsulated, data);
    }

    #[tokio::test]
    async fn test_ppp_auth_failure() {
        let mut ppp = PppSession::new();

        ppp.negotiate_lcp().await.unwrap();

        // Empty credentials should fail
        let result = ppp.authenticate_pap("", "").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sms_codec() {
        let message = "Emergency beacon";
        let encoded = SmsCodec::encode_gsm7(message);
        let decoded = SmsCodec::decode_gsm7(&encoded);
        assert_eq!(decoded, message);

        let pdu = SmsCodec::create_pdu("+1234567890", message);
        assert!(!pdu.is_empty());
    }

    #[tokio::test]
    async fn test_dialup_initialization() {
        let config = DialupConfig::default();
        let mut adapter = DialupAdapter::new(config);

        let result = adapter.initialize().await;
        assert!(result.is_ok());
        assert_eq!(adapter.get_status(), AdapterStatus::Ready);

        // Check that IP was assigned
        let state = adapter.state.read().await;
        assert!(state.ip_address.is_some());
    }

    #[tokio::test]
    async fn test_dialup_start_stop() {
        let config = DialupConfig::default();
        let mut adapter = DialupAdapter::new(config);

        adapter.initialize().await.unwrap();

        let result = adapter.start().await;
        assert!(result.is_ok());

        let result = adapter.stop().await;
        assert!(result.is_ok());
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
    }

    #[tokio::test]
    async fn test_address_parsing() {
        let adapter = DialupAdapter::new(DialupConfig::default());

        let addr = adapter.parse_address("dialup://5551234567@isp").unwrap();
        assert!(matches!(addr, Address::Dialup(ref s) if s == "5551234567"));

        let addr = adapter.parse_address("5551234567").unwrap();
        assert!(matches!(addr, Address::Dialup(ref s) if s == "5551234567"));

        assert!(adapter.supports_address(&addr));
    }

    #[tokio::test]
    async fn test_connection_test() {
        let config = DialupConfig::default();
        let mut adapter = DialupAdapter::new(config);

        adapter.initialize().await.unwrap();

        let test_addr = Address::Dialup("test".to_string());
        let results = adapter.test_connection(&test_addr).await.unwrap();

        assert!(results.success);
        assert!(results.rtt_ms.is_some());
        assert!(results.error.is_none());
    }

    #[tokio::test]
    async fn test_send_requires_connection() {
        use myriadmesh_protocol::{MessageId, MessageType, NodeId};

        let adapter = DialupAdapter::new(DialupConfig::default());

        let frame = Frame::new(
            MessageType::Data,
            NodeId::from_bytes([1; 64]),
            NodeId::from_bytes([2; 64]),
            b"test".to_vec(),
            MessageId::from_bytes([0; 16]),
            0,
        )
        .unwrap();
        let addr = Address::Dialup("test".to_string());

        // Should fail - not connected
        let result = adapter.send(&addr, &frame).await;
        assert!(result.is_err());
    }
}
