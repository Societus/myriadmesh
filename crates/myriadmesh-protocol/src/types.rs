//! Core protocol types

use serde::{Deserialize, Serialize};
use std::fmt;

/// Size of a node ID in bytes (32 bytes / 256 bits)
pub const NODE_ID_SIZE: usize = 32;

/// A unique identifier for a node in the MyriadMesh network
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct NodeId([u8; NODE_ID_SIZE]);

impl NodeId {
    /// Create a NodeId from a byte array
    pub fn from_bytes(bytes: [u8; NODE_ID_SIZE]) -> Self {
        NodeId(bytes)
    }

    /// Get the bytes of this NodeId
    pub fn as_bytes(&self) -> &[u8; NODE_ID_SIZE] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    pub fn from_hex(s: &str) -> Result<Self, String> {
        let bytes = hex::decode(s).map_err(|e| e.to_string())?;

        if bytes.len() != NODE_ID_SIZE {
            return Err(format!(
                "Invalid NodeId length: expected {}, got {}",
                NODE_ID_SIZE,
                bytes.len()
            ));
        }

        let mut arr = [0u8; NODE_ID_SIZE];
        arr.copy_from_slice(&bytes);
        Ok(NodeId(arr))
    }

    /// Calculate XOR distance between two node IDs (for Kademlia DHT)
    pub fn distance(&self, other: &NodeId) -> [u8; NODE_ID_SIZE] {
        let mut result = [0u8; NODE_ID_SIZE];
        for (i, item) in result.iter_mut().enumerate() {
            *item = self.0[i] ^ other.0[i];
        }
        result
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self.to_hex())
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.to_hex()[..16])
    }
}

/// Priority level for message routing (0-255)
///
/// Per specification.md:109-116:
/// - 0-63: BACKGROUND
/// - 64-127: LOW
/// - 128-191: NORMAL
/// - 192-223: HIGH
/// - 224-255: EMERGENCY
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Priority(u8);

impl Priority {
    /// Background priority (0-63)
    pub const BACKGROUND_MIN: u8 = 0;
    pub const BACKGROUND_MAX: u8 = 63;

    /// Low priority (64-127)
    pub const LOW_MIN: u8 = 64;
    pub const LOW_MAX: u8 = 127;

    /// Normal priority (128-191)
    pub const NORMAL_MIN: u8 = 128;
    pub const NORMAL_MAX: u8 = 191;

    /// High priority (192-223)
    pub const HIGH_MIN: u8 = 192;
    pub const HIGH_MAX: u8 = 223;

    /// Emergency priority (224-255)
    pub const EMERGENCY_MIN: u8 = 224;
    pub const EMERGENCY_MAX: u8 = 255;

    /// Create priority from u8 (any value 0-255 is valid)
    pub fn from_u8(value: u8) -> Self {
        Priority(value)
    }

    /// Convert to u8
    pub fn as_u8(&self) -> u8 {
        self.0
    }

    /// Create background priority (default: 32)
    pub fn background() -> Self {
        Priority(32)
    }

    /// Create low priority (default: 96)
    pub fn low() -> Self {
        Priority(96)
    }

    /// Create normal priority (default: 160)
    pub fn normal() -> Self {
        Priority(160)
    }

    /// Create high priority (default: 208)
    pub fn high() -> Self {
        Priority(208)
    }

    /// Create emergency priority (default: 240)
    pub fn emergency() -> Self {
        Priority(240)
    }

    /// Check if priority is in background range
    pub fn is_background(&self) -> bool {
        self.0 >= Self::BACKGROUND_MIN && self.0 <= Self::BACKGROUND_MAX
    }

    /// Check if priority is in low range
    pub fn is_low(&self) -> bool {
        self.0 >= Self::LOW_MIN && self.0 <= Self::LOW_MAX
    }

    /// Check if priority is in normal range
    pub fn is_normal(&self) -> bool {
        self.0 >= Self::NORMAL_MIN && self.0 <= Self::NORMAL_MAX
    }

    /// Check if priority is in high range
    pub fn is_high(&self) -> bool {
        self.0 >= Self::HIGH_MIN && self.0 <= Self::HIGH_MAX
    }

    /// Check if priority is in emergency range
    pub fn is_emergency(&self) -> bool {
        self.0 >= Self::EMERGENCY_MIN && self.0 <= Self::EMERGENCY_MAX
    }

    /// Get human-readable priority level
    pub fn level_name(&self) -> &'static str {
        match self.0 {
            Self::BACKGROUND_MIN..=Self::BACKGROUND_MAX => "BACKGROUND",
            Self::LOW_MIN..=Self::LOW_MAX => "LOW",
            Self::NORMAL_MIN..=Self::NORMAL_MAX => "NORMAL",
            Self::HIGH_MIN..=Self::HIGH_MAX => "HIGH",
            Self::EMERGENCY_MIN..=Self::EMERGENCY_MAX => "EMERGENCY",
        }
    }
}

impl Default for Priority {
    fn default() -> Self {
        Priority::normal()
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.level_name(), self.0)
    }
}

/// Network adapter type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum AdapterType {
    /// Ethernet/IP network
    Ethernet = 0x01,
    /// Bluetooth Classic
    Bluetooth = 0x02,
    /// Bluetooth Low Energy
    BluetoothLE = 0x03,
    /// Cellular (4G/5G)
    Cellular = 0x04,
    /// Wi-Fi HaLoW (802.11ah)
    WiFiHaLoW = 0x05,
    /// LoRaWAN
    LoRaWAN = 0x06,
    /// Meshtastic
    Meshtastic = 0x07,
    /// FRS/GMRS Radio
    FRSGMRS = 0x08,
    /// CB Radio
    CBRadio = 0x09,
    /// Shortwave Radio
    Shortwave = 0x0A,
    /// Amateur Packet Radio (APRS)
    APRS = 0x0B,
    /// Dial-up/Modem
    Dialup = 0x0C,
    /// PPPoE
    PPPoE = 0x0D,
    /// i2p overlay network
    I2P = 0x0E,
    /// Unknown/Custom adapter
    Unknown = 0xFF,
}

impl AdapterType {
    /// Create adapter type from u8
    pub fn from_u8(value: u8) -> Self {
        match value {
            0x01 => AdapterType::Ethernet,
            0x02 => AdapterType::Bluetooth,
            0x03 => AdapterType::BluetoothLE,
            0x04 => AdapterType::Cellular,
            0x05 => AdapterType::WiFiHaLoW,
            0x06 => AdapterType::LoRaWAN,
            0x07 => AdapterType::Meshtastic,
            0x08 => AdapterType::FRSGMRS,
            0x09 => AdapterType::CBRadio,
            0x0A => AdapterType::Shortwave,
            0x0B => AdapterType::APRS,
            0x0C => AdapterType::Dialup,
            0x0D => AdapterType::PPPoE,
            0x0E => AdapterType::I2P,
            _ => AdapterType::Unknown,
        }
    }

    /// Convert to u8
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            AdapterType::Ethernet => "Ethernet/IP",
            AdapterType::Bluetooth => "Bluetooth Classic",
            AdapterType::BluetoothLE => "Bluetooth LE",
            AdapterType::Cellular => "Cellular",
            AdapterType::WiFiHaLoW => "Wi-Fi HaLoW",
            AdapterType::LoRaWAN => "LoRaWAN",
            AdapterType::Meshtastic => "Meshtastic",
            AdapterType::FRSGMRS => "FRS/GMRS",
            AdapterType::CBRadio => "CB Radio",
            AdapterType::Shortwave => "Shortwave",
            AdapterType::APRS => "Amateur Radio (APRS)",
            AdapterType::Dialup => "Dial-up",
            AdapterType::PPPoE => "PPPoE",
            AdapterType::I2P => "i2p",
            AdapterType::Unknown => "Unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_hex() {
        let bytes = [42u8; NODE_ID_SIZE];
        let node_id = NodeId::from_bytes(bytes);

        let hex = node_id.to_hex();
        let parsed = NodeId::from_hex(&hex).unwrap();

        assert_eq!(node_id, parsed);
    }

    #[test]
    fn test_node_id_distance() {
        let id1 = NodeId::from_bytes([0xFF; NODE_ID_SIZE]);
        let id2 = NodeId::from_bytes([0x00; NODE_ID_SIZE]);

        let distance = id1.distance(&id2);
        assert_eq!(distance, [0xFF; NODE_ID_SIZE]);

        let distance2 = id2.distance(&id1);
        assert_eq!(distance, distance2); // XOR is symmetric
    }

    #[test]
    fn test_priority_ranges() {
        let bg = Priority::background();
        assert!(bg.is_background());
        assert_eq!(bg.level_name(), "BACKGROUND");

        let low = Priority::low();
        assert!(low.is_low());
        assert_eq!(low.level_name(), "LOW");

        let normal = Priority::normal();
        assert!(normal.is_normal());
        assert_eq!(normal.level_name(), "NORMAL");

        let high = Priority::high();
        assert!(high.is_high());
        assert_eq!(high.level_name(), "HIGH");

        let emergency = Priority::emergency();
        assert!(emergency.is_emergency());
        assert_eq!(emergency.level_name(), "EMERGENCY");
    }

    #[test]
    fn test_priority_conversion() {
        assert_eq!(Priority::from_u8(0).as_u8(), 0);
        assert_eq!(Priority::from_u8(128).as_u8(), 128);
        assert_eq!(Priority::from_u8(255).as_u8(), 255);

        assert_eq!(Priority::background().as_u8(), 32);
        assert_eq!(Priority::normal().as_u8(), 160);
    }

    #[test]
    fn test_priority_ordering() {
        let bg = Priority::background();
        let low = Priority::low();
        let normal = Priority::normal();
        let high = Priority::high();
        let emergency = Priority::emergency();

        assert!(bg < low);
        assert!(low < normal);
        assert!(normal < high);
        assert!(high < emergency);
    }

    #[test]
    fn test_priority_default() {
        let default = Priority::default();
        assert!(default.is_normal());
        assert_eq!(default, Priority::normal());
    }

    #[test]
    fn test_priority_boundary_cases() {
        // Test boundaries
        assert!(Priority::from_u8(0).is_background());
        assert!(Priority::from_u8(63).is_background());
        assert!(Priority::from_u8(64).is_low());
        assert!(Priority::from_u8(127).is_low());
        assert!(Priority::from_u8(128).is_normal());
        assert!(Priority::from_u8(191).is_normal());
        assert!(Priority::from_u8(192).is_high());
        assert!(Priority::from_u8(223).is_high());
        assert!(Priority::from_u8(224).is_emergency());
        assert!(Priority::from_u8(255).is_emergency());
    }

    #[test]
    fn test_adapter_type_conversion() {
        assert_eq!(AdapterType::from_u8(0x01), AdapterType::Ethernet);
        assert_eq!(AdapterType::from_u8(0x0E), AdapterType::I2P);
        assert_eq!(AdapterType::from_u8(0xFF), AdapterType::Unknown);

        assert_eq!(AdapterType::Ethernet.to_u8(), 0x01);
    }

    #[test]
    fn test_adapter_type_name() {
        assert_eq!(AdapterType::Ethernet.name(), "Ethernet/IP");
        assert_eq!(AdapterType::APRS.name(), "Amateur Radio (APRS)");
    }
}
