//! MyriadMesh Ethernet Adapter
//!
//! UDP-based Ethernet network adapter with multicast peer discovery.
//!
//! # Features
//! - UDP socket communication for frame transmission
//! - Multicast discovery on 239.255.77.77:4001
//! - Automatic peer discovery and tracking
//! - Unicast and broadcast support
//!
//! # Example
//!
//! ```no_run
//! use myriadmesh_adapter_ethernet::{EthernetAdapter, EthernetConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = EthernetConfig::default();
//!     let adapter = EthernetAdapter::new("eth0".to_string(), config).await?;
//!
//!     // Use adapter...
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use bincode::{deserialize, serialize};
use myriadmesh_network::{
    AdapterInfo, AdapterStats, NetworkAdapter, NetworkError, Result,
};
use myriadmesh_protocol::{Frame, NodeId};
use myriadmesh_protocol::types::AdapterType;
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};

/// Default multicast address for MyriadMesh discovery
pub const DEFAULT_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 77, 77);

/// Default multicast port
pub const DEFAULT_MULTICAST_PORT: u16 = 4001;

/// Default unicast base port
pub const DEFAULT_UNICAST_PORT: u16 = 4002;

/// Peer timeout duration (60 seconds)
const PEER_TIMEOUT: Duration = Duration::from_secs(60);

/// Discovery announcement interval (10 seconds)
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(10);

/// Configuration for Ethernet adapter
#[derive(Debug, Clone)]
pub struct EthernetConfig {
    /// Multicast address for discovery
    pub multicast_addr: Ipv4Addr,

    /// Multicast port
    pub multicast_port: u16,

    /// Unicast port for receiving direct messages
    pub unicast_port: u16,

    /// Enable multicast discovery
    pub discovery_enabled: bool,

    /// Interface to bind to (None = all interfaces)
    pub bind_interface: Option<IpAddr>,
}

impl Default for EthernetConfig {
    fn default() -> Self {
        Self {
            multicast_addr: DEFAULT_MULTICAST_ADDR,
            multicast_port: DEFAULT_MULTICAST_PORT,
            unicast_port: DEFAULT_UNICAST_PORT,
            discovery_enabled: true,
            bind_interface: None,
        }
    }
}

/// Peer information
#[derive(Debug, Clone)]
struct PeerInfo {
    node_id: NodeId,
    socket_addr: SocketAddr,
    last_seen: Instant,
}

/// Discovery message types
#[derive(serde::Serialize, serde::Deserialize, Debug)]
enum DiscoveryMessage {
    Announce { node_id: NodeId, port: u16 },
    Data { sender: NodeId, frame: Frame },
}

/// Ethernet network adapter implementation
pub struct EthernetAdapter {
    /// Adapter ID
    id: String,

    /// Configuration
    config: EthernetConfig,

    /// Local node ID
    local_node_id: Arc<RwLock<Option<NodeId>>>,

    /// Unicast socket for receiving directed messages
    unicast_socket: Arc<UdpSocket>,

    /// Multicast socket for discovery
    multicast_socket: Option<Arc<UdpSocket>>,

    /// Known peers
    peers: Arc<RwLock<HashMap<NodeId, PeerInfo>>>,

    /// Statistics
    stats: Arc<Mutex<AdapterStats>>,

    /// Received frames queue
    incoming_frames: Arc<Mutex<Vec<(NodeId, Frame)>>>,

    /// Shutdown signal
    shutdown: Arc<Mutex<bool>>,
}

impl EthernetAdapter {
    /// Create a new Ethernet adapter
    pub async fn new(id: String, config: EthernetConfig) -> Result<Self> {
        // Create unicast socket
        let unicast_addr = SocketAddr::new(
            config.bind_interface.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            config.unicast_port,
        );
        let unicast_socket = UdpSocket::bind(unicast_addr)
            .await
            .map_err(|e| NetworkError::IoError(e))?;

        // Create multicast socket if discovery is enabled
        let multicast_socket = if config.discovery_enabled {
            let multicast_addr = SocketAddr::new(
                IpAddr::V4(config.multicast_addr),
                config.multicast_port,
            );

            // Create socket with SO_REUSEADDR
            let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
                .map_err(|e| NetworkError::IoError(e))?;
            socket.set_reuse_address(true)
                .map_err(|e| NetworkError::IoError(e))?;
            socket.set_nonblocking(true)
                .map_err(|e| NetworkError::IoError(e))?;
            socket.bind(&multicast_addr.into())
                .map_err(|e| NetworkError::IoError(e))?;

            // Join multicast group
            socket.join_multicast_v4(&config.multicast_addr, &Ipv4Addr::UNSPECIFIED)
                .map_err(|e| NetworkError::IoError(e))?;

            let std_socket: std::net::UdpSocket = socket.into();
            std_socket.set_nonblocking(true)
                .map_err(|e| NetworkError::IoError(e))?;
            let udp_socket = UdpSocket::from_std(std_socket)
                .map_err(|e| NetworkError::IoError(e))?;

            Some(Arc::new(udp_socket))
        } else {
            None
        };

        let adapter = Self {
            id,
            config,
            local_node_id: Arc::new(RwLock::new(None)),
            unicast_socket: Arc::new(unicast_socket),
            multicast_socket,
            peers: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(Mutex::new(AdapterStats::default())),
            incoming_frames: Arc::new(Mutex::new(Vec::new())),
            shutdown: Arc::new(Mutex::new(false)),
        };

        // Start background tasks
        adapter.start_background_tasks();

        Ok(adapter)
    }

    /// Set the local node ID
    pub async fn set_node_id(&self, node_id: NodeId) {
        *self.local_node_id.write().await = Some(node_id);
    }

    /// Start background tasks for discovery and receiving
    fn start_background_tasks(&self) {
        // Discovery announcement task
        if let Some(multicast_socket) = &self.multicast_socket {
            let socket = multicast_socket.clone();
            let config = self.config.clone();
            let node_id = self.local_node_id.clone();
            let shutdown = self.shutdown.clone();

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(DISCOVERY_INTERVAL);
                loop {
                    if *shutdown.lock().await {
                        break;
                    }

                    interval.tick().await;

                    if let Some(local_id) = *node_id.read().await {
                        let announce = DiscoveryMessage::Announce {
                            node_id: local_id,
                            port: config.unicast_port,
                        };

                        if let Ok(data) = serialize(&announce) {
                            let multicast_addr = SocketAddr::new(
                                IpAddr::V4(config.multicast_addr),
                                config.multicast_port,
                            );
                            let _ = socket.send_to(&data, multicast_addr).await;
                        }
                    }
                }
            });
        }

        // Multicast receive task
        if let Some(multicast_socket) = &self.multicast_socket {
            let socket = multicast_socket.clone();
            let peers = self.peers.clone();
            let local_node_id = self.local_node_id.clone();
            let shutdown = self.shutdown.clone();

            tokio::spawn(async move {
                let mut buf = vec![0u8; 65535];
                loop {
                    if *shutdown.lock().await {
                        break;
                    }

                    match tokio::time::timeout(Duration::from_secs(1), socket.recv_from(&mut buf)).await {
                        Ok(Ok((len, addr))) => {
                            if let Ok(msg) = deserialize::<DiscoveryMessage>(&buf[..len]) {
                                if let DiscoveryMessage::Announce { node_id, port } = msg {
                                    // Don't add ourselves
                                    if let Some(local_id) = *local_node_id.read().await {
                                        if node_id == local_id {
                                            continue;
                                        }
                                    }

                                    let peer_addr = SocketAddr::new(addr.ip(), port);
                                    let mut peers_lock = peers.write().await;
                                    peers_lock.insert(node_id, PeerInfo {
                                        node_id,
                                        socket_addr: peer_addr,
                                        last_seen: Instant::now(),
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            });
        }

        // Unicast receive task
        {
            let socket = self.unicast_socket.clone();
            let incoming_frames = self.incoming_frames.clone();
            let stats = self.stats.clone();
            let shutdown = self.shutdown.clone();

            tokio::spawn(async move {
                let mut buf = vec![0u8; 65535];
                loop {
                    if *shutdown.lock().await {
                        break;
                    }

                    match tokio::time::timeout(Duration::from_secs(1), socket.recv_from(&mut buf)).await {
                        Ok(Ok((len, _addr))) => {
                            if let Ok(msg) = deserialize::<DiscoveryMessage>(&buf[..len]) {
                                if let DiscoveryMessage::Data { sender, frame } = msg {
                                    incoming_frames.lock().await.push((sender, frame));

                                    let mut stats_lock = stats.lock().await;
                                    stats_lock.frames_received += 1;
                                    stats_lock.bytes_received += len as u64;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            });
        }

        // Peer cleanup task
        {
            let peers = self.peers.clone();
            let shutdown = self.shutdown.clone();

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(30));
                loop {
                    if *shutdown.lock().await {
                        break;
                    }

                    interval.tick().await;

                    let now = Instant::now();
                    let mut peers_lock = peers.write().await;
                    peers_lock.retain(|_, peer| now.duration_since(peer.last_seen) < PEER_TIMEOUT);
                }
            });
        }
    }

    /// Get known peers
    pub async fn get_peers(&self) -> Vec<(NodeId, SocketAddr)> {
        let peers = self.peers.read().await;
        peers.iter()
            .map(|(id, info)| (*id, info.socket_addr))
            .collect()
    }
}

#[async_trait]
impl NetworkAdapter for EthernetAdapter {
    fn info(&self) -> AdapterInfo {
        AdapterInfo {
            id: self.id.clone(),
            adapter_type: AdapterType::Ethernet,
            name: format!("Ethernet ({})", self.id),
            mtu: 1500,
            available: true,
            address: Some(format!("{}:{}",
                self.config.bind_interface.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
                self.config.unicast_port)),
        }
    }

    async fn send_to(&self, peer: &NodeId, frame: &Frame) -> Result<()> {
        let peers = self.peers.read().await;
        let peer_info = peers.get(peer)
            .ok_or_else(|| NetworkError::Other(format!("Peer not found: {}", peer)))?;

        let msg = DiscoveryMessage::Data {
            sender: self.local_node_id.read().await
                .ok_or_else(|| NetworkError::Other("Local node ID not set".to_string()))?,
            frame: frame.clone(),
        };

        let data = serialize(&msg)
            .map_err(|e| NetworkError::SendFailed(e.to_string()))?;

        self.unicast_socket.send_to(&data, peer_info.socket_addr).await
            .map_err(|e| NetworkError::IoError(e))?;

        let mut stats = self.stats.lock().await;
        stats.frames_sent += 1;
        stats.bytes_sent += data.len() as u64;

        Ok(())
    }

    async fn broadcast(&self, frame: &Frame) -> Result<()> {
        let peers = self.peers.read().await;
        let peer_addrs: Vec<SocketAddr> = peers.values()
            .map(|info| info.socket_addr)
            .collect();

        drop(peers);

        let local_id = self.local_node_id.read().await
            .ok_or_else(|| NetworkError::Other("Local node ID not set".to_string()))?;

        let msg = DiscoveryMessage::Data {
            sender: local_id,
            frame: frame.clone(),
        };

        let data = serialize(&msg)
            .map_err(|e| NetworkError::SendFailed(e.to_string()))?;

        let mut success_count = 0;
        for addr in peer_addrs {
            if self.unicast_socket.send_to(&data, addr).await.is_ok() {
                success_count += 1;
            }
        }

        let mut stats = self.stats.lock().await;
        stats.frames_sent += success_count;
        stats.bytes_sent += (data.len() as u64) * success_count;

        if success_count > 0 {
            Ok(())
        } else {
            Err(NetworkError::SendFailed("No peers available".to_string()))
        }
    }

    async fn receive(&self) -> Result<(NodeId, Frame)> {
        loop {
            // Check shutdown
            if *self.shutdown.lock().await {
                return Err(NetworkError::Other("Adapter shutdown".to_string()));
            }

            // Check for incoming frames
            {
                let mut frames = self.incoming_frames.lock().await;
                if let Some((sender, frame)) = frames.pop() {
                    return Ok((sender, frame));
                }
            }

            // Wait a bit before checking again
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn is_available(&self) -> bool {
        !*self.shutdown.lock().await
    }

    async fn stats(&self) -> AdapterStats {
        self.stats.lock().await.clone()
    }

    async fn shutdown(&self) -> Result<()> {
        *self.shutdown.lock().await = true;
        Ok(())
    }
}

impl std::fmt::Debug for EthernetAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthernetAdapter")
            .field("id", &self.id)
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = EthernetConfig {
            unicast_port: 0, // Use random port for testing
            ..Default::default()
        };

        let adapter = EthernetAdapter::new("test".to_string(), config).await;
        assert!(adapter.is_ok());
    }

    #[tokio::test]
    async fn test_set_node_id() {
        let config = EthernetConfig {
            unicast_port: 0,
            discovery_enabled: false,
            ..Default::default()
        };

        let adapter = EthernetAdapter::new("test".to_string(), config).await.unwrap();
        let node_id = NodeId::from_bytes([1; 32]);

        adapter.set_node_id(node_id).await;

        let stored_id = adapter.local_node_id.read().await;
        assert_eq!(*stored_id, Some(node_id));
    }

    #[tokio::test]
    async fn test_adapter_info() {
        let config = EthernetConfig {
            unicast_port: 0,
            discovery_enabled: false,
            ..Default::default()
        };

        let adapter = EthernetAdapter::new("test".to_string(), config).await.unwrap();
        let info = adapter.info();

        assert_eq!(info.id, "test");
        assert_eq!(info.adapter_type, AdapterType::Ethernet);
        assert!(info.available);
    }

    #[tokio::test]
    async fn test_is_available() {
        let config = EthernetConfig {
            unicast_port: 0,
            discovery_enabled: false,
            ..Default::default()
        };

        let adapter = EthernetAdapter::new("test".to_string(), config).await.unwrap();
        assert!(adapter.is_available().await);

        adapter.shutdown().await.unwrap();
        assert!(!adapter.is_available().await);
    }
}
