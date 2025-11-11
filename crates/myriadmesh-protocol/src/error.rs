//! Error types for protocol operations

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ProtocolError>;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ProtocolError {
    #[error("Invalid message format")]
    InvalidMessageFormat,

    #[error("Invalid frame format")]
    InvalidFrameFormat,

    #[error("Invalid message type: {0}")]
    InvalidMessageType(u8),

    #[error("Message too large: {size} bytes (max: {max})")]
    MessageTooLarge { size: usize, max: usize },

    #[error("Invalid message ID")]
    InvalidMessageId,

    #[error("Invalid node ID")]
    InvalidNodeId,

    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("TTL exceeded")]
    TtlExceeded,

    #[error("Missing required field: {0}")]
    MissingField(String),
}
