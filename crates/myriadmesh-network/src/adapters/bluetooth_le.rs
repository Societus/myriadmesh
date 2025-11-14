//! Bluetooth Low Energy (BLE) network adapter
//!
//! This adapter provides connectivity through BLE for energy-efficient
//! short-range communication. Optimized for low power consumption and
//! periodic data transmission.
//!
//! Implementation notes:
//! - Uses GATT characteristics for data transfer
//! - Supports BLE advertising for discovery
//! - Channel-based transport for send/receive
//! - Can be integrated with platform BLE stacks (BlueZ, CoreBluetooth, WinRT)

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

/// BLE adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BleConfig {
    /// Device name for advertising
    pub device_name: String,
    /// Whether to advertise our service
    pub advertising: bool,
    /// Advertising interval in milliseconds
    pub advertising_interval_ms: u32,
    /// Service UUID for MyriadMesh protocol
    pub service_uuid: String,
    /// Characteristic UUID for data transfer
    pub characteristic_uuid: String,
    /// Connection interval in milliseconds
    pub connection_interval_ms: u32,
}

impl Default for BleConfig {
    fn default() -> Self {
        Self {
            device_name: "MyriadMesh-BLE".to_string(),
            advertising: true,
            advertising_interval_ms: 1000,
            service_uuid: "6E400001-B5A3-F393-E0A9-E50E24DCCA9E".to_string(),
            characteristic_uuid: "6E400002-B5A3-F393-E0A9-E50E24DCCA9E".to_string(),
            connection_interval_ms: 50,
        }
    }
}

/// BLE peer information
#[derive(Debug, Clone)]
struct BlePeer {
    address: String,
    name: Option<String>,
    node_id: Option<NodeId>,
    rssi: i8,
    last_seen: u64,
    connected: bool,
}

/// GATT connection state
struct GattConnection {
    remote_address: String,
    tx: mpsc::UnboundedSender<Vec<u8>>,
    connected_at: u64,
    mtu: usize,
}

/// Bluetooth Low Energy network adapter
pub struct BleAdapter {
    config: BleConfig,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    peers: Arc<RwLock<HashMap<String, BlePeer>>>,
    connections: Arc<RwLock<HashMap<String, GattConnection>>>,
    local_address: Option<String>,
    /// Receive channel for incoming frames
    rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<(Address, Frame)>>>>,
    /// Send channel for incoming frames
    incoming_tx: mpsc::UnboundedSender<(Address, Frame)>,
    /// Advertising state
    advertising: Arc<RwLock<bool>>,
}

impl BleAdapter {
    /// Create a new BLE adapter
    pub fn new(config: BleConfig) -> Self {
        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::BluetoothLE,
            max_message_size: 512, // Typical BLE MTU allows ~512 bytes
            typical_latency_ms: 100.0,
            typical_bandwidth_bps: 1_000_000, // ~1 Mbps for BLE 4.2+
            reliability: 0.90,
            range_meters: 50.0, // BLE 5.0 can reach up to 200m
            power_consumption: PowerConsumption::VeryLow,
            cost_per_mb: 0.0,
            supports_broadcast: true, // BLE supports advertising broadcasts
            supports_multicast: false,
        };

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
            advertising: Arc::new(RwLock::new(false)),
        }
    }

    /// Start BLE advertising
    async fn start_advertising(&self) -> Result<()> {
        // Platform-specific BLE advertising
        // On Linux: Use BlueZ D-Bus LE Advertising API
        // On macOS: Use CoreBluetooth CBPeripheralManager
        // On Windows: Use Windows.Devices.Bluetooth.Advertisement

        // Advertising payload would include:
        // - Service UUID (MyriadMesh service)
        // - Device name
        // - Flags (LE General Discoverable, BR/EDR not supported)

        *self.advertising.write().await = true;
        Ok(())
    }

    /// Stop BLE advertising
    async fn stop_advertising(&self) -> Result<()> {
        *self.advertising.write().await = false;
        Ok(())
    }

    /// Scan for BLE devices
    async fn scan_for_devices(&self) -> Result<Vec<BlePeer>> {
        // Platform-specific BLE scanning
        // Scans for advertising devices and filters by service UUID

        // Return cached peers for now
        let peers = self.peers.read().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(peers
            .values()
            .filter(|p| now - p.last_seen < 60) // BLE peers timeout faster
            .cloned()
            .collect())
    }

    /// Connect to BLE peripheral
    async fn connect_gatt(&self, address: &str) -> Result<()> {
        // Check if already connected
        {
            let connections = self.connections.read().await;
            if connections.contains_key(address) {
                return Ok(());
            }
        }

        // Platform-specific GATT connection
        // 1. Connect to peripheral
        // 2. Discover services and characteristics
        // 3. Subscribe to notification characteristic
        // 4. Enable notifications

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
                GattConnection {
                    remote_address: address.to_string(),
                    tx,
                    connected_at: now,
                    mtu: 247, // BLE 4.2 MTU
                },
            );
        }

        // Spawn GATT connection handler
        let addr = address.to_string();
        let incoming_tx = self.incoming_tx.clone();
        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                match bincode::deserialize::<Frame>(&data) {
                    Ok(frame) => {
                        let source_addr = Address::BluetoothLE(addr.clone());
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

        // Connect to GATT peripheral
        self.connect_gatt(address).await
    }
}

#[async_trait::async_trait]
impl NetworkAdapter for BleAdapter {
    async fn initialize(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::Initializing;

        // Platform-specific BLE initialization
        // 1. Check if BLE hardware is available
        // 2. Power on BLE adapter
        // 3. Get local BLE address from hardware

        self.local_address = Some("AA:BB:CC:DD:EE:FF".to_string());

        if self.config.advertising {
            self.start_advertising().await?;
        }

        *self.status.write().await = AdapterStatus::Ready;
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }
        drop(status);

        if self.config.advertising {
            self.start_advertising().await?;
        }

        // Start GATT server to accept incoming connections
        // Platform-specific implementation would:
        // 1. Create GATT server
        // 2. Add MyriadMesh service
        // 3. Add characteristics for TX/RX
        // 4. Start server

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::ShuttingDown;

        // Stop advertising
        self.stop_advertising().await?;

        // Close all connections
        {
            let mut connections = self.connections.write().await;
            connections.clear();
        }

        *self.status.write().await = AdapterStatus::Uninitialized;
        Ok(())
    }

    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }
        drop(status);

        let ble_address = match destination {
            Address::BluetoothLE(addr) => addr,
            _ => {
                return Err(NetworkError::InvalidAddress(
                    "Expected Bluetooth LE address".to_string(),
                ))
            }
        };

        // Ensure connection exists
        self.ensure_connection(ble_address).await?;

        // Serialize frame
        let frame_data = bincode::serialize(frame).map_err(|e| {
            NetworkError::SendFailed(format!("Failed to serialize frame: {}", e))
        })?;

        // Check size against BLE MTU
        let connections = self.connections.read().await;
        let connection = connections.get(ble_address).ok_or_else(|| {
            NetworkError::SendFailed("Connection lost after establishment".to_string())
        })?;

        if frame_data.len() > connection.mtu {
            // Fragment if needed (or return error for simplicity)
            return Err(NetworkError::MessageTooLarge {
                size: frame_data.len(),
                max: connection.mtu,
            });
        }

        // Send via GATT characteristic
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

        // Scan for BLE devices
        let discovered = self.scan_for_devices().await?;

        // Update peer cache
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
            .filter(|p| now - p.last_seen < 60)
            .map(|p| PeerInfo {
                node_id: p
                    .node_id
                    .unwrap_or_else(|| NodeId::from_bytes([0u8; NODE_ID_SIZE])),
                address: Address::BluetoothLE(p.address.clone()),
            })
            .collect())
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

        let ble_address = match destination {
            Address::BluetoothLE(addr) => addr,
            _ => {
                return Err(NetworkError::InvalidAddress(
                    "Expected Bluetooth LE address".to_string(),
                ))
            }
        };

        // Try to establish connection
        let start = std::time::Instant::now();

        match self.ensure_connection(ble_address).await {
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
            .map(|addr| Address::BluetoothLE(addr.clone()))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        let parts: Vec<&str> = addr_str.split(':').collect();
        if parts.len() != 6 {
            return Err(NetworkError::InvalidAddress(
                "BLE address must be in format XX:XX:XX:XX:XX:XX".to_string(),
            ));
        }

        for part in &parts {
            if part.len() != 2 || !part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(NetworkError::InvalidAddress(
                    "BLE address must contain hex digits only".to_string(),
                ));
            }
        }

        Ok(Address::BluetoothLE(addr_str.to_uppercase()))
    }

    fn supports_address(&self, address: &Address) -> bool {
        matches!(address, Address::BluetoothLE(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ble_adapter_creation() {
        let config = BleConfig::default();
        let adapter = BleAdapter::new(config);

        assert_eq!(
            adapter.get_capabilities().adapter_type,
            AdapterType::BluetoothLE
        );
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
    }

    #[tokio::test]
    async fn test_ble_capabilities() {
        let config = BleConfig::default();
        let adapter = BleAdapter::new(config);
        let caps = adapter.get_capabilities();

        assert_eq!(caps.max_message_size, 512);
        assert_eq!(caps.typical_latency_ms, 100.0);
        assert_eq!(caps.power_consumption, PowerConsumption::VeryLow);
    }

    #[test]
    fn test_ble_config_default() {
        let config = BleConfig::default();
        assert_eq!(config.device_name, "MyriadMesh-BLE");
        assert!(config.advertising);
        assert_eq!(config.advertising_interval_ms, 1000);
    }

    #[test]
    fn test_parse_address() {
        let config = BleConfig::default();
        let adapter = BleAdapter::new(config);

        let addr = adapter.parse_address("AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(addr, Address::BluetoothLE("AA:BB:CC:DD:EE:FF".to_string()));

        assert!(adapter.parse_address("invalid").is_err());
    }

    #[test]
    fn test_supports_address() {
        let config = BleConfig::default();
        let adapter = BleAdapter::new(config);

        assert!(adapter.supports_address(&Address::BluetoothLE("AA:BB:CC:DD:EE:FF".to_string())));
        assert!(!adapter.supports_address(&Address::Bluetooth("00:11:22:33:44:55".to_string())));
    }
}
