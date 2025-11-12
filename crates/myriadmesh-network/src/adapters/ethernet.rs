//! Ethernet/UDP network adapter
//!
//! Provides UDP-based communication over Ethernet networks with:
//! - Unicast messaging
//! - Multicast peer discovery
//! - IPv4 and IPv6 support

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::types::AdapterType;
use myriadmesh_protocol::{Frame, Message, MessageType, NodeId};
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket as TokioUdpSocket;
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;

/// Default UDP port for MyriadMesh
pub const DEFAULT_PORT: u16 = 4001;

/// Multicast group for peer discovery (IPv4)
pub const MULTICAST_ADDR: &str = "239.255.42.1";
pub const MULTICAST_PORT: u16 = 4002;

/// Maximum UDP packet size (typical MTU minus headers)
pub const MAX_UDP_SIZE: usize = 1400;

/// Ethernet/UDP adapter configuration
#[derive(Debug, Clone)]
pub struct EthernetConfig {
    /// Local bind address (0.0.0.0 for all interfaces)
    pub bind_addr: String,

    /// Local port (0 for OS-assigned)
    pub port: u16,

    /// Enable multicast peer discovery
    pub enable_multicast: bool,

    /// Multicast address for discovery
    pub multicast_addr: String,

    /// Multicast port
    pub multicast_port: u16,

    /// Discovery interval in seconds
    pub discovery_interval: u64,
}

impl Default for EthernetConfig {
    fn default() -> Self {
        EthernetConfig {
            bind_addr: "0.0.0.0".to_string(),
            port: DEFAULT_PORT,
            enable_multicast: true,
            multicast_addr: MULTICAST_ADDR.to_string(),
            multicast_port: MULTICAST_PORT,
            discovery_interval: 60,
        }
    }
}

/// Ethernet/UDP network adapter
pub struct EthernetAdapter {
    /// Adapter status
    status: Arc<RwLock<AdapterStatus>>,

    /// Configuration
    config: EthernetConfig,

    /// Local NodeId
    local_node_id: NodeId,

    /// UDP socket for messaging
    socket: Arc<Mutex<Option<TokioUdpSocket>>>,

    /// Multicast socket for discovery
    multicast_socket: Arc<Mutex<Option<UdpSocket>>>,

    /// Local address
    local_addr: Arc<RwLock<Option<SocketAddr>>>,

    /// Discovered peers
    peers: Arc<RwLock<Vec<PeerInfo>>>,

    /// Adapter capabilities
    capabilities: AdapterCapabilities,
}

impl EthernetAdapter {
    /// Create new Ethernet adapter
    pub fn new(local_node_id: NodeId, config: EthernetConfig) -> Self {
        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::Ethernet,
            max_message_size: MAX_UDP_SIZE,
            typical_latency_ms: 5.0,
            typical_bandwidth_bps: 100_000_000, // 100 Mbps
            reliability: 0.99,
            range_meters: 100.0,
            power_consumption: PowerConsumption::Medium,
            cost_per_mb: 0.0,
            supports_broadcast: false,
            supports_multicast: config.enable_multicast,
        };

        EthernetAdapter {
            status: Arc::new(RwLock::new(AdapterStatus::Uninitialized)),
            config,
            local_node_id,
            socket: Arc::new(Mutex::new(None)),
            multicast_socket: Arc::new(Mutex::new(None)),
            local_addr: Arc::new(RwLock::new(None)),
            peers: Arc::new(RwLock::new(Vec::new())),
            capabilities,
        }
    }

    /// Create with default configuration
    pub fn new_default(local_node_id: NodeId) -> Self {
        Self::new(local_node_id, EthernetConfig::default())
    }

    /// Get local socket address
    pub fn local_address(&self) -> Option<SocketAddr> {
        *self.local_addr.try_read().ok()?
    }

    /// Setup multicast socket for peer discovery
    fn setup_multicast(&mut self) -> Result<()> {
        if !self.config.enable_multicast {
            return Ok(());
        }

        let multicast_addr: Ipv4Addr = self.config.multicast_addr.parse().map_err(|e| {
            NetworkError::InvalidAddress(format!("Invalid multicast address: {}", e))
        })?;

        let socket =
            UdpSocket::bind(format!("0.0.0.0:{}", self.config.multicast_port)).map_err(|e| {
                NetworkError::InitializationFailed(format!(
                    "Failed to bind multicast socket: {}",
                    e
                ))
            })?;

        socket
            .join_multicast_v4(&multicast_addr, &Ipv4Addr::UNSPECIFIED)
            .map_err(|e| {
                NetworkError::InitializationFailed(format!("Failed to join multicast group: {}", e))
            })?;

        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .map_err(|e| {
                NetworkError::InitializationFailed(format!("Failed to set timeout: {}", e))
            })?;

        *self.multicast_socket.blocking_lock() = Some(socket);

        Ok(())
    }

    /// Send multicast discovery announcement
    async fn send_discovery_announcement(&self) -> Result<()> {
        if !self.config.enable_multicast {
            return Ok(());
        }

        let multicast_guard = self.multicast_socket.lock().await;
        let socket = multicast_guard.as_ref().ok_or_else(|| {
            NetworkError::InitializationFailed("Multicast socket not initialized".to_string())
        })?;

        // Simple discovery message with NodeId
        let message = format!(
            "MYRIADMESH_DISCOVER:{}",
            hex::encode(self.local_node_id.as_bytes())
        );
        let dest = format!(
            "{}:{}",
            self.config.multicast_addr, self.config.multicast_port
        );

        socket
            .send_to(message.as_bytes(), dest)
            .map_err(|e| NetworkError::SendFailed(format!("Multicast send failed: {}", e)))?;

        Ok(())
    }

    /// Listen for multicast discovery messages (non-blocking)
    fn receive_discovery_messages(&self) -> Result<Vec<PeerInfo>> {
        if !self.config.enable_multicast {
            return Ok(Vec::new());
        }

        let multicast_guard = self.multicast_socket.blocking_lock();
        let socket = multicast_guard.as_ref().ok_or_else(|| {
            NetworkError::InitializationFailed("Multicast socket not initialized".to_string())
        })?;

        let mut discovered = Vec::new();
        let mut buf = [0u8; 1024];

        // Non-blocking receive with timeout
        loop {
            match socket.recv_from(&mut buf) {
                Ok((size, source_addr)) => {
                    if let Ok(message) = std::str::from_utf8(&buf[..size]) {
                        if let Some(node_id_hex) = message.strip_prefix("MYRIADMESH_DISCOVER:") {
                            if let Ok(node_id_bytes) = hex::decode(node_id_hex) {
                                if node_id_bytes.len() == 32 {
                                    let mut bytes = [0u8; 32];
                                    bytes.copy_from_slice(&node_id_bytes);
                                    let node_id = NodeId::from_bytes(bytes);

                                    // Don't add ourselves
                                    if node_id != self.local_node_id {
                                        discovered.push(PeerInfo {
                                            node_id,
                                            address: Address::Ethernet(source_addr.to_string()),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No more messages
                    break;
                }
                Err(_) => {
                    // Other errors, stop receiving
                    break;
                }
            }
        }

        Ok(discovered)
    }
}

#[async_trait::async_trait]
impl NetworkAdapter for EthernetAdapter {
    async fn initialize(&mut self) -> Result<()> {
        {
            let mut status = self.status.write().await;
            *status = AdapterStatus::Initializing;
        }

        // Bind UDP socket
        let bind_addr = format!("{}:{}", self.config.bind_addr, self.config.port);
        let std_socket = std::net::UdpSocket::bind(&bind_addr).map_err(|e| {
            NetworkError::InitializationFailed(format!("Failed to bind UDP socket: {}", e))
        })?;

        std_socket.set_nonblocking(true).map_err(|e| {
            NetworkError::InitializationFailed(format!("Failed to set non-blocking: {}", e))
        })?;

        let socket = TokioUdpSocket::from_std(std_socket).map_err(|e| {
            NetworkError::InitializationFailed(format!("Failed to create tokio socket: {}", e))
        })?;

        let local_addr = socket.local_addr().map_err(|e| {
            NetworkError::InitializationFailed(format!("Failed to get local address: {}", e))
        })?;

        {
            let mut addr = self.local_addr.write().await;
            *addr = Some(local_addr);
        }

        *self.socket.lock().await = Some(socket);

        // Setup multicast
        self.setup_multicast()?;

        {
            let mut status = self.status.write().await;
            *status = AdapterStatus::Ready;
        }

        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        // Send initial discovery announcement
        if self.config.enable_multicast {
            self.send_discovery_announcement().await?;
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        {
            let mut status = self.status.write().await;
            *status = AdapterStatus::ShuttingDown;
        }

        // Close sockets
        *self.socket.lock().await = None;
        *self.multicast_socket.lock().await = None;

        Ok(())
    }

    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()> {
        let socket_guard = self.socket.lock().await;
        let socket = socket_guard
            .as_ref()
            .ok_or_else(|| NetworkError::SendFailed("Socket not initialized".to_string()))?;

        // Extract socket address from destination
        let dest_addr = match destination {
            Address::Ethernet(addr_str) => addr_str.parse::<SocketAddr>().map_err(|e| {
                NetworkError::InvalidAddress(format!("Invalid Ethernet address: {}", e))
            })?,
            _ => {
                return Err(NetworkError::InvalidAddress(
                    "Ethernet adapter requires Ethernet address".to_string(),
                ))
            }
        };

        // Serialize frame
        let data = bincode::serialize(frame)
            .map_err(|e| NetworkError::SendFailed(format!("Failed to serialize frame: {}", e)))?;

        if data.len() > MAX_UDP_SIZE {
            return Err(NetworkError::MessageTooLarge {
                size: data.len(),
                max: MAX_UDP_SIZE,
            });
        }

        // Send UDP packet
        socket
            .send_to(&data, dest_addr)
            .await
            .map_err(|e| NetworkError::SendFailed(format!("UDP send failed: {}", e)))?;

        Ok(())
    }

    async fn receive(&self, timeout_ms: u64) -> Result<(Address, Frame)> {
        let socket_guard = self.socket.lock().await;
        let socket = socket_guard
            .as_ref()
            .ok_or_else(|| NetworkError::ReceiveFailed("Socket not initialized".to_string()))?;

        let mut buf = vec![0u8; MAX_UDP_SIZE + 1024];

        let (size, source_addr) = timeout(
            Duration::from_millis(timeout_ms),
            socket.recv_from(&mut buf),
        )
        .await
        .map_err(|_| NetworkError::ReceiveFailed("Receive timeout".to_string()))?
        .map_err(|e| NetworkError::ReceiveFailed(format!("UDP receive failed: {}", e)))?;

        // Deserialize frame
        let frame: Frame = bincode::deserialize(&buf[..size]).map_err(|e| {
            NetworkError::ReceiveFailed(format!("Failed to deserialize frame: {}", e))
        })?;

        let source_address = Address::Ethernet(source_addr.to_string());

        Ok((source_address, frame))
    }

    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        // Send discovery announcement
        self.send_discovery_announcement().await?;

        // Wait a bit for responses
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Collect discovery messages
        let discovered = self.receive_discovery_messages()?;

        // Update peers list
        {
            let mut peers = self.peers.write().await;

            for peer in &discovered {
                // Add if not already in list
                if !peers.iter().any(|p| p.node_id == peer.node_id) {
                    peers.push(peer.clone());
                }
            }
        }

        Ok(discovered)
    }

    fn get_status(&self) -> AdapterStatus {
        self.status
            .try_read()
            .map(|s| *s)
            .unwrap_or(AdapterStatus::Error)
    }

    fn get_capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    async fn test_connection(&self, destination: &Address) -> Result<TestResults> {
        // Simple ping test
        let start = std::time::Instant::now();

        // Create a test message
        let test_message = Message::new(
            self.local_node_id,
            self.local_node_id,
            MessageType::Data,
            vec![0u8; 32],
        )
        .map_err(|e| NetworkError::SendFailed(format!("Failed to create test message: {}", e)))?;

        // Create frame from message
        let test_frame = Frame::from_message(&test_message)
            .map_err(|e| NetworkError::SendFailed(format!("Failed to create test frame: {}", e)))?;

        // Try to send (this doesn't guarantee delivery but tests connectivity)
        match self.send(destination, &test_frame).await {
            Ok(_) => {
                let rtt = start.elapsed().as_secs_f64() * 1000.0;
                Ok(TestResults {
                    success: true,
                    rtt_ms: Some(rtt),
                    error: None,
                })
            }
            Err(e) => Ok(TestResults {
                success: false,
                rtt_ms: None,
                error: Some(e.to_string()),
            }),
        }
    }

    fn get_local_address(&self) -> Option<Address> {
        self.local_addr
            .try_read()
            .ok()?
            .as_ref()
            .map(|addr| Address::Ethernet(addr.to_string()))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        addr_str
            .parse::<SocketAddr>()
            .map(|addr| Address::Ethernet(addr.to_string()))
            .map_err(|e| NetworkError::InvalidAddress(format!("Invalid Ethernet address: {}", e)))
    }

    fn supports_address(&self, address: &Address) -> bool {
        matches!(address, Address::Ethernet(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ethernet_config_default() {
        let config = EthernetConfig::default();
        assert_eq!(config.port, DEFAULT_PORT);
        assert!(config.enable_multicast);
    }

    #[test]
    fn test_ethernet_adapter_creation() {
        let node_id = NodeId::from_bytes([1u8; 32]);
        let adapter = EthernetAdapter::new_default(node_id);

        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
        assert_eq!(
            adapter.get_capabilities().adapter_type,
            AdapterType::Ethernet
        );
        assert_eq!(adapter.get_capabilities().max_message_size, MAX_UDP_SIZE);
    }

    #[test]
    fn test_parse_address() {
        let node_id = NodeId::from_bytes([1u8; 32]);
        let adapter = EthernetAdapter::new_default(node_id);

        let result = adapter.parse_address("192.168.1.1:4001");
        assert!(result.is_ok());

        let result = adapter.parse_address("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_supports_address() {
        let node_id = NodeId::from_bytes([1u8; 32]);
        let adapter = EthernetAdapter::new_default(node_id);

        assert!(adapter.supports_address(&Address::Ethernet("192.168.1.1:4001".to_string())));
        assert!(!adapter.supports_address(&Address::Bluetooth("00:11:22:33:44:55".to_string())));
        assert!(!adapter.supports_address(&Address::I2P("test.i2p".to_string())));
    }
}
