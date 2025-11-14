//! MyriadMesh Appliance Module
//!
//! This module provides appliance functionality for MyriadMesh nodes, enabling them to act as
//! gateways and caching proxies for mobile devices.
//!
//! # Features
//!
//! - **Device Pairing**: Secure QR code and PIN-based pairing with mobile devices
//! - **Message Caching**: Store-and-forward messaging with priority queues
//! - **Configuration Sync**: Synchronize preferences and routing policies
//! - **Relay & Bridge**: Proxy routing for mobile devices

pub mod cache;
pub mod device;
pub mod manager;
pub mod pairing;
pub mod types;

// Re-export commonly used types
pub use cache::{MessageCache, MessageCacheConfig, MessagePriority};
pub use device::{PairedDevice, PairedDeviceInfo};
pub use manager::{ApplianceManager, ApplianceStats};
pub use pairing::{PairingMethod, PairingRequest, PairingResult, PairingToken};
pub use types::{ApplianceCapabilities, ApplianceError, ApplianceResult};
