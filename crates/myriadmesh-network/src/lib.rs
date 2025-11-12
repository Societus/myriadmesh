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
pub mod manager;
pub mod metrics;
pub mod types;

pub use adapter::{NetworkAdapter, AdapterStatus};
pub use adapters::{EthernetAdapter, EthernetConfig};
pub use error::{NetworkError, Result};
pub use manager::AdapterManager;
pub use metrics::AdapterMetrics;
pub use types::{Address, AdapterCapabilities, PowerConsumption};

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        assert_eq!(2 + 2, 4);
    }
}
