//! Network adapter trait definition
//!
//! This module defines the interface that all network adapters must implement.

use async_trait::async_trait;
use myriadmesh_protocol::{Frame, NodeId};
use myriadmesh_protocol::types::AdapterType;
use std::fmt;
use thiserror::Error;

/// Errors that can occur during network operations
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Failed to send frame: {0}")]
    SendFailed(String),

    #[error("Failed to receive frame: {0}")]
    ReceiveFailed(String),

    #[error("Adapter not available")]
    AdapterUnavailable,

    #[error("Invalid frame format: {0}")]
    InvalidFrame(String),

    #[error("Timeout waiting for response")]
    Timeout,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, NetworkError>;

/// Statistics about adapter performance
#[derive(Debug, Clone, Default)]
pub struct AdapterStats {
    /// Total frames sent
    pub frames_sent: u64,

    /// Total frames received
    pub frames_received: u64,

    /// Total bytes sent
    pub bytes_sent: u64,

    /// Total bytes received
    pub bytes_received: u64,

    /// Number of send failures
    pub send_failures: u64,

    /// Number of receive failures
    pub receive_failures: u64,
}

/// Information about a network adapter
#[derive(Debug, Clone)]
pub struct AdapterInfo {
    /// Unique identifier for this adapter instance
    pub id: String,

    /// Type of adapter (Ethernet, Bluetooth, etc.)
    pub adapter_type: AdapterType,

    /// Human-readable name
    pub name: String,

    /// Maximum transmission unit (bytes)
    pub mtu: usize,

    /// Whether the adapter is currently available
    pub available: bool,

    /// Optional adapter-specific address (e.g., MAC, IP, etc.)
    pub address: Option<String>,
}

/// Network adapter trait
///
/// All network transport implementations must implement this trait to
/// integrate with the MyriadMesh network stack.
#[async_trait]
pub trait NetworkAdapter: Send + Sync + fmt::Debug {
    /// Get adapter information
    fn info(&self) -> AdapterInfo;

    /// Get adapter type
    fn adapter_type(&self) -> AdapterType {
        self.info().adapter_type
    }

    /// Send a frame to a specific peer
    ///
    /// Returns Ok(()) if the frame was successfully sent, or an error otherwise.
    async fn send_to(&self, peer: &NodeId, frame: &Frame) -> Result<()>;

    /// Broadcast a frame to all reachable peers on this adapter
    ///
    /// Returns Ok(()) if the frame was successfully broadcast, or an error otherwise.
    async fn broadcast(&self, frame: &Frame) -> Result<()>;

    /// Receive the next frame from this adapter
    ///
    /// This should block until a frame is available or an error occurs.
    /// Returns the frame and the sender's NodeId.
    async fn receive(&self) -> Result<(NodeId, Frame)>;

    /// Check if the adapter is currently available
    async fn is_available(&self) -> bool;

    /// Get adapter statistics
    async fn stats(&self) -> AdapterStats;

    /// Shutdown the adapter gracefully
    async fn shutdown(&self) -> Result<()>;
}
