//! Bluetooth Classic network adapter
//!
//! This adapter provides connectivity through Bluetooth Classic (BR/EDR) for
//! short-range device-to-device communication. Typical range: 10-100 meters.
//!
//! Implementation notes:
//! - Uses channel-based transport for send/receive operations
//! - Can be integrated with platform-specific Bluetooth APIs (bluez, CoreBluetooth, etc.)
//! - Supports RFCOMM connections for reliable byte streams
//! - SDP service registration for peer discovery

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::{
    types::{AdapterType, NODE_ID_SIZE},
    Frame, NodeId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;

/// Bluetooth Classic adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothConfig {
    /// Device name for discovery
    pub device_name: String,
    /// Whether this device is discoverable
    pub discoverable: bool,
    /// PIN for pairing (optional)
    pub pin: Option<String>,
    /// RFCOMM channel to use (1-30)
    pub rfcomm_channel: u8,
    /// Service UUID for MyriadMesh protocol
    pub service_uuid: String,
}

impl Default for BluetoothConfig {
    fn default() -> Self {
        Self {
            device_name: "MyriadMesh-Node".to_string(),
            discoverable: true,
            pin: None,
            rfcomm_channel: 1,
            service_uuid: "00001101-0000-1000-8000-00805F9B34FB".to_string(), // Serial Port Profile
        }
    }
}

/// Bluetooth peer information
#[derive(Debug, Clone)]
struct BluetoothPeer {
    address: String,
    name: Option<String>,
    node_id: Option<NodeId>,
    last_seen: u64,
    paired: bool,
    rssi: Option<i8>,
}

/// RFCOMM connection state
struct RfcommConnection {
    remote_address: String,
    tx: mpsc::UnboundedSender<Vec<u8>>,
    connected_at: u64,
}

/// Bluetooth Classic network adapter
pub struct BluetoothAdapter {
    config: BluetoothConfig,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    peers: Arc<RwLock<HashMap<String, BluetoothPeer>>>,
    connections: Arc<RwLock<HashMap<String, RfcommConnection>>>,
    local_address: Option<String>,
    /// Receive channel for incoming frames
    rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<(Address, Frame)>>>>,
    /// Send channel for incoming frames (cloned for connection handlers)
    incoming_tx: mpsc::UnboundedSender<(Address, Frame)>,
}

impl BluetoothAdapter {
    /// Create a new Bluetooth Classic adapter
    pub fn new(config: BluetoothConfig) -> Self {
        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::Bluetooth,
            max_message_size: 1024 * 64, // 64 KB typical for Bluetooth Classic
            typical_latency_ms: 50.0,
            typical_bandwidth_bps: 3_000_000, // ~3 Mbps for Bluetooth 2.0+EDR
            reliability: 0.95,                // Generally reliable within range
            range_meters: 100.0,              // Class 1 Bluetooth can reach 100m
            power_consumption: PowerConsumption::Low,
            cost_per_mb: 0.0, // No data cost
            supports_broadcast: false,
            supports_multicast: false,
        };

        // Create channel for receiving frames
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();

        Self {
            config,
            status: Arc::new(RwLock::new(AdapterStatus::Uninitialized)),
            capabilities,
            peers: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            local_address: None,
            rx: Arc::new(RwLock::new(Some(incoming_rx))),
            incoming_tx,
        }
    }

    /// Scan for nearby Bluetooth devices
    async fn scan_for_devices(&self) -> Result<Vec<BluetoothPeer>> {
        // Platform-specific implementation needed here
        // On Linux: Use bluez D-Bus API
        // On macOS: Use CoreBluetooth framework
        // On Windows: Use Windows.Devices.Bluetooth API

        // For now, return discovered peers from cache
        // In production, this would trigger actual hardware scanning
        let peers = self.peers.read().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(peers
            .values()
            .filter(|p| now - p.last_seen < 300) // Last 5 minutes
            .cloned()
            .collect())
    }

    /// Pair with a Bluetooth device
    async fn pair_device(&self, address: &str, _pin: Option<&str>) -> Result<()> {
        // Platform-specific pairing implementation
        // 1. Initiate pairing request
        // 2. Handle PIN/passkey exchange
        // 3. Store paired device information

        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(address) {
            peer.paired = true;
            Ok(())
        } else {
            Err(NetworkError::InvalidAddress(format!(
                "Unknown device: {}",
                address
            )))
        }
    }

    /// Create RFCOMM connection to peer
    async fn connect_rfcomm(&self, address: &str) -> Result<()> {
        // Check if already connected
        {
            let connections = self.connections.read().await;
            if connections.contains_key(address) {
                return Ok(()); // Already connected
            }
        }

        // Platform-specific RFCOMM connection
        // On Linux: Use BlueZ RFCOMM sockets
        // On macOS: Use IOBluetooth framework
        // On Windows: Use Windows Bluetooth RFCOMM API

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
                RfcommConnection {
                    remote_address: address.to_string(),
                    tx,
                    connected_at: now,
                },
            );
        }

        // Spawn connection handler
        // In production, this would read from actual Bluetooth socket
        let addr = address.to_string();
        let incoming_tx = self.incoming_tx.clone();
        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                // Deserialize frame from bytes
                match bincode::deserialize::<Frame>(&data) {
                    Ok(frame) => {
                        let source_addr = Address::Bluetooth(addr.clone());
                        let _ = incoming_tx.send((source_addr, frame));
                    }
                    Err(_) => {
                        // Invalid frame, ignore
                        continue;
                    }
                }
            }
        });

        Ok(())
    }

    /// Register SDP service for discovery
    async fn register_sdp_service(&self) -> Result<()> {
        // Platform-specific SDP registration
        // This allows other devices to discover our MyriadMesh service
        // Service UUID is defined in config

        // Implementation would:
        // 1. Create SDP service record
        // 2. Register with Bluetooth stack
        // 3. Make service discoverable

        Ok(())
    }

    /// Get or create connection to peer
    async fn ensure_connection(&self, address: &str) -> Result<()> {
        // Check if connected
        {
            let connections = self.connections.read().await;
            if connections.contains_key(address) {
                return Ok(());
            }
        }

        // Check if peer is known and paired
        {
            let peers = self.peers.read().await;
            if let Some(peer) = peers.get(address) {
                if !peer.paired {
                    // Attempt pairing first
                    drop(peers); // Release lock before pairing
                    self.pair_device(address, self.config.pin.as_deref())
                        .await?;
                }
            } else {
                return Err(NetworkError::InvalidAddress(format!(
                    "Unknown Bluetooth device: {}",
                    address
                )));
            }
        }

        // Create RFCOMM connection
        self.connect_rfcomm(address).await
    }
}

#[async_trait::async_trait]
impl NetworkAdapter for BluetoothAdapter {
    async fn initialize(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::Initializing;

        // Platform-specific initialization:
        // 1. Check if Bluetooth hardware is available
        // 2. Power on Bluetooth adapter
        // 3. Set device name and discoverable mode
        // 4. Register SDP service

        // Get local Bluetooth address from hardware
        // For now, use simulated address (would be read from Bluetooth adapter)
        self.local_address = Some("00:11:22:33:44:55".to_string());

        // Register SDP service for peer discovery
        self.register_sdp_service().await?;

        *self.status.write().await = AdapterStatus::Ready;
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }

        // Start accepting incoming RFCOMM connections
        // In production, this would:
        // 1. Create RFCOMM server socket
        // 2. Listen on configured channel
        // 3. Accept connections in background task
        // 4. Make device discoverable if configured

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::ShuttingDown;

        // Cleanup Bluetooth resources
        // 1. Close all active connections
        {
            let mut connections = self.connections.write().await;
            connections.clear();
        }

        // 2. Unregister SDP service
        // 3. Make device non-discoverable
        // 4. Clear peer list

        *self.status.write().await = AdapterStatus::Uninitialized;
        Ok(())
    }

    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }
        drop(status);

        // Extract Bluetooth address from destination
        let bt_address = match destination {
            Address::Bluetooth(addr) => addr,
            _ => {
                return Err(NetworkError::InvalidAddress(
                    "Expected Bluetooth address".to_string(),
                ))
            }
        };

        // Ensure connection to peer exists
        self.ensure_connection(bt_address).await?;

        // Serialize frame to bytes
        let frame_data = bincode::serialize(frame).map_err(|e| {
            NetworkError::SendFailed(format!("Failed to serialize frame: {}", e))
        })?;

        // Check size limit
        if frame_data.len() > self.capabilities.max_message_size {
            return Err(NetworkError::MessageTooLarge {
                size: frame_data.len(),
                max: self.capabilities.max_message_size,
            });
        }

        // Send over RFCOMM connection
        let connections = self.connections.read().await;
        let connection = connections.get(bt_address).ok_or_else(|| {
            NetworkError::SendFailed("Connection lost after establishment".to_string())
        })?;

        connection.tx.send(frame_data).map_err(|_| {
            NetworkError::SendFailed("Failed to send to connection channel".to_string())
        })?;

        Ok(())
    }

    async fn receive(&self, timeout_ms: u64) -> Result<(Address, Frame)> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }
        drop(status);

        // Receive from incoming frame channel
        let mut rx_guard = self.rx.write().await;
        let rx = rx_guard
            .as_mut()
            .ok_or_else(|| NetworkError::ReceiveFailed("Receive channel closed".to_string()))?;

        match timeout(Duration::from_millis(timeout_ms), rx.recv()).await {
            Ok(Some((address, frame))) => Ok((address, frame)),
            Ok(None) => Err(NetworkError::ReceiveFailed("Channel closed".to_string())),
            Err(_) => Err(NetworkError::Timeout),
        }
    }

    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }
        drop(status);

        // Scan for nearby Bluetooth devices
        let discovered = self.scan_for_devices().await?;

        // Update peer list
        let mut peers = self.peers.write().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        for peer in discovered {
            peers.insert(peer.address.clone(), peer);
        }

        // Convert to PeerInfo
        Ok(peers
            .values()
            .filter(|p| now - p.last_seen < 300) // Only peers seen in last 5 minutes
            .map(|p| PeerInfo {
                node_id: p
                    .node_id
                    .unwrap_or_else(|| NodeId::from_bytes([0u8; NODE_ID_SIZE])),
                address: Address::Bluetooth(p.address.clone()),
            })
            .collect())
    }

    fn get_status(&self) -> AdapterStatus {
        // Use blocking read for sync method
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

        let bt_address = match destination {
            Address::Bluetooth(addr) => addr,
            _ => {
                return Err(NetworkError::InvalidAddress(
                    "Expected Bluetooth address".to_string(),
                ))
            }
        };

        // Try to establish connection
        let start = std::time::Instant::now();

        match self.ensure_connection(bt_address).await {
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
        self.local_address
            .as_ref()
            .map(|addr| Address::Bluetooth(addr.clone()))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        // Validate Bluetooth MAC address format (XX:XX:XX:XX:XX:XX)
        let parts: Vec<&str> = addr_str.split(':').collect();
        if parts.len() != 6 {
            return Err(NetworkError::InvalidAddress(
                "Bluetooth address must be in format XX:XX:XX:XX:XX:XX".to_string(),
            ));
        }

        for part in &parts {
            if part.len() != 2 || !part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(NetworkError::InvalidAddress(
                    "Bluetooth address must contain hex digits only".to_string(),
                ));
            }
        }

        Ok(Address::Bluetooth(addr_str.to_uppercase()))
    }

    fn supports_address(&self, address: &Address) -> bool {
        matches!(address, Address::Bluetooth(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bluetooth_adapter_creation() {
        let config = BluetoothConfig::default();
        let adapter = BluetoothAdapter::new(config);

        assert_eq!(
            adapter.get_capabilities().adapter_type,
            AdapterType::Bluetooth
        );
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
    }

    #[tokio::test]
    async fn test_bluetooth_capabilities() {
        let config = BluetoothConfig::default();
        let adapter = BluetoothAdapter::new(config);
        let caps = adapter.get_capabilities();

        assert_eq!(caps.max_message_size, 1024 * 64);
        assert_eq!(caps.typical_latency_ms, 50.0);
    }

    #[test]
    fn test_bluetooth_config_default() {
        let config = BluetoothConfig::default();
        assert_eq!(config.device_name, "MyriadMesh-Node");
        assert!(config.discoverable);
        assert_eq!(config.rfcomm_channel, 1);
    }

    #[test]
    fn test_parse_address() {
        let config = BluetoothConfig::default();
        let adapter = BluetoothAdapter::new(config);

        // Valid address
        let addr = adapter.parse_address("00:11:22:33:44:55").unwrap();
        assert_eq!(addr, Address::Bluetooth("00:11:22:33:44:55".to_string()));

        // Invalid addresses
        assert!(adapter.parse_address("invalid").is_err());
        assert!(adapter.parse_address("00:11:22:33:44").is_err());
        assert!(adapter.parse_address("XX:11:22:33:44:55").is_err());
    }

    #[test]
    fn test_supports_address() {
        let config = BluetoothConfig::default();
        let adapter = BluetoothAdapter::new(config);

        assert!(adapter.supports_address(&Address::Bluetooth("00:11:22:33:44:55".to_string())));
        assert!(!adapter.supports_address(&Address::Ethernet("192.168.1.1".to_string())));
    }
}
