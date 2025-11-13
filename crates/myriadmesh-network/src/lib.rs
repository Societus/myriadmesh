//! MyriadMesh Network Abstraction Layer
//!
//! This module provides a unified interface for multiple network transport types:
//! - Ethernet/IP (UDP)
//! - Bluetooth (Classic and LE)
//! - Cellular (4G/5G)
//! - LoRaWAN
//! - Radio (APRS, CB, Shortwave, FRS/GMRS)
//! - Overlay networks (i2p)

pub mod adapter;
pub mod adapters;
pub mod error;
pub mod i2p;
pub mod manager;
pub mod metrics;
pub mod types;

pub use adapter::{AdapterStatus, NetworkAdapter};
pub use adapters::{
    BleAdapter, BleConfig, BluetoothAdapter, BluetoothConfig, CellularAdapter, CellularConfig,
    EthernetAdapter, EthernetConfig, NetworkType,
};
pub use error::{NetworkError, Result};
pub use i2p::{I2pAdapter, I2pRouterConfig};
pub use manager::AdapterManager;
pub use metrics::AdapterMetrics;
pub use types::{AdapterCapabilities, Address, PowerConsumption};

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        assert_eq!(2 + 2, 4);
    }
}
