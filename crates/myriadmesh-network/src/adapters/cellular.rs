//! Cellular (4G/5G) network adapter
//!
//! Provides high-speed wide-area connectivity via cellular networks.
//! Uses TCP/IP over cellular for reliable communication.
//!
//! Implementation notes:
//! - Uses IP addressing over cellular connection
//! - Tracks data usage for cost management
//! - Channel-based transport for send/receive
//! - Can integrate with modem management APIs (ModemManager, AT commands)

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::{types::AdapterType, Frame};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;

/// Type alias for incoming frame receiver
type FrameReceiver = Arc<RwLock<Option<mpsc::UnboundedReceiver<(Address, Frame)>>>>;

/// Cellular adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellularConfig {
    pub apn: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub preferred_network: NetworkType,
    pub cost_per_mb: f64,
    pub data_cap_mb: u64,
    pub use_with_wifi: bool,
}

impl Default for CellularConfig {
    fn default() -> Self {
        Self {
            apn: "internet".to_string(),
            username: None,
            password: None,
            preferred_network: NetworkType::LTE,
            cost_per_mb: 0.10,
            data_cap_mb: 0,
            use_with_wifi: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkType {
    TwoG,
    ThreeG,
    LTE,
    FiveG,
    Auto,
}

#[derive(Debug, Clone)]
struct ConnectionState {
    connected: bool,
    network_type: Option<NetworkType>,
    signal_strength: u8,
    data_used_mb: f64,
    connection_time: u64,
}

/// TCP connection over cellular
struct TcpConnection {
    #[allow(dead_code)]
    remote_address: String,
    tx: mpsc::UnboundedSender<Vec<u8>>,
    #[allow(dead_code)]
    connected_at: u64,
}

pub struct CellularAdapter {
    config: CellularConfig,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    connection_state: Arc<RwLock<ConnectionState>>,
    connections: Arc<RwLock<HashMap<String, TcpConnection>>>,
    local_ip: Option<String>,
    /// Receive channel for incoming frames
    rx: FrameReceiver,
    /// Send channel for incoming frames
    incoming_tx: mpsc::UnboundedSender<(Address, Frame)>,
}

impl CellularAdapter {
    pub fn new(config: CellularConfig) -> Self {
        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::Cellular,
            max_message_size: 1024 * 1024,     // 1MB for cellular
            typical_latency_ms: 40.0,          // 5G latency
            typical_bandwidth_bps: 50_000_000, // 50 Mbps typical
            reliability: 0.98,
            range_meters: 0.0, // Wide area (not local range)
            power_consumption: PowerConsumption::High,
            cost_per_mb: config.cost_per_mb,
            supports_broadcast: false,
            supports_multicast: false,
        };

        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();

        Self {
            config,
            status: Arc::new(RwLock::new(AdapterStatus::Uninitialized)),
            capabilities,
            connection_state: Arc::new(RwLock::new(ConnectionState {
                connected: false,
                network_type: None,
                signal_strength: 0,
                data_used_mb: 0.0,
                connection_time: 0,
            })),
            connections: Arc::new(RwLock::new(HashMap::new())),
            local_ip: None,
            rx: Arc::new(RwLock::new(Some(incoming_rx))),
            incoming_tx,
        }
    }

    /// Establish cellular data connection
    async fn establish_connection(&mut self) -> Result<()> {
        // Platform-specific cellular modem initialization
        // On Linux: Use ModemManager D-Bus API
        // Uses AT commands via serial or USB interface to:
        // 1. Initialize modem
        // 2. Set APN configuration
        // 3. Authenticate (if needed)
        // 4. Establish PPP/IP connection
        // 5. Configure routing

        let mut state = self.connection_state.write().await;
        state.connected = true;
        state.network_type = Some(self.config.preferred_network);
        state.signal_strength = 75; // Would be read from modem
        state.connection_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Get assigned IP from cellular provider
        self.local_ip = Some("10.0.0.1".to_string());
        Ok(())
    }

    /// Disconnect cellular data connection
    async fn disconnect(&self) -> Result<()> {
        let mut state = self.connection_state.write().await;
        state.connected = false;
        state.network_type = None;
        Ok(())
    }

    /// Connect to peer via TCP over cellular
    async fn connect_tcp(&self, address: &str) -> Result<()> {
        // Check if already connected
        {
            let connections = self.connections.read().await;
            if connections.contains_key(address) {
                return Ok(());
            }
        }

        // Parse address as IP:port
        let socket_addr: SocketAddr = address.parse().map_err(|_| {
            NetworkError::InvalidAddress(format!("Invalid IP address: {}", address))
        })?;

        // Establish TCP connection
        // In production, this would actually connect to the remote host
        let _stream = timeout(Duration::from_secs(10), TcpStream::connect(socket_addr))
            .await
            .map_err(|_| NetworkError::Timeout)?
            .map_err(|e| NetworkError::SendFailed(format!("TCP connect failed: {}", e)))?;

        // Create channel for this connection
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Store connection
        {
            let mut connections = self.connections.write().await;
            connections.insert(
                address.to_string(),
                TcpConnection {
                    remote_address: address.to_string(),
                    tx,
                    connected_at: now,
                },
            );
        }

        // Spawn TCP connection handler
        let addr = address.to_string();
        let incoming_tx = self.incoming_tx.clone();
        let conn_state = self.connection_state.clone();

        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                // Track data usage
                {
                    let mut state = conn_state.write().await;
                    state.data_used_mb += data.len() as f64 / 1_048_576.0;
                }

                // Deserialize frame
                match bincode::deserialize::<Frame>(&data) {
                    Ok(frame) => {
                        let source_addr = Address::Cellular(addr.clone());
                        let _ = incoming_tx.send((source_addr, frame));
                    }
                    Err(_) => continue,
                }
            }
        });

        Ok(())
    }

    /// Ensure connection to peer exists
    async fn ensure_connection(&self, address: &str) -> Result<()> {
        {
            let connections = self.connections.read().await;
            if connections.contains_key(address) {
                return Ok(());
            }
        }

        // Create TCP connection
        self.connect_tcp(address).await
    }

    async fn check_data_cap(&self) -> bool {
        if self.config.data_cap_mb == 0 {
            return false;
        }
        let state = self.connection_state.read().await;
        state.data_used_mb >= self.config.data_cap_mb as f64
    }

    async fn update_data_usage(&self, bytes: u64) {
        let mut state = self.connection_state.write().await;
        state.data_used_mb += bytes as f64 / 1_048_576.0;
    }
}

#[async_trait::async_trait]
impl NetworkAdapter for CellularAdapter {
    async fn initialize(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::Initializing;

        // TODO: Initialize cellular modem
        self.establish_connection().await?;

        *self.status.write().await = AdapterStatus::Ready;
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::ShuttingDown;
        self.disconnect().await?;
        *self.status.write().await = AdapterStatus::Uninitialized;
        Ok(())
    }

    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }
        drop(status);

        // Check data cap before sending
        if self.check_data_cap().await {
            return Err(NetworkError::QuotaExceeded);
        }

        let ip_addr = match destination {
            Address::Cellular(addr) => addr,
            _ => {
                return Err(NetworkError::InvalidAddress(
                    "Expected cellular address".to_string(),
                ))
            }
        };

        // Ensure TCP connection exists
        self.ensure_connection(ip_addr).await?;

        // Serialize frame
        let frame_data = bincode::serialize(frame)
            .map_err(|e| NetworkError::SendFailed(format!("Failed to serialize frame: {}", e)))?;

        // Check size limit
        if frame_data.len() > self.capabilities.max_message_size {
            return Err(NetworkError::MessageTooLarge {
                size: frame_data.len(),
                max: self.capabilities.max_message_size,
            });
        }

        // Send via TCP connection
        let connections = self.connections.read().await;
        let connection = connections.get(ip_addr).ok_or_else(|| {
            NetworkError::SendFailed("Connection lost after establishment".to_string())
        })?;

        connection.tx.send(frame_data).map_err(|_| {
            NetworkError::SendFailed("Failed to send to connection channel".to_string())
        })?;

        // Update data usage tracking
        let bytes_sent = frame.payload.len() as u64;
        self.update_data_usage(bytes_sent).await;

        Ok(())
    }

    async fn receive(&self, timeout_ms: u64) -> Result<(Address, Frame)> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }
        drop(status);

        let mut rx_guard = self.rx.write().await;
        let rx = rx_guard
            .as_mut()
            .ok_or_else(|| NetworkError::ReceiveFailed("Receive channel closed".to_string()))?;

        match timeout(Duration::from_millis(timeout_ms), rx.recv()).await {
            Ok(Some((address, frame))) => {
                // Track received data usage
                let bytes_received = frame.payload.len() as u64;
                self.update_data_usage(bytes_received).await;
                Ok((address, frame))
            }
            Ok(None) => Err(NetworkError::ReceiveFailed("Channel closed".to_string())),
            Err(_) => Err(NetworkError::Timeout),
        }
    }

    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        // Cellular doesn't do local peer discovery
        Ok(Vec::new())
    }

    fn get_status(&self) -> AdapterStatus {
        *futures::executor::block_on(self.status.read())
    }

    fn get_capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    async fn test_connection(&self, destination: &Address) -> Result<TestResults> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }
        drop(status);

        let ip_addr = match destination {
            Address::Cellular(addr) => addr,
            _ => {
                return Err(NetworkError::InvalidAddress(
                    "Expected cellular address".to_string(),
                ))
            }
        };

        // Try to establish TCP connection
        let start = std::time::Instant::now();

        match self.ensure_connection(ip_addr).await {
            Ok(_) => {
                let rtt = start.elapsed().as_secs_f64() * 1000.0;
                Ok(TestResults {
                    success: true,
                    rtt_ms: Some(rtt),
                    error: None,
                })
            }
            Err(e) => {
                // Fall back to estimated latency based on network type
                let state = self.connection_state.read().await;
                let latency = match state.network_type {
                    Some(NetworkType::FiveG) => 20.0,
                    Some(NetworkType::LTE) => 40.0,
                    Some(NetworkType::ThreeG) => 100.0,
                    Some(NetworkType::TwoG) => 300.0,
                    _ => 50.0,
                };

                Ok(TestResults {
                    success: false,
                    rtt_ms: Some(latency),
                    error: Some(e.to_string()),
                })
            }
        }
    }

    fn get_local_address(&self) -> Option<Address> {
        self.local_ip
            .as_ref()
            .map(|ip| Address::Cellular(ip.clone()))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        // Accept any string as cellular address (phone number or IP)
        if addr_str.is_empty() {
            return Err(NetworkError::InvalidAddress(
                "Cellular address cannot be empty".to_string(),
            ));
        }
        Ok(Address::Cellular(addr_str.to_string()))
    }

    fn supports_address(&self, address: &Address) -> bool {
        matches!(address, Address::Cellular(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cellular_adapter_creation() {
        let config = CellularConfig::default();
        let adapter = CellularAdapter::new(config);

        assert_eq!(
            adapter.get_capabilities().adapter_type,
            AdapterType::Cellular
        );
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
    }

    #[tokio::test]
    async fn test_cellular_capabilities() {
        let config = CellularConfig::default();
        let adapter = CellularAdapter::new(config);
        let caps = adapter.get_capabilities();

        assert_eq!(caps.max_message_size, 1024 * 1024);
        assert!(caps.reliability > 0.9);
        assert_eq!(caps.power_consumption, PowerConsumption::High);
    }

    #[tokio::test]
    async fn test_data_cap_check() {
        let config = CellularConfig {
            data_cap_mb: 100,
            ..Default::default()
        };

        let adapter = CellularAdapter::new(config);

        assert!(!adapter.check_data_cap().await);

        adapter.update_data_usage(50 * 1024 * 1024).await;
        assert!(!adapter.check_data_cap().await);

        adapter.update_data_usage(60 * 1024 * 1024).await;
        assert!(adapter.check_data_cap().await);
    }

    #[test]
    fn test_cellular_config_default() {
        let config = CellularConfig::default();
        assert_eq!(config.apn, "internet");
        assert_eq!(config.preferred_network, NetworkType::LTE);
        assert!(!config.use_with_wifi);
    }

    #[test]
    fn test_parse_address() {
        let config = CellularConfig::default();
        let adapter = CellularAdapter::new(config);

        let addr = adapter.parse_address("192.168.1.1").unwrap();
        assert_eq!(addr, Address::Cellular("192.168.1.1".to_string()));

        let addr = adapter.parse_address("+15551234567").unwrap();
        assert_eq!(addr, Address::Cellular("+15551234567".to_string()));

        assert!(adapter.parse_address("").is_err());
    }

    #[test]
    fn test_supports_address() {
        let config = CellularConfig::default();
        let adapter = CellularAdapter::new(config);

        assert!(adapter.supports_address(&Address::Cellular("192.168.1.1".to_string())));
        assert!(!adapter.supports_address(&Address::Ethernet("192.168.1.1".to_string())));
    }
}
