//! MyriadMesh Protocol Module
//!
//! This module defines the core protocol data structures and message formats
//! for the MyriadMesh network.

pub mod error;
pub mod frame;
pub mod message;
pub mod types;

pub use error::{ProtocolError, Result};
pub use frame::{Frame, FrameHeader};
pub use message::{Message, MessageId, MessageType};
pub use types::NodeId;

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        assert_eq!(2 + 2, 4);
    }
}
