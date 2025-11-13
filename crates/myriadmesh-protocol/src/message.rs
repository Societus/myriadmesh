//! Message types and structures

use blake2::{Blake2b512, Digest};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{ProtocolError, Result};
use crate::types::{NodeId, Priority};

#[cfg(test)]
use crate::types::NODE_ID_SIZE;

/// Size of a message ID in bytes (per specification.md:128-131)
pub const MESSAGE_ID_SIZE: usize = 16;

/// Maximum message payload size (1 MB)
pub const MAX_PAYLOAD_SIZE: usize = 1024 * 1024;

/// A unique identifier for a message
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId([u8; MESSAGE_ID_SIZE]);

impl MessageId {
    /// Generate a new message ID from message contents
    /// Uses BLAKE2b(timestamp + source_id + random_nonce)[0:16]
    pub fn generate(
        source: &NodeId,
        destination: &NodeId,
        payload: &[u8],
        timestamp: u64,
        sequence: u32,
    ) -> Self {
        let mut hasher = Blake2b512::new();

        hasher.update(timestamp.to_le_bytes());
        hasher.update(source.as_bytes());
        hasher.update(destination.as_bytes());
        hasher.update(payload);
        hasher.update(sequence.to_le_bytes());

        let hash = hasher.finalize();

        // Take first 16 bytes (per specification)
        let mut id = [0u8; MESSAGE_ID_SIZE];
        id.copy_from_slice(&hash[..MESSAGE_ID_SIZE]);

        MessageId(id)
    }

    /// Create from bytes
    pub fn from_bytes(bytes: [u8; MESSAGE_ID_SIZE]) -> Self {
        MessageId(bytes)
    }

    /// Get bytes
    pub fn as_bytes(&self) -> &[u8; MESSAGE_ID_SIZE] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes =
            hex::decode(s).map_err(|e| ProtocolError::DeserializationFailed(e.to_string()))?;

        if bytes.len() != MESSAGE_ID_SIZE {
            return Err(ProtocolError::InvalidMessageId);
        }

        let mut arr = [0u8; MESSAGE_ID_SIZE];
        arr.copy_from_slice(&bytes);
        Ok(MessageId(arr))
    }
}

impl std::fmt::Debug for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MessageId({}...)", &self.to_hex()[..12])
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.to_hex()[..12])
    }
}

/// Message type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessageType {
    /// Reserved (0x00)
    Reserved = 0x00,
    /// User data message (0x01)
    Data = 0x01,
    /// Protocol control message (0x02)
    Control = 0x02,
    /// DHT lookup request (0x03)
    DhtQuery = 0x03,
    /// DHT lookup response (0x04)
    DhtResponse = 0x04,
    /// Store value in DHT (0x05)
    DhtStore = 0x05,
    /// Node discovery announcement (0x06)
    Discovery = 0x06,
    /// Keep-alive message (0x07)
    Heartbeat = 0x07,
    /// Cryptographic key exchange (0x08)
    KeyExchange = 0x08,
    /// Blockchain block (0x09)
    LedgerBlock = 0x09,
    /// Query ledger (0x0A)
    LedgerQuery = 0x0A,
    /// Performance test request (0x0B)
    TestRequest = 0x0B,
    /// Performance test response (0x0C)
    TestResponse = 0x0C,
    /// Request route information (0x0D)
    RouteRequest = 0x0D,
    /// Provide route information (0x0E)
    RouteResponse = 0x0E,

    // Legacy compatibility (will be remapped)
    /// DHT FIND_NODE request (mapped to DhtQuery)
    FindNode = 0x10,
    /// DHT FIND_NODE response (mapped to DhtResponse)
    FindNodeResponse = 0x11,
    /// DHT STORE request (mapped to DhtStore)
    Store = 0x12,
    /// DHT STORE acknowledgment (mapped to DhtResponse)
    StoreAck = 0x13,
    /// DHT FIND_VALUE request (mapped to DhtQuery)
    FindValue = 0x14,
    /// DHT FIND_VALUE response (mapped to DhtResponse)
    FindValueResponse = 0x15,
    /// Ping message (mapped to Heartbeat)
    Ping = 0x20,
    /// Pong response (mapped to Heartbeat)
    Pong = 0x21,
    /// Key exchange response (mapped to KeyExchange)
    KeyExchangeResponse = 0x31,
    /// Message acknowledgment (mapped to Control)
    Ack = 0x40,
    /// Error message
    Error = 0xFF,
}

impl MessageType {
    /// Create from u8
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            0x00 => Ok(MessageType::Reserved),
            0x01 => Ok(MessageType::Data),
            0x02 => Ok(MessageType::Control),
            0x03 => Ok(MessageType::DhtQuery),
            0x04 => Ok(MessageType::DhtResponse),
            0x05 => Ok(MessageType::DhtStore),
            0x06 => Ok(MessageType::Discovery),
            0x07 => Ok(MessageType::Heartbeat),
            0x08 => Ok(MessageType::KeyExchange),
            0x09 => Ok(MessageType::LedgerBlock),
            0x0A => Ok(MessageType::LedgerQuery),
            0x0B => Ok(MessageType::TestRequest),
            0x0C => Ok(MessageType::TestResponse),
            0x0D => Ok(MessageType::RouteRequest),
            0x0E => Ok(MessageType::RouteResponse),
            // Legacy compatibility
            0x10 => Ok(MessageType::FindNode),
            0x11 => Ok(MessageType::FindNodeResponse),
            0x12 => Ok(MessageType::Store),
            0x13 => Ok(MessageType::StoreAck),
            0x14 => Ok(MessageType::FindValue),
            0x15 => Ok(MessageType::FindValueResponse),
            0x20 => Ok(MessageType::Ping),
            0x21 => Ok(MessageType::Pong),
            0x31 => Ok(MessageType::KeyExchangeResponse),
            0x40 => Ok(MessageType::Ack),
            0xFF => Ok(MessageType::Error),
            _ => Err(ProtocolError::InvalidMessageType(value)),
        }
    }

    /// Convert to u8
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

/// A message in the MyriadMesh protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier
    pub id: MessageId,

    /// Source node ID
    pub source: NodeId,

    /// Destination node ID
    pub destination: NodeId,

    /// Message type
    pub message_type: MessageType,

    /// Message priority
    pub priority: Priority,

    /// Time-to-live (number of hops remaining)
    pub ttl: u8,

    /// Timestamp (Unix time in milliseconds)
    pub timestamp: u64,

    /// Sequence number (for ordering)
    pub sequence: u32,

    /// Message payload
    pub payload: Vec<u8>,
}

impl Message {
    /// Create a new message
    pub fn new(
        source: NodeId,
        destination: NodeId,
        message_type: MessageType,
        payload: Vec<u8>,
    ) -> Result<Self> {
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::MessageTooLarge {
                size: payload.len(),
                max: MAX_PAYLOAD_SIZE,
            });
        }

        // Timestamp in milliseconds (per specification.md:143-146)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let sequence = 0; // Should be managed by higher layers
        let id = MessageId::generate(&source, &destination, &payload, timestamp, sequence);

        Ok(Message {
            id,
            source,
            destination,
            message_type,
            priority: Priority::default(),
            ttl: 32, // Default TTL (per specification.md:122)
            timestamp,
            sequence,
            payload,
        })
    }

    /// Set priority
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Set TTL
    pub fn with_ttl(mut self, ttl: u8) -> Self {
        self.ttl = ttl;
        self
    }

    /// Set sequence number
    pub fn with_sequence(mut self, sequence: u32) -> Self {
        self.sequence = sequence;
        self
    }

    /// Decrement TTL (returns false if TTL reaches 0)
    pub fn decrement_ttl(&mut self) -> bool {
        if self.ttl > 0 {
            self.ttl -= 1;
            true
        } else {
            false
        }
    }

    /// Validate message
    pub fn validate(&self) -> Result<()> {
        if self.payload.len() > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::MessageTooLarge {
                size: self.payload.len(),
                max: MAX_PAYLOAD_SIZE,
            });
        }

        if self.ttl == 0 {
            return Err(ProtocolError::TtlExceeded);
        }

        Ok(())
    }

    /// Get message size in bytes (approximate)
    pub fn size(&self) -> usize {
        MESSAGE_ID_SIZE
            + 32 // source
            + 32 // destination
            + 1  // message_type
            + 1  // priority
            + 1  // ttl
            + 8  // timestamp
            + 4  // sequence
            + self.payload.len()
    }

    /// Check if timestamp is fresh (within acceptable time window)
    /// Per specification.md:486-488: timestamp must be within ±5 minutes
    pub fn is_timestamp_fresh(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let age_ms = (now as i64 - self.timestamp as i64).abs();
        age_ms <= 300_000 // 5 minutes in milliseconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_id_size() {
        assert_eq!(MESSAGE_ID_SIZE, 16);
    }

    #[test]
    fn test_message_id_generation() {
        let source = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let payload = b"test payload";
        let timestamp = 1704067200000u64; // milliseconds
        let sequence = 42;

        let id1 = MessageId::generate(&source, &dest, payload, timestamp, sequence);
        let id2 = MessageId::generate(&source, &dest, payload, timestamp, sequence);

        // Same inputs should produce same ID
        assert_eq!(id1, id2);

        // Different inputs should produce different ID
        let id3 = MessageId::generate(&source, &dest, payload, timestamp, sequence + 1);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_message_creation() {
        let source = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let payload = b"Hello, MyriadMesh!".to_vec();

        let msg = Message::new(source, dest, MessageType::Data, payload).unwrap();

        assert_eq!(msg.source, source);
        assert_eq!(msg.destination, dest);
        assert_eq!(msg.message_type, MessageType::Data);
        assert_eq!(msg.priority, Priority::normal());
        assert_eq!(msg.ttl, 32);

        // Timestamp should be in milliseconds (13 digits for year 2024+)
        assert!(msg.timestamp > 1_000_000_000_000); // > 2001 in milliseconds
    }

    #[test]
    fn test_message_ttl() {
        let source = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let payload = b"test".to_vec();

        let mut msg = Message::new(source, dest, MessageType::Data, payload)
            .unwrap()
            .with_ttl(2);

        assert!(msg.decrement_ttl());
        assert_eq!(msg.ttl, 1);

        assert!(msg.decrement_ttl());
        assert_eq!(msg.ttl, 0);

        assert!(!msg.decrement_ttl());
        assert_eq!(msg.ttl, 0);
    }

    #[test]
    fn test_message_validation() {
        let source = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let payload = b"test".to_vec();

        let msg = Message::new(source, dest, MessageType::Data, payload).unwrap();
        assert!(msg.validate().is_ok());

        let mut msg_no_ttl = msg.clone();
        msg_no_ttl.ttl = 0;
        assert!(msg_no_ttl.validate().is_err());
    }

    #[test]
    fn test_message_too_large() {
        let source = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let payload = vec![0u8; MAX_PAYLOAD_SIZE + 1];

        let result = Message::new(source, dest, MessageType::Data, payload);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_id_hex() {
        let source = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let id = MessageId::generate(&source, &dest, b"test", 123, 0);

        let hex = id.to_hex();
        assert_eq!(hex.len(), 32); // 16 bytes = 32 hex chars
        let parsed = MessageId::from_hex(&hex).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_timestamp_freshness() {
        let source = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let payload = b"test".to_vec();

        // Fresh message
        let msg = Message::new(source, dest, MessageType::Data, payload.clone()).unwrap();
        assert!(msg.is_timestamp_fresh());

        // Old message (6 minutes ago)
        let old_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            - 360_000; // 6 minutes in ms

        let mut old_msg = Message::new(source, dest, MessageType::Data, payload).unwrap();
        old_msg.timestamp = old_timestamp;
        assert!(!old_msg.is_timestamp_fresh());
    }

    #[test]
    fn test_message_type_conversion() {
        assert_eq!(MessageType::from_u8(0x01).unwrap(), MessageType::Data);
        assert_eq!(MessageType::from_u8(0x07).unwrap(), MessageType::Heartbeat);
        assert_eq!(
            MessageType::from_u8(0x0B).unwrap(),
            MessageType::TestRequest
        );

        assert_eq!(MessageType::Data.to_u8(), 0x01);
        assert_eq!(MessageType::Heartbeat.to_u8(), 0x07);

        // Invalid type
        assert!(MessageType::from_u8(0x99).is_err());
    }
}
