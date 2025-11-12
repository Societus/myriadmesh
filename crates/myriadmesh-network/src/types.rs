//! Network type definitions

use myriadmesh_protocol::types::AdapterType;
use serde::{Deserialize, Serialize};

/// Network address (transport-specific)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Address {
    /// Ethernet/IP address (e.g., "192.168.1.1:4001")
    Ethernet(String),

    /// Bluetooth MAC address
    Bluetooth(String),

    /// Cellular phone number or IP
    Cellular(String),

    /// LoRaWAN device address
    LoRaWAN(String),

    /// Radio callsign
    Radio(String),

    /// i2p destination
    I2P(String),

    /// Unknown/custom address
    Unknown(String),
}

impl Address {
    /// Get address as string
    pub fn as_str(&self) -> &str {
        match self {
            Address::Ethernet(s) => s,
            Address::Bluetooth(s) => s,
            Address::Cellular(s) => s,
            Address::LoRaWAN(s) => s,
            Address::Radio(s) => s,
            Address::I2P(s) => s,
            Address::Unknown(s) => s,
        }
    }

    /// Get adapter type for this address
    pub fn adapter_type(&self) -> AdapterType {
        match self {
            Address::Ethernet(_) => AdapterType::Ethernet,
            Address::Bluetooth(_) => AdapterType::Bluetooth,
            Address::Cellular(_) => AdapterType::Cellular,
            Address::LoRaWAN(_) => AdapterType::LoRaWAN,
            Address::Radio(_) => AdapterType::APRS,
            Address::I2P(_) => AdapterType::I2P,
            Address::Unknown(_) => AdapterType::Unknown,
        }
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Power consumption level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerConsumption {
    /// No power consumption (mains powered)
    None,
    /// Very low (<1mW)
    VeryLow,
    /// Low (1-10mW)
    Low,
    /// Medium (10-100mW)
    Medium,
    /// High (100mW-1W)
    High,
    /// Very high (>1W)
    VeryHigh,
}

/// Adapter capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterCapabilities {
    /// Adapter type
    pub adapter_type: AdapterType,

    /// Maximum message size (bytes)
    pub max_message_size: usize,

    /// Typical latency (milliseconds)
    pub typical_latency_ms: f64,

    /// Typical bandwidth (bits per second)
    pub typical_bandwidth_bps: u64,

    /// Reliability (0.0 - 1.0)
    pub reliability: f64,

    /// Range in meters (0 = global)
    pub range_meters: f64,

    /// Power consumption level
    pub power_consumption: PowerConsumption,

    /// Cost per megabyte (USD)
    pub cost_per_mb: f64,

    /// Supports broadcast
    pub supports_broadcast: bool,

    /// Supports multicast
    pub supports_multicast: bool,
}

impl AdapterCapabilities {
    /// Calculate adapter score for message routing
    ///
    /// Higher score = better adapter for this use case
    pub fn calculate_score(&self, _message_size: usize, priority: u8) -> f64 {
        // Weight factors based on message priority
        match priority {
            224..=255 => {
                // EMERGENCY: prioritize reliability and availability
                self.reliability * 0.6 + (1.0 - self.latency_score()) * 0.3 + self.availability_score() * 0.1
            }
            192..=223 => {
                // HIGH: balance latency, reliability, and bandwidth
                (1.0 - self.latency_score()) * 0.4
                    + self.reliability * 0.3
                    + self.bandwidth_score() * 0.2
                    + (1.0 - self.cost_score()) * 0.1
            }
            _ => {
                // NORMAL, LOW, BACKGROUND: optimize for cost and bandwidth
                (1.0 - self.latency_score()) * 0.25
                    + self.reliability * 0.25
                    + self.bandwidth_score() * 0.2
                    + (1.0 - self.cost_score()) * 0.2
                    + self.availability_score() * 0.1
            }
        }
    }

    fn latency_score(&self) -> f64 {
        // Normalize latency (lower is better)
        // 0-10ms = 0.0, 1000ms+ = 1.0
        (self.typical_latency_ms / 1000.0).min(1.0)
    }

    fn bandwidth_score(&self) -> f64 {
        // Normalize bandwidth (higher is better)
        // 1Mbps = 0.5, 100Mbps+ = 1.0
        (self.typical_bandwidth_bps as f64 / 100_000_000.0).min(1.0)
    }

    fn cost_score(&self) -> f64 {
        // Normalize cost (lower is better)
        // $0 = 0.0, $1/MB+ = 1.0
        self.cost_per_mb.min(1.0)
    }

    fn availability_score(&self) -> f64 {
        // Simple availability heuristic based on range
        if self.range_meters == 0.0 {
            1.0 // Global
        } else if self.range_meters > 10_000.0 {
            0.9 // Long range
        } else if self.range_meters > 1_000.0 {
            0.7 // Medium range
        } else {
            0.5 // Short range
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_display() {
        let addr = Address::Ethernet("192.168.1.1:4001".to_string());
        assert_eq!(addr.to_string(), "192.168.1.1:4001");
    }

    #[test]
    fn test_address_adapter_type() {
        let addr = Address::Ethernet("test".to_string());
        assert_eq!(addr.adapter_type(), AdapterType::Ethernet);

        let addr = Address::I2P("test.i2p".to_string());
        assert_eq!(addr.adapter_type(), AdapterType::I2P);
    }

    #[test]
    fn test_adapter_score_emergency() {
        let caps = AdapterCapabilities {
            adapter_type: AdapterType::Ethernet,
            max_message_size: 1400,
            typical_latency_ms: 5.0,
            typical_bandwidth_bps: 100_000_000,
            reliability: 0.99,
            range_meters: 100.0,
            power_consumption: PowerConsumption::None,
            cost_per_mb: 0.0,
            supports_broadcast: true,
            supports_multicast: true,
        };

        let score = caps.calculate_score(100, 255); // Emergency priority
        assert!(score > 0.5); // Should have high score for reliable adapter
    }

    #[test]
    fn test_power_consumption_levels() {
        assert_eq!(PowerConsumption::None as u8, 0);
        assert!(matches!(PowerConsumption::VeryLow, PowerConsumption::VeryLow));
    }
}
