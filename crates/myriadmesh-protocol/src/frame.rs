//! Frame format for wire transmission
//!
//! Frames are the on-wire representation of messages, following the MyriadMesh
//! Protocol Specification (docs/protocol/specification.md).
//!
//! Frame Structure (163-byte header + payload + 64-byte signature):
//! - Magic (4 bytes): 0x4D594D53 ("MYMS")
//! - Version (1 byte): Protocol version (0x01)
//! - Flags (1 byte): Message flags bitfield
//! - Message Type (1 byte): Type of message
//! - Priority (1 byte): Message priority (0-255)
//! - TTL (1 byte): Time-to-live (hop count)
//! - Payload Length (2 bytes): Length of payload (big-endian)
//! - Message ID (16 bytes): Unique message identifier
//! - Source Node ID (64 bytes): Sender's node ID (SECURITY C6: increased for collision resistance)
//! - Dest Node ID (64 bytes): Recipient's node ID (SECURITY C6: increased for collision resistance)
//! - Timestamp (8 bytes): Unix timestamp in milliseconds (big-endian)
//! - Payload (variable): Encrypted message payload
//! - Signature (64 bytes): Ed25519 signature of header+payload

use serde::{Deserialize, Serialize};

use crate::error::{ProtocolError, Result};
use crate::message::{Message, MessageId, MessageType};
use crate::types::{NodeId, Priority, NODE_ID_SIZE};

/// Protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Magic bytes to identify MyriadMesh frames: "MYMS"
pub const MAGIC_BYTES: [u8; 4] = [0x4D, 0x59, 0x4D, 0x53];

/// Total header size: 4 + 1 + 1 + 1 + 1 + 1 + 2 + 16 + 64 + 64 + 8 = 163 bytes
///
/// SECURITY C6: Increased from 99 to 163 bytes due to NodeID expansion (32→64 bytes each)
/// for collision resistance against birthday attacks.
pub const HEADER_SIZE: usize = 163;

/// Signature size (64 bytes for Ed25519)
pub const SIGNATURE_SIZE: usize = 64;

/// Maximum frame size (1 MB + header + signature)
pub const MAX_FRAME_SIZE: usize = 1024 * 1024 + HEADER_SIZE + SIGNATURE_SIZE;

/// Maximum payload size
pub const MAX_PAYLOAD_SIZE: usize = 65535;

/// Frame flags bitfield (per specification.md:77-87)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameFlags(u8);

impl FrameFlags {
    /// Payload is encrypted (Bit 0)
    pub const ENCRYPTED: u8 = 0b0000_0001;

    /// Signature present (Bit 1)
    pub const SIGNED: u8 = 0b0000_0010;

    /// Payload is compressed (Bit 2)
    pub const COMPRESSED: u8 = 0b0000_0100;

    /// Message is being relayed (Bit 3)
    pub const RELAY: u8 = 0b0000_1000;

    /// Sender wants delivery confirmation (Bit 4)
    pub const ACK_REQUIRED: u8 = 0b0001_0000;

    /// This is an acknowledgment (Bit 5)
    pub const ACK_MESSAGE: u8 = 0b0010_0000;

    /// Message to all nodes (Bit 6)
    pub const BROADCAST: u8 = 0b0100_0000;

    /// Reserved (Bit 7)
    pub const RESERVED: u8 = 0b1000_0000;

    /// Create new frame flags
    pub fn new(flags: u8) -> Self {
        FrameFlags(flags)
    }

    /// Check if flag is set
    pub fn contains(&self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    /// Set a flag
    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    /// Clear a flag
    pub fn clear(&mut self, flag: u8) {
        self.0 &= !flag;
    }

    /// Get raw value
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

impl Default for FrameFlags {
    fn default() -> Self {
        // By default: encrypted and signed
        FrameFlags(Self::ENCRYPTED | Self::SIGNED)
    }
}

/// Frame header (99 bytes fixed size)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameHeader {
    /// Magic bytes for protocol identification
    pub magic: [u8; 4],

    /// Protocol version
    pub version: u8,

    /// Frame flags
    pub flags: FrameFlags,

    /// Message type
    pub message_type: MessageType,

    /// Message priority (0-255)
    pub priority: Priority,

    /// Time-to-live (hop count)
    pub ttl: u8,

    /// Length of the payload (in bytes, 0-65535)
    pub payload_length: u16,

    /// Unique message identifier (16 bytes)
    pub message_id: MessageId,

    /// Source node ID (32 bytes)
    pub source: NodeId,

    /// Destination node ID (32 bytes)
    pub destination: NodeId,

    /// Unix timestamp in milliseconds (8 bytes)
    pub timestamp: u64,
}

impl FrameHeader {
    /// Create a new frame header
    pub fn new(
        message_type: MessageType,
        source: NodeId,
        destination: NodeId,
        payload_length: u16,
        message_id: MessageId,
        timestamp: u64,
    ) -> Self {
        FrameHeader {
            magic: MAGIC_BYTES,
            version: PROTOCOL_VERSION,
            flags: FrameFlags::default(),
            message_type,
            priority: Priority::default(),
            ttl: 32, // Default TTL
            payload_length,
            message_id,
            source,
            destination,
            timestamp,
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

        if self.payload_length as usize > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::MessageTooLarge {
                size: self.payload_length as usize,
                max: MAX_PAYLOAD_SIZE,
            });
        }

        if self.ttl == 0 {
            return Err(ProtocolError::TtlExceeded);
        }

        Ok(())
    }

    /// Serialize header to bytes (99 bytes)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_SIZE);

        // Magic (4 bytes)
        bytes.extend_from_slice(&self.magic);

        // Version (1 byte)
        bytes.push(self.version);

        // Flags (1 byte)
        bytes.push(self.flags.as_u8());

        // Message Type (1 byte)
        bytes.push(self.message_type.to_u8());

        // Priority (1 byte)
        bytes.push(self.priority.as_u8());

        // TTL (1 byte)
        bytes.push(self.ttl);

        // Payload Length (2 bytes, big-endian)
        bytes.extend_from_slice(&self.payload_length.to_be_bytes());

        // Message ID (16 bytes)
        bytes.extend_from_slice(self.message_id.as_bytes());

        // Source Node ID (32 bytes)
        bytes.extend_from_slice(self.source.as_bytes());

        // Destination Node ID (32 bytes)
        bytes.extend_from_slice(self.destination.as_bytes());

        // Timestamp (8 bytes, big-endian)
        bytes.extend_from_slice(&self.timestamp.to_be_bytes());

        debug_assert_eq!(bytes.len(), HEADER_SIZE, "Header size mismatch");

        bytes
    }

    /// Deserialize header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_SIZE {
            return Err(ProtocolError::InvalidFrameFormat);
        }

        let mut offset = 0;

        // Magic (4 bytes)
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[offset..offset + 4]);
        offset += 4;

        // Version (1 byte)
        let version = bytes[offset];
        offset += 1;

        // Flags (1 byte)
        let flags = FrameFlags::new(bytes[offset]);
        offset += 1;

        // Message Type (1 byte)
        let message_type = MessageType::from_u8(bytes[offset])?;
        offset += 1;

        // Priority (1 byte)
        let priority = Priority::from_u8(bytes[offset]);
        offset += 1;

        // TTL (1 byte)
        let ttl = bytes[offset];
        offset += 1;

        // Payload Length (2 bytes, big-endian)
        let payload_length = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;

        // Message ID (16 bytes)
        let mut message_id_bytes = [0u8; 16];
        message_id_bytes.copy_from_slice(&bytes[offset..offset + 16]);
        let message_id = MessageId::from_bytes(message_id_bytes);
        offset += 16;

        // SECURITY C6: Source Node ID (64 bytes for collision resistance)
        let mut source_bytes = [0u8; NODE_ID_SIZE];
        source_bytes.copy_from_slice(&bytes[offset..offset + NODE_ID_SIZE]);
        let source = NodeId::from_bytes(source_bytes);
        offset += NODE_ID_SIZE;

        // SECURITY C6: Destination Node ID (64 bytes for collision resistance)
        let mut dest_bytes = [0u8; NODE_ID_SIZE];
        dest_bytes.copy_from_slice(&bytes[offset..offset + NODE_ID_SIZE]);
        let destination = NodeId::from_bytes(dest_bytes);
        offset += NODE_ID_SIZE;

        // Timestamp (8 bytes, big-endian)
        let timestamp = u64::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);

        let header = FrameHeader {
            magic,
            version,
            flags,
            message_type,
            priority,
            ttl,
            payload_length,
            message_id,
            source,
            destination,
            timestamp,
        };

        header.validate()?;

        Ok(header)
    }
}

/// A complete frame with header, payload, and signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    /// Frame header (99 bytes)
    pub header: FrameHeader,

    /// Message payload (variable, 0-65535 bytes)
    pub payload: Vec<u8>,

    /// Ed25519 signature (64 bytes) of header+payload
    pub signature: Vec<u8>,
}

impl Frame {
    /// Create a new frame
    pub fn new(
        message_type: MessageType,
        source: NodeId,
        destination: NodeId,
        payload: Vec<u8>,
        message_id: MessageId,
        timestamp: u64,
    ) -> Result<Self> {
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::MessageTooLarge {
                size: payload.len(),
                max: MAX_PAYLOAD_SIZE,
            });
        }

        let header = FrameHeader::new(
            message_type,
            source,
            destination,
            payload.len() as u16,
            message_id,
            timestamp,
        );

        Ok(Frame {
            header,
            payload,
            signature: Vec::new(), // Signature added separately
        })
    }

    /// Create a frame from a Message (compatibility helper)
    pub fn from_message(message: &Message) -> Result<Self> {
        Self::new(
            message.message_type,
            message.source,
            message.destination,
            message.payload.clone(),
            message.id,
            message.timestamp,
        )
    }

    /// Convert frame to Message (compatibility helper)
    pub fn to_message(&self) -> Result<Message> {
        Ok(Message {
            id: self.header.message_id,
            source: self.header.source,
            destination: self.header.destination,
            message_type: self.header.message_type,
            priority: self.header.priority,
            ttl: self.header.ttl,
            timestamp: self.header.timestamp,
            sequence: 0, // Not stored in frame
            payload: self.payload.clone(),
        })
    }

    /// Get bytes to sign (header + payload)
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut bytes = self.header.to_bytes();
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    /// Set the signature
    pub fn set_signature(&mut self, signature: Vec<u8>) -> Result<()> {
        if signature.len() != SIGNATURE_SIZE {
            return Err(ProtocolError::ValidationFailed(format!(
                "Invalid signature size: expected {}, got {}",
                SIGNATURE_SIZE,
                signature.len()
            )));
        }
        self.signature = signature;
        self.header.flags.set(FrameFlags::SIGNED);
        Ok(())
    }

    /// Verify signature (requires external crypto library)
    /// Returns the bytes that were signed for verification
    pub fn verify_signature_bytes(&self) -> Vec<u8> {
        self.signable_bytes()
    }

    /// Serialize frame to bytes for transmission
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = self.header.to_bytes();
        bytes.extend_from_slice(&self.payload);
        bytes.extend_from_slice(&self.signature);
        bytes
    }

    /// Deserialize frame from bytes
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_SIZE + SIGNATURE_SIZE {
            return Err(ProtocolError::InvalidFrameFormat);
        }

        // Parse header
        let header = FrameHeader::from_bytes(&bytes[0..HEADER_SIZE])?;

        // Calculate expected frame size
        let expected_size = HEADER_SIZE + header.payload_length as usize + SIGNATURE_SIZE;
        if bytes.len() != expected_size {
            return Err(ProtocolError::ValidationFailed(format!(
                "Frame size mismatch: expected {}, got {}",
                expected_size,
                bytes.len()
            )));
        }

        // Extract payload
        let payload_start = HEADER_SIZE;
        let payload_end = payload_start + header.payload_length as usize;
        let payload = bytes[payload_start..payload_end].to_vec();

        // Extract signature
        let signature = bytes[payload_end..payload_end + SIGNATURE_SIZE].to_vec();

        Ok(Frame {
            header,
            payload,
            signature,
        })
    }

    /// Get the total size of the frame
    pub fn size(&self) -> usize {
        HEADER_SIZE + self.payload.len() + SIGNATURE_SIZE
    }

    /// Validate the frame
    pub fn validate(&self) -> Result<()> {
        self.header.validate()?;

        if self.payload.len() != self.header.payload_length as usize {
            return Err(ProtocolError::ValidationFailed(
                "Payload length mismatch".to_string(),
            ));
        }

        if self.header.flags.contains(FrameFlags::SIGNED) && self.signature.len() != SIGNATURE_SIZE
        {
            return Err(ProtocolError::ValidationFailed(format!(
                "Invalid signature size: expected {}, got {}",
                SIGNATURE_SIZE,
                self.signature.len()
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_frame() -> Frame {
        let message_id = MessageId::from_bytes([1u8; 16]);
        let source = NodeId::from_bytes([2u8; 32]);
        let destination = NodeId::from_bytes([3u8; 32]);
        let payload = b"Hello, MyriadMesh!".to_vec();
        let timestamp = 1704067200000u64; // 2024-01-01 00:00:00 UTC in milliseconds

        Frame::new(
            MessageType::Data,
            source,
            destination,
            payload,
            message_id,
            timestamp,
        )
        .unwrap()
    }

    #[test]
    fn test_magic_bytes() {
        // Verify magic bytes spell "MYMS"
        assert_eq!(MAGIC_BYTES, [0x4D, 0x59, 0x4D, 0x53]);
        assert_eq!(std::str::from_utf8(&MAGIC_BYTES).unwrap(), "MYMS");
    }

    #[test]
    fn test_header_size() {
        assert_eq!(HEADER_SIZE, 99);
    }

    #[test]
    fn test_frame_flags() {
        let mut flags = FrameFlags::default();
        assert!(flags.contains(FrameFlags::ENCRYPTED));
        assert!(flags.contains(FrameFlags::SIGNED));
        assert!(!flags.contains(FrameFlags::COMPRESSED));

        flags.set(FrameFlags::COMPRESSED);
        assert!(flags.contains(FrameFlags::COMPRESSED));

        flags.clear(FrameFlags::COMPRESSED);
        assert!(!flags.contains(FrameFlags::COMPRESSED));
    }

    #[test]
    fn test_frame_creation() {
        let frame = create_test_frame();

        assert_eq!(frame.header.magic, MAGIC_BYTES);
        assert_eq!(frame.header.version, PROTOCOL_VERSION);
        assert_eq!(frame.header.message_type, MessageType::Data);
        assert_eq!(frame.payload, b"Hello, MyriadMesh!");
    }

    #[test]
    fn test_header_serialization() {
        let frame = create_test_frame();
        let header_bytes = frame.header.to_bytes();

        assert_eq!(header_bytes.len(), HEADER_SIZE);

        let deserialized = FrameHeader::from_bytes(&header_bytes).unwrap();
        assert_eq!(frame.header, deserialized);
    }

    #[test]
    fn test_frame_serialization() {
        let mut frame = create_test_frame();
        frame.set_signature(vec![0xAAu8; SIGNATURE_SIZE]).unwrap();

        let bytes = frame.serialize();
        let deserialized = Frame::deserialize(&bytes).unwrap();

        assert_eq!(frame.header.message_id, deserialized.header.message_id);
        assert_eq!(frame.payload, deserialized.payload);
        assert_eq!(frame.signature, deserialized.signature);
    }

    #[test]
    fn test_header_validation() {
        let mut frame = create_test_frame();

        // Valid frame
        assert!(frame.header.validate().is_ok());

        // Invalid magic
        frame.header.magic = [0, 0, 0, 0];
        assert!(frame.header.validate().is_err());
        frame.header.magic = MAGIC_BYTES;

        // Invalid version
        frame.header.version = 99;
        assert!(frame.header.validate().is_err());
        frame.header.version = PROTOCOL_VERSION;

        // TTL = 0
        frame.header.ttl = 0;
        assert!(frame.header.validate().is_err());
        frame.header.ttl = 32;

        // Payload size is validated at Frame creation, not in header
        // u16 max is 65535 which equals MAX_PAYLOAD_SIZE, so all u16 values are valid
    }

    #[test]
    fn test_frame_size() {
        let mut frame = create_test_frame();
        frame.set_signature(vec![0u8; SIGNATURE_SIZE]).unwrap();

        let expected_size = HEADER_SIZE + frame.payload.len() + SIGNATURE_SIZE;
        assert_eq!(frame.size(), expected_size);
    }

    #[test]
    fn test_signable_bytes() {
        let frame = create_test_frame();
        let signable = frame.signable_bytes();

        // Should be header + payload
        assert_eq!(signable.len(), HEADER_SIZE + frame.payload.len());
    }

    #[test]
    fn test_invalid_signature_size() {
        let mut frame = create_test_frame();

        // Wrong signature size should fail
        let result = frame.set_signature(vec![0u8; 32]);
        assert!(result.is_err());
    }
}
