//! I2P Network Adapter
//!
//! Provides zero-configuration i2p networking with automatic router management,
//! destination persistence, and NetworkAdapter trait implementation.

use super::embedded_router::{I2pRouterConfig, I2pRouterMode};
use super::sam_client::{SamSession, SessionStyle};
use crate::{
    adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults},
    error::{NetworkError, Result},
    types::{AdapterCapabilities, Address, PowerConsumption},
};
use async_trait::async_trait;
use myriadmesh_protocol::Frame;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// I2P network adapter with automatic router management
pub struct I2pAdapter {
    /// I2P router (system or embedded)
    router: Arc<RwLock<Option<I2pRouterMode>>>,

    /// SAM session for connections
    session: Arc<RwLock<Option<SamSession>>>,

    /// Our i2p destination
    destination: Arc<RwLock<Option<String>>>,

    /// Active connections to remote destinations
    connections: Arc<RwLock<HashMap<String, TcpStream>>>,

    /// Path to persist destination keys
    keys_path: PathBuf,

    /// Session ID for SAM
    session_id: String,

    /// Router configuration
    router_config: I2pRouterConfig,

    /// Adapter status
    status: Arc<RwLock<AdapterStatus>>,

    /// Adapter capabilities
    capabilities: AdapterCapabilities,
}

impl I2pAdapter {
    /// Create a new i2p adapter with automatic configuration
    pub fn new() -> Self {
        Self::with_config(I2pRouterConfig::default())
    }

    /// Create i2p adapter with custom configuration
    pub fn with_config(config: I2pRouterConfig) -> Self {
        let keys_path = config.data_dir.join("destination.keys");

        let capabilities = AdapterCapabilities {
            adapter_type: myriadmesh_protocol::types::AdapterType::I2P,
            max_message_size: 32768, // i2p supports large messages
            typical_latency_ms: 5000.0, // i2p has high latency
            typical_bandwidth_bps: 1_024_000, // ~1 Mbps
            reliability: 0.95, // i2p is quite reliable
            range_meters: 0.0, // global reach
            power_consumption: PowerConsumption::Medium,
            cost_per_mb: 0.0, // free
            supports_broadcast: false,
            supports_multicast: false,
        };

        I2pAdapter {
            router: Arc::new(RwLock::new(None)),
            session: Arc::new(RwLock::new(None)),
            destination: Arc::new(RwLock::new(None)),
            connections: Arc::new(RwLock::new(HashMap::new())),
            keys_path,
            session_id: format!("myriadmesh_{}", uuid::Uuid::new_v4()),
            router_config: config,
            status: Arc::new(RwLock::new(AdapterStatus::Uninitialized)),
            capabilities,
        }
    }

    /// Get SAM address for this adapter
    fn sam_address(&self) -> String {
        format!("127.0.0.1:{}", self.router_config.sam_port)
    }

    /// Load or generate i2p destination
    async fn ensure_destination(&self) -> Result<String> {
        // Check if we already have a destination
        {
            let dest = self.destination.read().await;
            if let Some(d) = dest.as_ref() {
                return Ok(d.clone());
            }
        }

        // Try to load from disk
        if self.keys_path.exists() {
            match fs::read_to_string(&self.keys_path) {
                Ok(dest) => {
                    let mut destination = self.destination.write().await;
                    *destination = Some(dest.clone());
                    log::info!("Loaded i2p destination from {}", self.keys_path.display());
                    return Ok(dest);
                }
                Err(e) => {
                    log::warn!("Failed to load destination keys: {}", e);
                }
            }
        }

        // Generate new destination
        let sam_addr = self.sam_address();
        let mut conn = super::sam_client::SamConnection::connect(&sam_addr)
            .map_err(|e| NetworkError::InitializationFailed(format!("SAM connect failed: {}", e)))?;

        let dest = conn
            .generate_destination()
            .map_err(|e| NetworkError::InitializationFailed(format!("Dest generation failed: {}", e)))?;

        // Save to disk
        if let Some(parent) = self.keys_path.parent() {
            fs::create_dir_all(parent).ok();
        }

        fs::write(&self.keys_path, &dest.destination).map_err(|e| {
            NetworkError::InitializationFailed(format!("Failed to save destination: {}", e))
        })?;

        log::info!(
            "Generated new i2p destination, saved to {}",
            self.keys_path.display()
        );

        let mut destination = self.destination.write().await;
        *destination = Some(dest.destination.clone());

        Ok(dest.destination)
    }

    /// Initialize SAM session
    async fn ensure_session(&self) -> Result<()> {
        // Check if session already exists
        {
            let session = self.session.read().await;
            if session.is_some() {
                return Ok(());
            }
        }

        // Get destination
        let destination = self.ensure_destination().await?;

        // Create SAM session
        let sam_addr = self.sam_address();
        let session = SamSession::create(
            &sam_addr,
            self.session_id.clone(),
            SessionStyle::Stream,
            Some(destination.clone()),
        )
        .map_err(|e| NetworkError::InitializationFailed(format!("SAM session failed: {}", e)))?;

        let mut session_lock = self.session.write().await;
        *session_lock = Some(session);

        log::info!("I2P SAM session established with ID: {}", self.session_id);

        Ok(())
    }

    /// Get our i2p destination address
    pub async fn get_destination(&self) -> Result<String> {
        self.ensure_destination().await
    }

    /// Get or create connection to destination
    async fn get_connection(&self, destination: &str) -> Result<TcpStream> {
        // Check if we have an existing connection
        {
            let connections = self.connections.read().await;
            if let Some(conn) = connections.get(destination) {
                // Try to clone the connection
                if let Ok(cloned) = conn.try_clone() {
                    return Ok(cloned);
                }
            }
        }

        // Need to create new connection
        let mut session = self.session.write().await;
        let session = session.as_mut().ok_or_else(|| {
            NetworkError::InitializationFailed("No SAM session".to_string())
        })?;

        let stream = session
            .connect(destination)
            .map_err(|e| NetworkError::SendFailed(format!("I2P connect failed: {}", e)))?;

        // Store connection
        let cloned = stream.try_clone()?;
        let mut connections = self.connections.write().await;
        connections.insert(destination.to_string(), stream);

        Ok(cloned)
    }

    /// Send frame over i2p stream
    async fn send_frame(&self, destination: &str, frame: &Frame) -> Result<()> {
        let mut stream = self.get_connection(destination).await?;

        // Serialize frame
        let frame_bytes = bincode::serialize(frame)
            .map_err(|e| NetworkError::SendFailed(format!("Frame serialization failed: {}", e)))?;

        // Send length prefix
        let len = frame_bytes.len() as u32;
        stream
            .write_all(&len.to_be_bytes())
            .map_err(|e| NetworkError::SendFailed(format!("Write failed: {}", e)))?;

        // Send frame data
        stream
            .write_all(&frame_bytes)
            .map_err(|e| NetworkError::SendFailed(format!("Write failed: {}", e)))?;

        stream.flush()?;

        Ok(())
    }

    /// Receive frame from i2p stream (with timeout)
    async fn receive_frame(&self, timeout_ms: u64) -> Result<(String, Frame)> {
        // For now, we'll accept a new connection and receive from it
        // In a production system, you'd want to manage multiple connections

        let mut session = self.session.write().await;
        let session = session.as_mut().ok_or_else(|| {
            NetworkError::ReceiveFailed("No SAM session".to_string())
        })?;

        // Set timeout on accept
        let (mut stream, remote_dest) = session
            .accept()
            .map_err(|e| NetworkError::ReceiveFailed(format!("Accept failed: {}", e)))?;

        stream.set_read_timeout(Some(Duration::from_millis(timeout_ms)))?;

        // Read length prefix
        let mut len_bytes = [0u8; 4];
        stream
            .read_exact(&mut len_bytes)
            .map_err(|e| NetworkError::ReceiveFailed(format!("Read length failed: {}", e)))?;

        let len = u32::from_be_bytes(len_bytes) as usize;

        // Read frame data
        let mut frame_bytes = vec![0u8; len];
        stream
            .read_exact(&mut frame_bytes)
            .map_err(|e| NetworkError::ReceiveFailed(format!("Read frame failed: {}", e)))?;

        // Deserialize frame
        let frame: Frame = bincode::deserialize(&frame_bytes)
            .map_err(|e| NetworkError::ReceiveFailed(format!("Frame deserialization failed: {}", e)))?;

        Ok((remote_dest.destination, frame))
    }
}

impl Default for I2pAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NetworkAdapter for I2pAdapter {
    async fn initialize(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::Initializing;

        // Initialize i2p router
        let router = I2pRouterMode::initialize(self.router_config.clone())
            .await
            .map_err(|e| NetworkError::InitializationFailed(e.to_string()))?;

        *self.router.write().await = Some(router);

        // Initialize session
        self.ensure_session().await?;

        *self.status.write().await = AdapterStatus::Ready;

        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        // For i2p, initialization does the work
        if *self.status.read().await == AdapterStatus::Uninitialized {
            self.initialize().await?;
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::ShuttingDown;

        // Close all connections
        self.connections.write().await.clear();

        // Clear session
        *self.session.write().await = None;

        *self.status.write().await = AdapterStatus::Uninitialized;

        Ok(())
    }

    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()> {
        // Extract i2p destination from address
        let dest_str = match destination {
            Address::I2P(dest) => dest,
            _ => {
                return Err(NetworkError::InvalidAddress(
                    "Expected i2p address".to_string(),
                ))
            }
        };

        self.send_frame(dest_str, frame).await
    }

    async fn receive(&self, timeout_ms: u64) -> Result<(Address, Frame)> {
        let (dest, frame) = self.receive_frame(timeout_ms).await?;
        Ok((Address::I2P(dest), frame))
    }

    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        // I2P doesn't support peer discovery in the traditional sense
        // You need to know the destination in advance
        Ok(Vec::new())
    }

    fn get_status(&self) -> AdapterStatus {
        // Need to use blocking read since this is sync
        *futures::executor::block_on(self.status.read())
    }

    fn get_capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    async fn test_connection(&self, destination: &Address) -> Result<TestResults> {
        let dest_str = match destination {
            Address::I2P(dest) => dest,
            _ => {
                return Ok(TestResults {
                    success: false,
                    rtt_ms: None,
                    error: Some("Not an i2p address".to_string()),
                })
            }
        };

        let start = Instant::now();

        // Try to connect
        match self.get_connection(dest_str).await {
            Ok(_) => {
                let elapsed = start.elapsed();
                Ok(TestResults {
                    success: true,
                    rtt_ms: Some(elapsed.as_secs_f64() * 1000.0),
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
        // Need to use blocking read since this is sync
        let dest = futures::executor::block_on(self.destination.read());
        dest.as_ref().map(|d| Address::I2P(d.clone()))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        // I2P addresses are base64-encoded destinations or .i2p domains
        if addr_str.ends_with(".i2p") || addr_str.contains('~') {
            Ok(Address::I2P(addr_str.to_string()))
        } else {
            Err(NetworkError::InvalidAddress(format!(
                "Invalid i2p address: {}",
                addr_str
            )))
        }
    }

    fn supports_address(&self, address: &Address) -> bool {
        matches!(address, Address::I2P(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = I2pAdapter::new();
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
        assert_eq!(adapter.get_capabilities().range_meters, 0.0); // Global reach
        assert!(adapter.get_capabilities().reliability > 0.9); // High reliability
    }

    #[test]
    fn test_address_parsing() {
        let adapter = I2pAdapter::new();

        let addr1 = adapter.parse_address("example.i2p");
        assert!(addr1.is_ok());

        let addr2 = adapter.parse_address("abc123~xyz.i2p");
        assert!(addr2.is_ok());

        let addr3 = adapter.parse_address("invalid");
        assert!(addr3.is_err());
    }

    #[test]
    fn test_address_support() {
        let adapter = I2pAdapter::new();

        assert!(adapter.supports_address(&Address::I2P("test.i2p".to_string())));
        assert!(!adapter.supports_address(&Address::Ethernet("127.0.0.1:4001".to_string())));
    }

    #[tokio::test]
    #[ignore] // Requires i2p router
    async fn test_initialization() {
        let mut adapter = I2pAdapter::new();
        let result = adapter.initialize().await;
        assert!(result.is_ok());
        assert_eq!(adapter.get_status(), AdapterStatus::Ready);
    }

    #[tokio::test]
    #[ignore] // Requires i2p router
    async fn test_destination_persistence() {
        let config = I2pRouterConfig::default();
        let adapter1 = I2pAdapter::with_config(config.clone());

        let dest1 = adapter1.get_destination().await.unwrap();
        assert!(!dest1.is_empty());

        // Create new adapter with same config
        let adapter2 = I2pAdapter::with_config(config);
        let dest2 = adapter2.get_destination().await.unwrap();

        // Should have same destination
        assert_eq!(dest1, dest2);
    }
}
