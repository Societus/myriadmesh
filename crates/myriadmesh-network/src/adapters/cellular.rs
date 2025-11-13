//! Cellular (4G/5G) network adapter

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::{types::AdapterType, Frame};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

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

pub struct CellularAdapter {
    config: CellularConfig,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    connection_state: Arc<RwLock<ConnectionState>>,
    local_ip: Option<String>,
}

impl CellularAdapter {
    pub fn new(config: CellularConfig) -> Self {
        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::Cellular,
            max_message_size: 1024 * 1024,
            typical_latency_ms: 40.0,
            typical_bandwidth_bps: 50_000_000,
            reliability: 0.98,
            range_meters: 0.0,
            power_consumption: PowerConsumption::High,
            cost_per_mb: config.cost_per_mb,
            supports_broadcast: false,
            supports_multicast: false,
        };

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
            local_ip: None,
        }
    }

    async fn establish_connection(&mut self) -> Result<()> {
        // TODO: Implement cellular connection
        let mut state = self.connection_state.write().await;
        state.connected = true;
        state.network_type = Some(self.config.preferred_network);
        state.signal_strength = 75;
        state.connection_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.local_ip = Some("10.0.0.1".to_string());
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        // TODO: Implement disconnect
        let mut state = self.connection_state.write().await;
        state.connected = false;
        state.network_type = None;
        Ok(())
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

        if self.check_data_cap().await {
            return Err(NetworkError::QuotaExceeded);
        }

        let _ip_addr = match destination {
            Address::Cellular(addr) => addr,
            _ => {
                return Err(NetworkError::InvalidAddress(
                    "Expected cellular address".to_string(),
                ))
            }
        };

        // TODO: Send frame over cellular connection
        let bytes_sent = frame.payload.len() as u64;
        self.update_data_usage(bytes_sent).await;

        Ok(())
    }

    async fn receive(&self, _timeout_ms: u64) -> Result<(Address, Frame)> {
        let status = self.status.read().await;
        if *status != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }

        // TODO: Receive frame over cellular connection
        Err(NetworkError::Timeout)
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

        let _ip_addr = match destination {
            Address::Cellular(addr) => addr,
            _ => {
                return Err(NetworkError::InvalidAddress(
                    "Expected cellular address".to_string(),
                ))
            }
        };

        // TODO: Implement ping test
        let state = self.connection_state.read().await;
        let latency = match state.network_type {
            Some(NetworkType::FiveG) => 20.0,
            Some(NetworkType::LTE) => 40.0,
            Some(NetworkType::ThreeG) => 100.0,
            Some(NetworkType::TwoG) => 300.0,
            _ => 50.0,
        };

        Ok(TestResults {
            success: true,
            rtt_ms: Some(latency),
            error: None,
        })
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
