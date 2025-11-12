//! Routing error types

use thiserror::Error;

/// Routing-specific errors
#[derive(Error, Debug)]
pub enum RoutingError {
    #[error("TTL exceeded")]
    TtlExceeded,

    #[error("Destination not found: {0}")]
    DestinationNotFound(String),

    #[error("Destination unknown")]
    DestinationUnknown,

    #[error("No route to destination")]
    NoRoute,

    #[error("Message replay detected")]
    ReplayDetected,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid timestamp")]
    InvalidTimestamp,

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Global rate limit exceeded")]
    GlobalRateLimitExceeded,

    #[error("Queue full: {0}")]
    QueueFull(String),

    #[error("Cache full")]
    CacheFull,

    #[error("Message filtered by policy")]
    MessageFiltered,

    #[error("Insufficient relays for onion routing")]
    InsufficientRelays,

    #[error("Protocol error: {0}")]
    Protocol(#[from] myriadmesh_protocol::ProtocolError),

    #[error("Crypto error: {0}")]
    Crypto(#[from] myriadmesh_crypto::CryptoError),

    #[error("DHT error: {0}")]
    Dht(#[from] myriadmesh_dht::DhtError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for routing operations
pub type Result<T> = std::result::Result<T, RoutingError>;
