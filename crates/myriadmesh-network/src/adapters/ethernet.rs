//! Ethernet/UDP network adapter
//!
//! Provides UDP-based communication over Ethernet networks with:
//! - Unicast messaging
//! - Multicast peer discovery
//! - IPv4 and IPv6 support
//! - SECURITY C3: Authenticated UDP frames with Ed25519 signatures

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_crypto::identity::NodeIdentity;
use myriadmesh_crypto::signing::{sign_message, verify_signature};
use myriadmesh_protocol::types::{AdapterType, NODE_ID_SIZE};
use myriadmesh_protocol::{Frame, Message, MessageType, NodeId};
use sodiumoxide::crypto::sign::ed25519;
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

/// SECURITY C3: Size of Ed25519 public key (32 bytes)
const PUBLIC_KEY_SIZE: usize = 32;

/// SECURITY C3: Size of Ed25519 signature (64 bytes)
const SIGNATURE_SIZE: usize = 64;

/// SECURITY C3: Overhead for authenticated UDP packet (public key + signature)
const AUTH_OVERHEAD: usize = PUBLIC_KEY_SIZE + SIGNATURE_SIZE;

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

/// SECURITY H1: Authenticated discovery message
/// Prevents multicast spoofing attacks where attackers claim to be arbitrary NodeIDs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DiscoveryMessage {
    /// NodeID of the announcing peer (64 bytes)
    #[serde(with = "serde_big_array::BigArray")]
    node_id: [u8; NODE_ID_SIZE],
    /// Ed25519 public key (32 bytes) for verification
    public_key: [u8; PUBLIC_KEY_SIZE],
    /// Ed25519 signature (64 bytes) over (node_id || public_key)
    #[serde(with = "serde_big_array::BigArray")]
    signature: [u8; SIGNATURE_SIZE],
}

/// Ethernet/UDP network adapter
pub struct EthernetAdapter {
    /// Adapter status
    status: Arc<RwLock<AdapterStatus>>,

    /// Configuration
    config: EthernetConfig,

    /// Local NodeId
    local_node_id: NodeId,

    /// SECURITY C3: Node identity for signing UDP packets
    identity: Arc<NodeIdentity>,

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
    /// SECURITY C3: Create new Ethernet adapter with authenticated UDP
    ///
    /// Requires a NodeIdentity for signing outgoing frames and verifying incoming frames.
    pub fn new(identity: Arc<NodeIdentity>, config: EthernetConfig) -> Self {
        let local_node_id = NodeId::from_bytes(*identity.node_id.as_bytes());

        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::Ethernet,
            max_message_size: MAX_UDP_SIZE - AUTH_OVERHEAD, // Account for auth overhead
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
            identity,
            socket: Arc::new(Mutex::new(None)),
            multicast_socket: Arc::new(Mutex::new(None)),
            local_addr: Arc::new(RwLock::new(None)),
            peers: Arc::new(RwLock::new(Vec::new())),
            capabilities,
        }
    }

    /// Create with default configuration
    pub fn new_default(identity: Arc<NodeIdentity>) -> Self {
        Self::new(identity, EthernetConfig::default())
    }

    /// Get local socket address
    pub fn local_address(&self) -> Option<SocketAddr> {
        *self.local_addr.try_read().ok()?
    }

    /// SECURITY C3: Create authenticated UDP packet
    ///
    /// Format: [public_key: 32 bytes][frame_data][signature: 64 bytes]
    fn create_authenticated_packet(&self, frame_data: &[u8]) -> Result<Vec<u8>> {
        // Calculate total size
        let total_size = PUBLIC_KEY_SIZE + frame_data.len() + SIGNATURE_SIZE;
        let mut packet = Vec::with_capacity(total_size);

        // Add public key
        packet.extend_from_slice(self.identity.public_key.as_ref());

        // Add frame data
        packet.extend_from_slice(frame_data);

        // Sign: public_key + frame_data
        let signable_data = &packet[..PUBLIC_KEY_SIZE + frame_data.len()];
        let signature = sign_message(&self.identity, signable_data)
            .map_err(|e| NetworkError::SendFailed(format!("Failed to sign packet: {}", e)))?;

        // Add signature
        packet.extend_from_slice(signature.as_bytes());

        Ok(packet)
    }

    /// SECURITY C3: Verify and extract frame from authenticated UDP packet
    ///
    /// Returns: (source_public_key, frame_data)
    fn verify_authenticated_packet(&self, packet: &[u8]) -> Result<(ed25519::PublicKey, Vec<u8>)> {
        // Check minimum size
        if packet.len() < PUBLIC_KEY_SIZE + SIGNATURE_SIZE {
            return Err(NetworkError::ReceiveFailed(
                "Packet too small for authentication".to_string(),
            ));
        }

        // Extract components
        let public_key_bytes = &packet[..PUBLIC_KEY_SIZE];
        let signature_offset = packet.len() - SIGNATURE_SIZE;
        let frame_data = &packet[PUBLIC_KEY_SIZE..signature_offset];
        let signature_bytes = &packet[signature_offset..];

        // Parse public key
        let public_key = ed25519::PublicKey::from_slice(public_key_bytes).ok_or_else(|| {
            NetworkError::ReceiveFailed("Invalid public key in packet".to_string())
        })?;

        // Parse signature
        let mut sig_array = [0u8; SIGNATURE_SIZE];
        sig_array.copy_from_slice(signature_bytes);
        let signature = myriadmesh_crypto::signing::Signature::from_bytes(sig_array);

        // Verify signature over public_key + frame_data
        let signable_data = &packet[..signature_offset];
        verify_signature(&public_key, signable_data, &signature).map_err(|e| {
            NetworkError::ReceiveFailed(format!("Signature verification failed: {}", e))
        })?;

        // Verify that public key matches claimed source NodeId in frame
        // (This will be done after deserializing the frame)

        Ok((public_key, frame_data.to_vec()))
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

    /// SECURITY H1: Send authenticated multicast discovery announcement
    async fn send_discovery_announcement(&self) -> Result<()> {
        if !self.config.enable_multicast {
            return Ok(());
        }

        let multicast_guard = self.multicast_socket.lock().await;
        let socket = multicast_guard.as_ref().ok_or_else(|| {
            NetworkError::InitializationFailed("Multicast socket not initialized".to_string())
        })?;

        // SECURITY H1: Create signed discovery message
        let node_id_bytes = *self.local_node_id.as_bytes();
        let public_key_bytes = {
            let mut bytes = [0u8; PUBLIC_KEY_SIZE];
            bytes.copy_from_slice(self.identity.public_key.as_ref());
            bytes
        };

        // Sign the message: node_id || public_key
        let mut signable_data = Vec::with_capacity(NODE_ID_SIZE + PUBLIC_KEY_SIZE);
        signable_data.extend_from_slice(&node_id_bytes);
        signable_data.extend_from_slice(&public_key_bytes);

        let signature = sign_message(&self.identity, &signable_data)
            .map_err(|e| NetworkError::SendFailed(format!("Failed to sign discovery: {}", e)))?;

        let signature_bytes = {
            let mut bytes = [0u8; SIGNATURE_SIZE];
            bytes.copy_from_slice(signature.as_bytes());
            bytes
        };

        let discovery_msg = DiscoveryMessage {
            node_id: node_id_bytes,
            public_key: public_key_bytes,
            signature: signature_bytes,
        };

        // Serialize and send
        let serialized = bincode::serialize(&discovery_msg).map_err(|e| {
            NetworkError::SendFailed(format!("Failed to serialize discovery: {}", e))
        })?;

        let dest = format!(
            "{}:{}",
            self.config.multicast_addr, self.config.multicast_port
        );

        socket
            .send_to(&serialized, dest)
            .map_err(|e| NetworkError::SendFailed(format!("Multicast send failed: {}", e)))?;

        Ok(())
    }

    /// SECURITY H1: Listen for authenticated multicast discovery messages (non-blocking)
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
                    // SECURITY H1: Verify signed discovery message
                    match self.verify_discovery_message(&buf[..size]) {
                        Ok(node_id) => {
                            // Don't add ourselves
                            if node_id != self.local_node_id {
                                discovered.push(PeerInfo {
                                    node_id,
                                    address: Address::Ethernet(source_addr.to_string()),
                                });
                            }
                        }
                        Err(_) => {
                            // Ignore invalid/unsigned discovery messages
                            continue;
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

    /// SECURITY H1: Verify discovery message signature and return NodeId
    fn verify_discovery_message(&self, data: &[u8]) -> Result<NodeId> {
        // Deserialize discovery message
        let msg: DiscoveryMessage = bincode::deserialize(data)
            .map_err(|e| NetworkError::ReceiveFailed(format!("Invalid discovery format: {}", e)))?;

        // Parse public key
        let public_key = ed25519::PublicKey::from_slice(&msg.public_key).ok_or_else(|| {
            NetworkError::ReceiveFailed("Invalid public key in discovery".to_string())
        })?;

        // Reconstruct signed data: node_id || public_key
        let mut signable_data = Vec::with_capacity(NODE_ID_SIZE + PUBLIC_KEY_SIZE);
        signable_data.extend_from_slice(&msg.node_id);
        signable_data.extend_from_slice(&msg.public_key);

        // Verify signature
        let signature = myriadmesh_crypto::signing::Signature::from_bytes(msg.signature);
        verify_signature(&public_key, &signable_data, &signature).map_err(|e| {
            NetworkError::ReceiveFailed(format!("Discovery signature invalid: {}", e))
        })?;

        // SECURITY H1: Verify that public key derives to claimed NodeId (prevents impersonation)
        let derived_node_id = NodeIdentity::derive_node_id(&public_key);
        let claimed_node_id = NodeId::from_bytes(msg.node_id);

        if derived_node_id.as_bytes() != claimed_node_id.as_bytes() {
            return Err(NetworkError::ReceiveFailed(
                "Discovery public key does not derive to claimed NodeId".to_string(),
            ));
        }

        Ok(claimed_node_id)
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
        let frame_data = bincode::serialize(frame)
            .map_err(|e| NetworkError::SendFailed(format!("Failed to serialize frame: {}", e)))?;

        // SECURITY C3: Create authenticated packet
        let authenticated_packet = self.create_authenticated_packet(&frame_data)?;

        if authenticated_packet.len() > MAX_UDP_SIZE {
            return Err(NetworkError::MessageTooLarge {
                size: authenticated_packet.len(),
                max: MAX_UDP_SIZE,
            });
        }

        // Send authenticated UDP packet
        socket
            .send_to(&authenticated_packet, dest_addr)
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

        // SECURITY C3: Verify authenticated packet
        let (source_public_key, frame_data) = self.verify_authenticated_packet(&buf[..size])?;

        // Deserialize frame
        let frame: Frame = bincode::deserialize(&frame_data).map_err(|e| {
            NetworkError::ReceiveFailed(format!("Failed to deserialize frame: {}", e))
        })?;

        // SECURITY C3: Verify that public key matches frame's source NodeId
        let claimed_node_id = NodeIdentity::derive_node_id(&source_public_key);
        let frame_source_id_bytes = frame.header.source.as_bytes();
        if claimed_node_id.as_bytes() != frame_source_id_bytes {
            return Err(NetworkError::ReceiveFailed(
                "Source public key does not match frame source NodeId".to_string(),
            ));
        }

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
        myriadmesh_crypto::init().unwrap();
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        let adapter = EthernetAdapter::new_default(identity);

        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
        assert_eq!(
            adapter.get_capabilities().adapter_type,
            AdapterType::Ethernet
        );
        // SECURITY C3: Max size reduced by authentication overhead
        assert_eq!(
            adapter.get_capabilities().max_message_size,
            MAX_UDP_SIZE - AUTH_OVERHEAD
        );
    }

    #[test]
    fn test_parse_address() {
        myriadmesh_crypto::init().unwrap();
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        let adapter = EthernetAdapter::new_default(identity);

        let result = adapter.parse_address("192.168.1.1:4001");
        assert!(result.is_ok());

        let result = adapter.parse_address("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_supports_address() {
        myriadmesh_crypto::init().unwrap();
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        let adapter = EthernetAdapter::new_default(identity);

        assert!(adapter.supports_address(&Address::Ethernet("192.168.1.1:4001".to_string())));
        assert!(!adapter.supports_address(&Address::Bluetooth("00:11:22:33:44:55".to_string())));
        assert!(!adapter.supports_address(&Address::I2P("test.i2p".to_string())));
    }

    // SECURITY C3: Test authenticated packet creation and verification
    #[test]
    fn test_authenticated_packet() {
        myriadmesh_crypto::init().unwrap();
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        let adapter = EthernetAdapter::new_default(identity.clone());

        let frame_data = b"test frame data";

        // Create authenticated packet
        let packet = adapter.create_authenticated_packet(frame_data).unwrap();

        // Verify it has the correct size
        assert_eq!(
            packet.len(),
            PUBLIC_KEY_SIZE + frame_data.len() + SIGNATURE_SIZE
        );

        // Verify the packet
        let (recovered_public_key, recovered_data) =
            adapter.verify_authenticated_packet(&packet).unwrap();

        // Check that data matches
        assert_eq!(recovered_data, frame_data);

        // Check that public key matches
        assert_eq!(recovered_public_key.as_ref(), identity.public_key.as_ref());
    }

    // SECURITY C3: Test that tampered packets are rejected
    #[test]
    fn test_reject_tampered_packet() {
        myriadmesh_crypto::init().unwrap();
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        let adapter = EthernetAdapter::new_default(identity);

        let frame_data = b"test frame data";
        let mut packet = adapter.create_authenticated_packet(frame_data).unwrap();

        // Tamper with the data
        if packet.len() > PUBLIC_KEY_SIZE + 10 {
            packet[PUBLIC_KEY_SIZE + 5] ^= 0xFF;
        }

        // Verification should fail
        assert!(adapter.verify_authenticated_packet(&packet).is_err());
    }

    // SECURITY C3: Test that packets with wrong signature are rejected
    #[test]
    fn test_reject_wrong_signature() {
        myriadmesh_crypto::init().unwrap();
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        let adapter = EthernetAdapter::new_default(identity);

        let frame_data = b"test frame data";
        let mut packet = adapter.create_authenticated_packet(frame_data).unwrap();

        // Corrupt the signature (last 64 bytes)
        let sig_start = packet.len() - SIGNATURE_SIZE;
        packet[sig_start] ^= 0xFF;

        // Verification should fail
        assert!(adapter.verify_authenticated_packet(&packet).is_err());
    }

    // SECURITY H1: Test that valid signed discovery messages are accepted
    #[test]
    fn test_valid_discovery_message() {
        myriadmesh_crypto::init().unwrap();
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        let adapter = EthernetAdapter::new_default(identity.clone());

        let node_id_bytes = *adapter.local_node_id.as_bytes();
        let public_key_bytes = {
            let mut bytes = [0u8; PUBLIC_KEY_SIZE];
            bytes.copy_from_slice(identity.public_key.as_ref());
            bytes
        };

        // Create signed discovery message
        let mut signable_data = Vec::with_capacity(NODE_ID_SIZE + PUBLIC_KEY_SIZE);
        signable_data.extend_from_slice(&node_id_bytes);
        signable_data.extend_from_slice(&public_key_bytes);

        let signature = sign_message(&identity, &signable_data).unwrap();
        let signature_bytes = {
            let mut bytes = [0u8; SIGNATURE_SIZE];
            bytes.copy_from_slice(signature.as_bytes());
            bytes
        };

        let discovery_msg = DiscoveryMessage {
            node_id: node_id_bytes,
            public_key: public_key_bytes,
            signature: signature_bytes,
        };

        let serialized = bincode::serialize(&discovery_msg).unwrap();

        // Verify discovery message
        let verified_node_id = adapter.verify_discovery_message(&serialized).unwrap();
        assert_eq!(verified_node_id, adapter.local_node_id);
    }

    // SECURITY H1: Test that discovery messages with invalid signatures are rejected
    #[test]
    fn test_reject_invalid_discovery_signature() {
        myriadmesh_crypto::init().unwrap();
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        let adapter = EthernetAdapter::new_default(identity.clone());

        let node_id_bytes = *adapter.local_node_id.as_bytes();
        let public_key_bytes = {
            let mut bytes = [0u8; PUBLIC_KEY_SIZE];
            bytes.copy_from_slice(identity.public_key.as_ref());
            bytes
        };

        // Create discovery message with invalid signature
        let mut invalid_signature = [0u8; SIGNATURE_SIZE];
        invalid_signature[0] = 0xFF;

        let discovery_msg = DiscoveryMessage {
            node_id: node_id_bytes,
            public_key: public_key_bytes,
            signature: invalid_signature,
        };

        let serialized = bincode::serialize(&discovery_msg).unwrap();

        // Verification should fail
        assert!(adapter.verify_discovery_message(&serialized).is_err());
    }

    // SECURITY H1: Test that discovery messages with mismatched NodeID are rejected
    #[test]
    fn test_reject_discovery_nodeid_mismatch() {
        myriadmesh_crypto::init().unwrap();
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        let adapter = EthernetAdapter::new_default(identity.clone());

        // Use wrong NodeID (all zeros instead of derived from public key)
        let wrong_node_id_bytes = [0u8; NODE_ID_SIZE];
        let public_key_bytes = {
            let mut bytes = [0u8; PUBLIC_KEY_SIZE];
            bytes.copy_from_slice(identity.public_key.as_ref());
            bytes
        };

        // Sign with wrong node_id
        let mut signable_data = Vec::with_capacity(NODE_ID_SIZE + PUBLIC_KEY_SIZE);
        signable_data.extend_from_slice(&wrong_node_id_bytes);
        signable_data.extend_from_slice(&public_key_bytes);

        let signature = sign_message(&identity, &signable_data).unwrap();
        let signature_bytes = {
            let mut bytes = [0u8; SIGNATURE_SIZE];
            bytes.copy_from_slice(signature.as_bytes());
            bytes
        };

        let discovery_msg = DiscoveryMessage {
            node_id: wrong_node_id_bytes,
            public_key: public_key_bytes,
            signature: signature_bytes,
        };

        let serialized = bincode::serialize(&discovery_msg).unwrap();

        // Verification should fail due to NodeID derivation mismatch
        assert!(adapter.verify_discovery_message(&serialized).is_err());
    }

    // SECURITY H1: Test that tampered discovery messages are rejected
    #[test]
    fn test_reject_tampered_discovery_message() {
        myriadmesh_crypto::init().unwrap();
        let identity = Arc::new(NodeIdentity::generate().unwrap());
        let adapter = EthernetAdapter::new_default(identity.clone());

        let node_id_bytes = *adapter.local_node_id.as_bytes();
        let public_key_bytes = {
            let mut bytes = [0u8; PUBLIC_KEY_SIZE];
            bytes.copy_from_slice(identity.public_key.as_ref());
            bytes
        };

        // Create valid signed discovery message
        let mut signable_data = Vec::with_capacity(NODE_ID_SIZE + PUBLIC_KEY_SIZE);
        signable_data.extend_from_slice(&node_id_bytes);
        signable_data.extend_from_slice(&public_key_bytes);

        let signature = sign_message(&identity, &signable_data).unwrap();
        let signature_bytes = {
            let mut bytes = [0u8; SIGNATURE_SIZE];
            bytes.copy_from_slice(signature.as_bytes());
            bytes
        };

        let discovery_msg = DiscoveryMessage {
            node_id: node_id_bytes,
            public_key: public_key_bytes,
            signature: signature_bytes,
        };

        let mut serialized = bincode::serialize(&discovery_msg).unwrap();

        // Tamper with the serialized data (modify a byte in the middle)
        if serialized.len() > 10 {
            serialized[5] ^= 0xFF;
        }

        // Verification should fail
        assert!(adapter.verify_discovery_message(&serialized).is_err());
    }
}
