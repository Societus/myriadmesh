//! MyriadMesh Network Abstraction Module
//!
//! This module provides a unified interface for managing multiple network adapters
//! (Ethernet, Bluetooth, Wi-Fi, Radio, etc.) in the MyriadMesh protocol stack.
//!
//! # Architecture
//!
//! The network abstraction layer consists of:
//! - `NetworkAdapter` trait: Interface that all adapters must implement
//! - `AdapterManager`: Coordinates multiple adapters and routes frames
//!
//! # Example
//!
//! ```no_run
//! use myriadmesh_network::{AdapterManager, ManagerConfig};
//!
//! #[tokio::main]
//! async fn main() {
//!     let manager = AdapterManager::new(ManagerConfig::default());
//!
//!     // Register adapters...
//!     // manager.register_adapter(my_ethernet_adapter);
//!     // manager.register_adapter(my_bluetooth_adapter);
//!
//!     // Receive frames from any adapter
//!     while let Some(received) = manager.receive().await {
//!         println!("Received frame from {}", received.sender);
//!     }
//! }
//! ```

pub mod adapter;
pub mod manager;

pub use adapter::{
    AdapterInfo, AdapterStats, NetworkAdapter, NetworkError, Result,
};
pub use manager::{AdapterManager, ManagerConfig, ReceivedFrame};

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_imports() {
        // Verify that all public types are accessible
        use crate::{AdapterInfo, AdapterManager, AdapterStats, ManagerConfig, ReceivedFrame};

        // Just check that the types exist and can be referenced
        let _config: Option<ManagerConfig> = None;
        let _stats: Option<AdapterStats> = None;
        let _info: Option<AdapterInfo> = None;
        let _received: Option<ReceivedFrame> = None;
        let _manager: Option<AdapterManager> = None;
    }
}
