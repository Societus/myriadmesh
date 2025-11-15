//! Network adapter implementations

pub mod bluetooth;
pub mod bluetooth_le;
pub mod cellular;
pub mod ethernet;

// Phase 5: Specialized Adapters
pub mod aprs;
pub mod dialup;
pub mod frsgmrs;
pub mod hf_radio;
pub mod lora;
pub mod wifi_halow;

pub use bluetooth::{BluetoothAdapter, BluetoothConfig};
pub use bluetooth_le::{BleAdapter, BleConfig};
pub use cellular::{CellularAdapter, CellularConfig, NetworkType};
pub use ethernet::{EthernetAdapter, EthernetConfig};

// Phase 5 exports
pub use aprs::{AprsAdapter, AprsConfig};
pub use dialup::{DialupAdapter, DialupConfig, ModemType};
pub use frsgmrs::{FrsGmrsAdapter, FrsGmrsConfig, ModulationType};
pub use hf_radio::{DigitalMode, HfRadioAdapter, HfRadioConfig};
pub use lora::{LoRaAdapter, LoRaConfig};
pub use wifi_halow::{WifiHalowAdapter, WifiHalowConfig};
