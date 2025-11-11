//! Frame format for wire transmission
//!
//! Frames are the on-wire representation of messages, including header and payload.

use serde::{Deserialize, Serialize};

use crate::error::{ProtocolError, Result};
use crate::message::Message;

/// Protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Magic bytes to identify MyriadMesh frames
pub const MAGIC_BYTES: [u8; 4] = [0x4D, 0x59, 0x52, 0x44]; // "MYRD"

/// Maximum frame size (1 MB + header overhead)
pub const MAX_FRAME_SIZE: usize = 1024 * 1024 + 256;

/// Frame header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameHeader {
    /// Magic bytes for protocol identification
    pub magic: [u8; 4],

    /// Protocol version
    pub version: u8,

    /// Frame flags (reserved for future use)
    pub flags: u8,

    /// Length of the payload (in bytes)
    pub payload_length: u32,

    /// CRC32 checksum of the payload
    pub checksum: u32,
}

impl FrameHeader {
    /// Create a new frame header
    pub fn new(payload_length: u32, checksum: u32) -> Self {
        FrameHeader {
            magic: MAGIC_BYTES,
            version: PROTOCOL_VERSION,
            flags: 0,
            payload_length,
            checksum,
        }
    }

    /// Validate the header
    pub fn validate(&self) -> Result<()> {
        if self.magic != MAGIC_BYTES {
            return Err(ProtocolError::InvalidFrameFormat);
        }

        if self.version != PROTOCOL_VERSION {
            return Err(ProtocolError::ValidationFailed(format!(
                "Unsupported protocol version: {}",
                self.version
            )));
        }

        if self.payload_length as usize > MAX_FRAME_SIZE {
            return Err(ProtocolError::MessageTooLarge {
                size: self.payload_length as usize,
                max: MAX_FRAME_SIZE,
            });
        }

        Ok(())
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| ProtocolError::SerializationFailed(e.to_string()))
    }

    /// Deserialize header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|e| ProtocolError::DeserializationFailed(e.to_string()))
    }
}

/// A complete frame with header and payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    /// Frame header
    pub header: FrameHeader,

    /// Serialized message payload
    pub payload: Vec<u8>,
}

impl Frame {
    /// Create a new frame from a message
    pub fn from_message(message: &Message) -> Result<Self> {
        // Serialize the message
        let payload = bincode::serialize(message)
            .map_err(|e| ProtocolError::SerializationFailed(e.to_string()))?;

        // Calculate checksum
        let checksum = crc32fast::hash(&payload);

        // Create header
        let header = FrameHeader::new(payload.len() as u32, checksum);

        Ok(Frame { header, payload })
    }

    /// Parse a message from this frame
    pub fn to_message(&self) -> Result<Message> {
        // Validate checksum
        let checksum = crc32fast::hash(&self.payload);
        if checksum != self.header.checksum {
            return Err(ProtocolError::ValidationFailed(
                "Checksum mismatch".to_string(),
            ));
        }

        // Deserialize message
        bincode::deserialize(&self.payload)
            .map_err(|e| ProtocolError::DeserializationFailed(e.to_string()))
    }

    /// Serialize frame to bytes for transmission
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| ProtocolError::SerializationFailed(e.to_string()))
    }

    /// Deserialize frame from bytes
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        let frame: Frame = bincode::deserialize(bytes)
            .map_err(|e| ProtocolError::DeserializationFailed(e.to_string()))?;

        // Validate header
        frame.header.validate()?;

        Ok(frame)
    }

    /// Get the total size of the frame
    pub fn size(&self) -> usize {
        // Approximate size (header + payload)
        16 + self.payload.len()
    }
}

/// Utility function to calculate CRC32 checksum
fn crc32fast_hash(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{MessageId, MessageType};
    use crate::types::{NodeId, Priority};

    fn create_test_message() -> Message {
        Message {
            id: MessageId::from_bytes([1u8; 32]),
            source: NodeId::from_bytes([2u8; 32]),
            destination: NodeId::from_bytes([3u8; 32]),
            message_type: MessageType::Data,
            priority: Priority::Normal,
            ttl: 32,
            timestamp: 1234567890,
            sequence: 42,
            payload: b"Hello, MyriadMesh!".to_vec(),
        }
    }

    #[test]
    fn test_frame_creation() {
        let message = create_test_message();
        let frame = Frame::from_message(&message).unwrap();

        assert_eq!(frame.header.magic, MAGIC_BYTES);
        assert_eq!(frame.header.version, PROTOCOL_VERSION);
        assert!(frame.header.payload_length > 0);
    }

    #[test]
    fn test_frame_roundtrip() {
        let message = create_test_message();
        let frame = Frame::from_message(&message).unwrap();

        let recovered_message = frame.to_message().unwrap();

        assert_eq!(message.id, recovered_message.id);
        assert_eq!(message.source, recovered_message.source);
        assert_eq!(message.destination, recovered_message.destination);
        assert_eq!(message.payload, recovered_message.payload);
    }

    #[test]
    fn test_frame_serialization() {
        let message = create_test_message();
        let frame = Frame::from_message(&message).unwrap();

        let bytes = frame.serialize().unwrap();
        let deserialized = Frame::deserialize(&bytes).unwrap();

        assert_eq!(frame.header.checksum, deserialized.header.checksum);
        assert_eq!(frame.payload, deserialized.payload);
    }

    #[test]
    fn test_checksum_validation() {
        let message = create_test_message();
        let mut frame = Frame::from_message(&message).unwrap();

        // Tamper with payload
        if !frame.payload.is_empty() {
            frame.payload[0] ^= 0xFF;
        }

        // Should fail checksum validation
        assert!(frame.to_message().is_err());
    }

    #[test]
    fn test_header_validation() {
        let mut header = FrameHeader::new(100, 0x12345678);

        // Valid header
        assert!(header.validate().is_ok());

        // Invalid magic
        header.magic = [0, 0, 0, 0];
        assert!(header.validate().is_err());
        header.magic = MAGIC_BYTES;

        // Invalid version
        header.version = 99;
        assert!(header.validate().is_err());
        header.version = PROTOCOL_VERSION;

        // Too large payload
        header.payload_length = (MAX_FRAME_SIZE + 1) as u32;
        assert!(header.validate().is_err());
    }

    #[test]
    fn test_header_serialization() {
        let header = FrameHeader::new(1024, 0xABCDEF01);

        let bytes = header.to_bytes().unwrap();
        let deserialized = FrameHeader::from_bytes(&bytes).unwrap();

        assert_eq!(header.magic, deserialized.magic);
        assert_eq!(header.version, deserialized.version);
        assert_eq!(header.payload_length, deserialized.payload_length);
        assert_eq!(header.checksum, deserialized.checksum);
    }
}
