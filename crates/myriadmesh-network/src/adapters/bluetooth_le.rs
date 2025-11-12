//! Bluetooth Low Energy (BLE) network adapter
//!
//! This adapter provides connectivity through BLE for energy-efficient
//! short-range communication. Optimized for low power consumption and
//! periodic data transmission.

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::{types::AdapterType, Frame, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    rssi: i8,
    last_seen: u64,
    connected: bool,
}

/// Bluetooth Low Energy network adapter
pub struct BleAdapter {
    config: BleConfig,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    peers: Arc<RwLock<HashMap<String, BlePeer>>>,
    local_address: Option<String>,
}

impl BleAdapter {
    /// Create a new BLE adapter
    pub fn new(config: BleConfig) -> Self {
        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::BluetoothLE,
            max_message_size: 512,
            typical_latency_ms: 100.0,
            typical_bandwidth_bps: 1_000_000,
            reliability: 0.90,
            range_meters: 50.0,
            power_consumption: PowerConsumption::VeryLow,
            cost_per_mb: 0.0,
            supports_broadcast: true,
            supports_multicast: false,
        };

        Self {
            config,
            status: Arc::new(RwLock::new(AdapterStatus::Uninitialized)),
            capabilities,
            peers: Arc::new(RwLock::new(HashMap::new())),
            local_address: None,
        }
    }

    async fn start_advertising(&self) -> Result<()> {
        // TODO: Implement BLE advertising
        Ok(())
    }

    async fn stop_advertising(&self) -> Result<()> {
        // TODO: Stop BLE advertising
        Ok(())
    }

    async fn scan_for_devices(&self) -> Result<Vec<BlePeer>> {
        // TODO: Implement BLE scanning
        Ok(Vec::new())
    }
}

#[async_trait::async_trait]
impl NetworkAdapter for BleAdapter {
    async fn initialize(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::Initializing;

        // TODO: Initialize BLE adapter
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

        if self.config.advertising {
            self.start_advertising().await?;
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::ShuttingDown;

        self.stop_advertising().await?;

        *self.status.write().await = AdapterStatus::Uninitialized;
        Ok(())
    }

    async fn send(&self, destination: &Address, _frame: &Frame) -> Result<()> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }

        let _ble_address = match destination {
            Address::BluetoothLE(addr) => addr,
            _ => return Err(NetworkError::InvalidAddress("Expected Bluetooth LE address".to_string())),
        };

        // TODO: Send frame over BLE GATT characteristic

        Ok(())
    }

    async fn receive(&self, _timeout_ms: u64) -> Result<(Address, Frame)> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }

        // TODO: Receive frame from any connected peer

        Err(NetworkError::Timeout)
    }

    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }

        let discovered = self.scan_for_devices().await?;

        let mut peers = self.peers.write().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        for peer in discovered {
            peers.insert(peer.address.clone(), peer);
        }

        Ok(peers
            .values()
            .filter(|p| now - p.last_seen < 60)
            .map(|p| PeerInfo {
                node_id: NodeId::from_bytes([0u8; 32]),
                address: Address::BluetoothLE(p.address.clone()),
            })
            .collect())
    }

    fn get_status(&self) -> AdapterStatus {
        futures::executor::block_on(self.status.read()).clone()
    }

    fn get_capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    async fn test_connection(&self, destination: &Address) -> Result<TestResults> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }

        let _ble_address = match destination {
            Address::BluetoothLE(addr) => addr,
            _ => return Err(NetworkError::InvalidAddress("Expected Bluetooth LE address".to_string())),
        };

        // TODO: Implement connection test

        Ok(TestResults {
            success: true,
            rtt_ms: Some(100.0),
            error: None,
        })
    }

    fn get_local_address(&self) -> Option<Address> {
        self.local_address
            .as_ref()
            .map(|addr| Address::BluetoothLE(addr.clone()))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        let parts: Vec<&str> = addr_str.split(':').collect();
        if parts.len() != 6 {
            return Err(NetworkError::InvalidAddress("BLE address must be in format XX:XX:XX:XX:XX:XX".to_string()));
        }

        for part in &parts {
            if part.len() != 2 || !part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(NetworkError::InvalidAddress("BLE address must contain hex digits only".to_string()));
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

        assert_eq!(adapter.get_capabilities().adapter_type, AdapterType::BluetoothLE);
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
