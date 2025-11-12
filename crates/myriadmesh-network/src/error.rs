//! Network error types

use thiserror::Error;

/// Network-specific errors
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Adapter not found: {0}")]
    AdapterNotFound(String),

    #[error("Adapter initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Adapter already registered: {0}")]
    AdapterAlreadyRegistered(String),

    #[error("No adapters available")]
    NoAdaptersAvailable,

    #[error("No common adapter with destination")]
    NoCommonAdapter,

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    #[error("Message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: usize, max: usize },

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Discovery failed: {0}")]
    DiscoveryFailed(String),

    #[error("Adapter health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Protocol error: {0}")]
    Protocol(#[from] myriadmesh_protocol::ProtocolError),

    #[error("Crypto error: {0}")]
    Crypto(#[from] myriadmesh_crypto::CryptoError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for network operations
pub type Result<T> = std::result::Result<T, NetworkError>;
