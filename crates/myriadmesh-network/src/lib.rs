//! MyriadMesh Network Abstraction Layer
//!
//! This module provides a unified interface for multiple network transport types:
//! - Ethernet/IP (UDP)
//! - Bluetooth (Classic and LE)
//! - Cellular (4G/5G)
//! - LoRaWAN
//! - Radio (APRS, CB, Shortwave, FRS/GMRS)
//! - Overlay networks (i2p)
//!
//! ## Adaptive Security
//!
//! The network layer includes adaptive security features:
//! - Component version tracking with reputation impact
//! - Hot-reloadable adapters for zero-downtime updates
//! - Coordinated update scheduling with network neighbors
//! - Health monitoring and automatic rollback

pub mod adapter;
pub mod adapters;
pub mod error;
pub mod i2p;
pub mod manager;
pub mod metrics;
pub mod reload;
pub mod types;
pub mod version_tracking;

pub use adapter::{AdapterStatus, NetworkAdapter};
pub use adapters::{
    BleAdapter, BleConfig, BluetoothAdapter, BluetoothConfig, CellularAdapter, CellularConfig,
    EthernetAdapter, EthernetConfig, NetworkType,
};
pub use error::{NetworkError, Result};
pub use i2p::{I2pAdapter, I2pRouterConfig};
pub use manager::AdapterManager;
pub use metrics::AdapterMetrics;
pub use reload::{AdapterLoadStatus, AdapterMetadata, AdapterRegistry};
pub use types::{AdapterCapabilities, Address, PowerConsumption};
pub use version_tracking::{
    AdapterComponentStatus, AdapterVersionInfo, ComponentManifest, CveInfo, CveSeverity,
    SemanticVersion, calculate_version_penalty,
};

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        assert_eq!(2 + 2, 4);
    }
}
