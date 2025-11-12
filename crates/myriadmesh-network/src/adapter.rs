//! Network adapter trait

use crate::error::Result;
use crate::types::{Address, AdapterCapabilities};
use myriadmesh_protocol::{Frame, NodeId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Adapter status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterStatus {
    /// Adapter is not initialized
    Uninitialized,
    /// Adapter is initializing
    Initializing,
    /// Adapter is ready and operational
    Ready,
    /// Adapter is temporarily unavailable
    Unavailable,
    /// Adapter encountered an error
    Error,
    /// Adapter is shutting down
    ShuttingDown,
}

impl fmt::Display for AdapterStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdapterStatus::Uninitialized => write!(f, "Uninitialized"),
            AdapterStatus::Initializing => write!(f, "Initializing"),
            AdapterStatus::Ready => write!(f, "Ready"),
            AdapterStatus::Unavailable => write!(f, "Unavailable"),
            AdapterStatus::Error => write!(f, "Error"),
            AdapterStatus::ShuttingDown => write!(f, "Shutting Down"),
        }
    }
}

/// Information about a discovered peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer node ID
    pub node_id: NodeId,

    /// Peer address on this adapter
    pub address: Address,
}

/// Test results for adapter connection testing
#[derive(Debug, Clone)]
pub struct TestResults {
    /// Whether the test succeeded
    pub success: bool,

    /// Round-trip time in milliseconds
    pub rtt_ms: Option<f64>,

    /// Error message if test failed
    pub error: Option<String>,
}

/// Network adapter trait
///
/// All network transport implementations must implement this trait
#[async_trait::async_trait]
pub trait NetworkAdapter: Send + Sync {
    /// Initialize the adapter
    async fn initialize(&mut self) -> Result<()>;

    /// Start the adapter
    async fn start(&mut self) -> Result<()>;

    /// Stop the adapter
    async fn stop(&mut self) -> Result<()>;

    /// Send a frame to a destination
    async fn send(&self, destination: &Address, frame: &Frame) -> Result<()>;

    /// Receive the next frame (blocking until frame arrives or timeout)
    async fn receive(&self, timeout_ms: u64) -> Result<(Address, Frame)>;

    /// Discover peers on this network
    async fn discover_peers(&self) -> Result<Vec<PeerInfo>>;

    /// Get adapter status
    fn get_status(&self) -> AdapterStatus;

    /// Get adapter capabilities
    fn get_capabilities(&self) -> &AdapterCapabilities;

    /// Test connection to a destination
    async fn test_connection(&self, destination: &Address) -> Result<TestResults>;

    /// Get local address for this adapter
    fn get_local_address(&self) -> Option<Address>;

    /// Parse an address string for this adapter
    fn parse_address(&self, addr_str: &str) -> Result<Address>;

    /// Check if adapter supports a specific address type
    fn supports_address(&self, address: &Address) -> bool;
}
