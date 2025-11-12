//! I2P network adapter with embedded router support
//!
//! This module provides zero-configuration i2p integration by:
//! - Automatically managing an embedded i2pd router process
//! - Detecting and using existing system i2p routers
//! - Persisting i2p destination keys across restarts
//! - Providing SAM (Simple Anonymous Messaging) protocol client

pub mod embedded_router;
pub mod sam_client;
pub mod adapter;

pub use embedded_router::{EmbeddedI2pRouter, I2pRouterMode, I2pRouterConfig, I2pRouterError};
pub use sam_client::{SamConnection, SamDestination, SamSession, SessionStyle, SamError};
pub use adapter::I2pAdapter;
