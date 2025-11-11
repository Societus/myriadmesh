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

/// Priority level for message routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum Priority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
    Urgent = 3,
}

impl Priority {
    /// Create priority from u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Priority::Low),
            1 => Some(Priority::Normal),
            2 => Some(Priority::High),
            3 => Some(Priority::Urgent),
            _ => None,
        }
    }

    /// Convert to u8
    pub fn to_u8(self) -> u8 {
        self as u8
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
    fn test_priority_conversion() {
        assert_eq!(Priority::from_u8(0), Some(Priority::Low));
        assert_eq!(Priority::from_u8(1), Some(Priority::Normal));
        assert_eq!(Priority::from_u8(2), Some(Priority::High));
        assert_eq!(Priority::from_u8(3), Some(Priority::Urgent));
        assert_eq!(Priority::from_u8(4), None);

        assert_eq!(Priority::Low.to_u8(), 0);
        assert_eq!(Priority::Normal.to_u8(), 1);
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
