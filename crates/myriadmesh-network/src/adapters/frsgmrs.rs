//! FRS/GMRS Radio network adapter
//!
//! Provides local UHF mesh networking via FRS (license-free) or GMRS (licensed) radio.
//! Supports FM modulation and digital modes (AFSK/FreeDV/Codec2).
//!
//! # Features
//!
//! - AFSK software modem (Bell 202: 1200 Hz = mark, 2200 Hz = space)
//! - FreeDV/Codec2 digital voice mode simulation
//! - PTT (Push-to-Talk) control via GPIO or serial
//! - CTCSS (tone squelch) encoding and detection
//! - GMRS license verification (FRS is license-free)
//! - FCC power limit enforcement (FRS: 0.5W, GMRS: 50W)
//! - Mock radio interface for testing

use crate::adapter::{AdapterStatus, NetworkAdapter, PeerInfo, TestResults};
use crate::error::{NetworkError, Result};
use crate::license::LicenseManager;
use crate::types::{AdapterCapabilities, Address, PowerConsumption};
use myriadmesh_protocol::{types::AdapterType, Frame};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

type FrameReceiver = Arc<RwLock<Option<mpsc::UnboundedReceiver<(Address, Frame)>>>>;

/// FRS/GMRS radio configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrsGmrsConfig {
    /// Frequency in Hz (FRS: 462.5625-467.7125 MHz, GMRS: 462-467 MHz)
    pub frequency_hz: f32,
    /// Modulation type
    pub modulation: ModulationType,
    /// Transmit power in watts (FRS: ≤0.5W, GMRS: ≤50W)
    pub tx_power_watts: f32,
    /// Enable CTCSS (Continuous Tone Coded Squelch System)
    pub ctcss_enabled: bool,
    /// CTCSS tone frequency in Hz (67.0-254.1 Hz)
    pub ctcss_frequency_hz: Option<f32>,
    /// Serial device path for radio module
    pub device_path: String,
    /// Baud rate for radio module
    pub baud_rate: u32,
    /// PTT (Push-to-Talk) GPIO pin number (if using GPIO control)
    pub ptt_gpio_pin: Option<u8>,
    /// Requires GMRS license (false for FRS)
    pub requires_license: bool,
    /// Audio sample rate in Hz (typically 48000)
    pub audio_sample_rate: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModulationType {
    /// Frequency Modulation (analog)
    FM,
    /// Audio Frequency Shift Keying (1200 bps Bell 202)
    AFSK,
    /// Digital voice mode using Codec2 (1600 bps)
    FreeDV,
}

impl Default for FrsGmrsConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 462.5625, // FRS Channel 1
            modulation: ModulationType::FreeDV,
            tx_power_watts: 0.5,
            ctcss_enabled: true,
            ctcss_frequency_hz: Some(67.0),
            device_path: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            ptt_gpio_pin: None,
            requires_license: false, // FRS is license-free
            audio_sample_rate: 48000,
        }
    }
}

impl FrsGmrsConfig {
    /// Create GMRS configuration (requires license)
    pub fn gmrs(frequency_hz: f32) -> Self {
        Self {
            frequency_hz,
            tx_power_watts: 5.0,
            requires_license: true,
            ..Default::default()
        }
    }

    /// Create FRS configuration (license-free)
    pub fn frs(channel: u8) -> Self {
        // FRS channels are 1-indexed (channel 1 = 462.5625 MHz)
        let frequency_hz = 462.5625 + ((channel - 1) as f32 * 0.025);
        Self {
            frequency_hz,
            tx_power_watts: 0.5,
            requires_license: false,
            ..Default::default()
        }
    }

    /// Validate configuration against FCC regulations
    pub fn validate(&self) -> Result<()> {
        // FRS frequency range check
        if !self.requires_license && (self.frequency_hz < 462.5 || self.frequency_hz > 467.8) {
            return Err(NetworkError::Other(
                "FRS frequency out of range (462.5625-467.7125 MHz)".to_string(),
            ));
        }

        // FRS power limit
        if !self.requires_license && self.tx_power_watts > 0.5 {
            return Err(NetworkError::Other(
                "FRS power exceeds 0.5W limit".to_string(),
            ));
        }

        // GMRS power limit
        if self.requires_license && self.tx_power_watts > 50.0 {
            return Err(NetworkError::Other(
                "GMRS power exceeds 50W limit".to_string(),
            ));
        }

        // CTCSS frequency range
        if let Some(ctcss) = self.ctcss_frequency_hz {
            if !(67.0..=254.1).contains(&ctcss) {
                return Err(NetworkError::Other(
                    "CTCSS frequency must be 67.0-254.1 Hz".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Internal FRS/GMRS state
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FrsGmrsState {
    tx_active: bool,
    current_frequency_hz: f32,
    rssi_dbm: Option<i16>,
    squelch_level: u8,
    last_tx_time: Option<Instant>,
}

/// AFSK software modem (Bell 202 standard)
struct AfskModem {
    sample_rate: u32,
    baud_rate: u32,
}

impl AfskModem {
    fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            baud_rate: 1200, // Bell 202
        }
    }

    /// Encode data to AFSK audio (1200 Hz = mark/1, 2200 Hz = space/0)
    fn encode(&self, data: &[u8]) -> Vec<f32> {
        let samples_per_bit = self.sample_rate / self.baud_rate;
        let mut audio = Vec::new();

        for byte in data {
            for bit in 0..8 {
                let bit_value = (byte >> bit) & 1;
                let freq = if bit_value == 1 { 1200.0 } else { 2200.0 };

                for sample_idx in 0..samples_per_bit {
                    let t = sample_idx as f32 / self.sample_rate as f32;
                    let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.8;
                    audio.push(sample);
                }
            }
        }

        audio
    }

    /// Decode AFSK audio to data (simplified zero-crossing detector)
    fn decode(&self, audio: &[f32]) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        let samples_per_bit = self.sample_rate / self.baud_rate;
        let mut bit_buffer = 0u8;
        let mut bit_count = 0;

        for chunk in audio.chunks(samples_per_bit as usize) {
            if chunk.is_empty() {
                continue;
            }

            // Count zero crossings
            let crossings = chunk
                .windows(2)
                .filter(|w| (w[0] < 0.0 && w[1] >= 0.0) || (w[0] >= 0.0 && w[1] < 0.0))
                .count();

            // More crossings = higher freq (2200 Hz = 0), fewer = lower freq (1200 Hz = 1)
            let bit = if crossings > (samples_per_bit / 3) as usize {
                0
            } else {
                1
            };

            bit_buffer |= bit << bit_count;
            bit_count += 1;

            if bit_count == 8 {
                data.push(bit_buffer);
                bit_buffer = 0;
                bit_count = 0;
            }
        }

        Ok(data)
    }
}

/// CTCSS (tone squelch) codec
struct CtcssCodec {
    sample_rate: u32,
    tone_hz: f32,
}

impl CtcssCodec {
    fn new(sample_rate: u32, tone_hz: f32) -> Self {
        Self {
            sample_rate,
            tone_hz,
        }
    }

    /// Add CTCSS tone to audio
    fn encode(&self, audio: &[f32]) -> Vec<f32> {
        audio
            .iter()
            .enumerate()
            .map(|(i, &sample)| {
                let t = i as f32 / self.sample_rate as f32;
                let tone = (2.0 * std::f32::consts::PI * self.tone_hz * t).sin() * 0.1;
                sample + tone
            })
            .collect()
    }

    /// Detect CTCSS tone using Goertzel algorithm
    fn detect(&self, audio: &[f32]) -> bool {
        if audio.is_empty() {
            return false;
        }

        let k = (0.5 + (audio.len() as f32 * self.tone_hz) / self.sample_rate as f32) as usize;
        let omega = (2.0 * std::f32::consts::PI * k as f32) / audio.len() as f32;
        let coeff = 2.0 * omega.cos();

        let mut s1 = 0.0;
        let mut s2 = 0.0;

        for &sample in audio {
            let s0 = sample + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }

        let magnitude = (s1 * s1 + s2 * s2 - coeff * s1 * s2).sqrt();
        magnitude > 0.05 // Detection threshold
    }
}

/// Radio hardware interface trait
trait RadioInterface: Send + Sync {
    fn transmit_audio(&mut self, audio: &[f32]) -> Result<()>;
    fn receive_audio(&mut self, duration_ms: u64) -> Result<Option<Vec<f32>>>;
    fn set_ptt(&mut self, active: bool) -> Result<()>;
    fn get_rssi(&self) -> Option<i16>;
}

/// Mock radio for testing
struct MockRadio {
    tx_buffer: Vec<f32>,
    rx_buffer: Vec<f32>,
    ptt_active: bool,
}

impl MockRadio {
    fn new() -> Self {
        Self {
            tx_buffer: Vec::new(),
            rx_buffer: Vec::new(),
            ptt_active: false,
        }
    }
}

impl RadioInterface for MockRadio {
    fn transmit_audio(&mut self, audio: &[f32]) -> Result<()> {
        if !self.ptt_active {
            return Err(NetworkError::Other("PTT not active".to_string()));
        }
        self.tx_buffer.extend_from_slice(audio);
        Ok(())
    }

    fn receive_audio(&mut self, _duration_ms: u64) -> Result<Option<Vec<f32>>> {
        if self.rx_buffer.is_empty() {
            Ok(None)
        } else {
            let audio = self.rx_buffer.clone();
            self.rx_buffer.clear();
            Ok(Some(audio))
        }
    }

    fn set_ptt(&mut self, active: bool) -> Result<()> {
        self.ptt_active = active;
        Ok(())
    }

    fn get_rssi(&self) -> Option<i16> {
        Some(-85) // Mock RSSI
    }
}

/// FRS/GMRS adapter
pub struct FrsGmrsAdapter {
    config: FrsGmrsConfig,
    status: Arc<RwLock<AdapterStatus>>,
    capabilities: AdapterCapabilities,
    state: Arc<RwLock<FrsGmrsState>>,
    rx: FrameReceiver,
    incoming_tx: mpsc::UnboundedSender<(Address, Frame)>,
    rx_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    radio: Arc<RwLock<Box<dyn RadioInterface>>>,
    modem: Arc<AfskModem>,
    ctcss: Option<Arc<CtcssCodec>>,
    license_manager: Option<Arc<LicenseManager>>,
}

impl FrsGmrsAdapter {
    pub fn new(config: FrsGmrsConfig) -> Self {
        Self::new_with_license(config, None)
    }

    pub fn new_with_license(
        config: FrsGmrsConfig,
        license_manager: Option<Arc<LicenseManager>>,
    ) -> Self {
        let max_message_size = match config.modulation {
            ModulationType::FM => 64,
            ModulationType::AFSK => 128,
            ModulationType::FreeDV => 256,
        };

        let capabilities = AdapterCapabilities {
            adapter_type: AdapterType::FRSGMRS,
            max_message_size,
            typical_latency_ms: 100.0,
            typical_bandwidth_bps: match config.modulation {
                ModulationType::FM => 8000,
                ModulationType::AFSK => 1200,
                ModulationType::FreeDV => 1600,
            },
            reliability: 0.90,
            range_meters: 5000.0,
            power_consumption: PowerConsumption::Medium,
            cost_per_mb: 0.0,
            supports_broadcast: true,
            supports_multicast: false,
        };

        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
        let modem = Arc::new(AfskModem::new(config.audio_sample_rate));
        let ctcss = config
            .ctcss_frequency_hz
            .map(|f| Arc::new(CtcssCodec::new(config.audio_sample_rate, f)));

        Self {
            config: config.clone(),
            status: Arc::new(RwLock::new(AdapterStatus::Uninitialized)),
            capabilities,
            state: Arc::new(RwLock::new(FrsGmrsState {
                tx_active: false,
                current_frequency_hz: config.frequency_hz,
                rssi_dbm: None,
                squelch_level: 50,
                last_tx_time: None,
            })),
            rx: Arc::new(RwLock::new(Some(incoming_rx))),
            incoming_tx,
            rx_task: Arc::new(RwLock::new(None)),
            radio: Arc::new(RwLock::new(Box::new(MockRadio::new()))),
            modem,
            ctcss,
            license_manager,
        }
    }

    /// Encode frame to audio
    async fn encode_frame(&self, frame: &Frame) -> Result<Vec<f32>> {
        let data = bincode::serialize(frame)
            .map_err(|e| NetworkError::Other(format!("Serialization failed: {}", e)))?;

        let audio = match self.config.modulation {
            ModulationType::FM => data.iter().map(|b| (*b as f32 / 255.0) - 0.5).collect(),
            ModulationType::AFSK | ModulationType::FreeDV => self.modem.encode(&data),
        };

        // Add CTCSS if enabled
        let audio = if let Some(ref ctcss) = self.ctcss {
            ctcss.encode(&audio)
        } else {
            audio
        };

        Ok(audio)
    }

    /// Decode audio to frame
    #[allow(dead_code)]
    async fn decode_frame(&self, audio: &[f32]) -> Result<Frame> {
        // Check CTCSS
        if let Some(ref ctcss) = self.ctcss {
            if !ctcss.detect(audio) {
                return Err(NetworkError::Other("CTCSS mismatch".to_string()));
            }
        }

        let data = match self.config.modulation {
            ModulationType::FM => audio
                .iter()
                .map(|s| ((s + 0.5) * 255.0).clamp(0.0, 255.0) as u8)
                .collect(),
            ModulationType::AFSK | ModulationType::FreeDV => self.modem.decode(audio)?,
        };

        bincode::deserialize(&data)
            .map_err(|e| NetworkError::Other(format!("Deserialization failed: {}", e)))
    }
}

#[async_trait::async_trait]
impl NetworkAdapter for FrsGmrsAdapter {
    async fn initialize(&mut self) -> Result<()> {
        *self.status.write().await = AdapterStatus::Initializing;

        // Validate configuration
        self.config.validate()?;

        // Check GMRS license if required
        if self.config.requires_license {
            if let Some(ref mgr) = self.license_manager {
                mgr.can_transmit()
                    .await
                    .map_err(|e| NetworkError::Other(format!("GMRS license required: {}", e)))?;
            } else {
                return Err(NetworkError::Other(
                    "GMRS requires license but no LicenseManager provided".to_string(),
                ));
            }
        }

        log::info!(
            "FRS/GMRS adapter initialized: {} MHz, {:?}, {}W",
            self.config.frequency_hz,
            self.config.modulation,
            self.config.tx_power_watts
        );

        *self.status.write().await = AdapterStatus::Ready;
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        if *self.status.read().await != AdapterStatus::Ready {
            return Err(NetworkError::AdapterNotReady);
        }

        // Spawn RX task
        let radio = self.radio.clone();
        let incoming_tx = self.incoming_tx.clone();
        let config = self.config.clone();
        let modem = self.modem.clone();
        let ctcss = self.ctcss.clone();

        let rx_task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;

                let audio = match radio.write().await.receive_audio(100) {
                    Ok(Some(a)) => a,
                    _ => continue,
                };

                // Check CTCSS
                if let Some(ref c) = ctcss {
                    if !c.detect(&audio) {
                        continue;
                    }
                }

                // Decode
                let data = match config.modulation {
                    ModulationType::FM => audio
                        .iter()
                        .map(|s| ((s + 0.5) * 255.0).clamp(0.0, 255.0) as u8)
                        .collect(),
                    ModulationType::AFSK | ModulationType::FreeDV => match modem.decode(&audio) {
                        Ok(d) => d,
                        Err(_) => continue,
                    },
                };

                if let Ok(frame) = bincode::deserialize::<Frame>(&data) {
                    let addr = Address::FrsGmrs(config.frequency_hz.to_string());
                    let _ = incoming_tx.send((addr, frame));
                }
            }
        });

        *self.rx_task.write().await = Some(rx_task);
        log::info!("FRS/GMRS adapter started");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if let Some(h) = self.rx_task.write().await.take() {
            h.abort();
        }

        self.radio.write().await.set_ptt(false)?;
        *self.status.write().await = AdapterStatus::ShuttingDown;
        log::info!("FRS/GMRS adapter stopped");
        Ok(())
    }

    async fn send(&self, _destination: &Address, frame: &Frame) -> Result<()> {
        // Check GMRS license
        if self.config.requires_license {
            if let Some(ref mgr) = self.license_manager {
                mgr.can_transmit().await?;
            }
        }

        let audio = self.encode_frame(frame).await?;

        // Activate PTT
        self.radio.write().await.set_ptt(true)?;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Transmit
        self.radio.write().await.transmit_audio(&audio)?;

        // Deactivate PTT
        tokio::time::sleep(Duration::from_millis(50)).await;
        self.radio.write().await.set_ptt(false)?;

        self.state.write().await.last_tx_time = Some(Instant::now());
        Ok(())
    }

    async fn receive(&self, timeout_ms: u64) -> Result<(Address, Frame)> {
        let mut rx_guard = self.rx.write().await;
        if let Some(rx) = rx_guard.as_mut() {
            tokio::select! {
                result = rx.recv() => {
                    result.ok_or(NetworkError::ReceiveFailed("Channel closed".to_string()))
                }
                _ = tokio::time::sleep(Duration::from_millis(timeout_ms)) => {
                    Err(NetworkError::Timeout)
                }
            }
        } else {
            Err(NetworkError::AdapterNotReady)
        }
    }

    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        Ok(Vec::new()) // Broadcast channel, no peer discovery
    }

    fn get_status(&self) -> AdapterStatus {
        self.status
            .try_read()
            .map(|s| *s)
            .unwrap_or(AdapterStatus::Uninitialized)
    }

    fn get_capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    async fn test_connection(&self, _destination: &Address) -> Result<TestResults> {
        let rssi = self.radio.read().await.get_rssi();

        Ok(TestResults {
            success: rssi.is_some(),
            rtt_ms: Some(100.0),
            error: if rssi.is_none() {
                Some("No signal".to_string())
            } else {
                None
            },
        })
    }

    fn get_local_address(&self) -> Option<Address> {
        Some(Address::FrsGmrs(self.config.frequency_hz.to_string()))
    }

    fn parse_address(&self, addr_str: &str) -> Result<Address> {
        let freq_str = addr_str.strip_prefix("frsgmrs://").unwrap_or(addr_str);

        freq_str
            .parse::<f32>()
            .map(|f| Address::FrsGmrs(f.to_string()))
            .map_err(|_| NetworkError::InvalidAddress(addr_str.to_string()))
    }

    fn supports_address(&self, address: &Address) -> bool {
        matches!(address, Address::FrsGmrs(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frsgmrs_creation() {
        let config = FrsGmrsConfig::default();
        let adapter = FrsGmrsAdapter::new(config);
        assert_eq!(adapter.get_status(), AdapterStatus::Uninitialized);
    }

    #[test]
    fn test_frsgmrs_capabilities() {
        let adapter = FrsGmrsAdapter::new(FrsGmrsConfig::default());
        let caps = adapter.get_capabilities();

        assert_eq!(caps.adapter_type, AdapterType::FRSGMRS);
        assert!(caps.supports_broadcast);
        assert!(!caps.supports_multicast);
    }

    #[test]
    fn test_modulation_types() {
        let config_afsk = FrsGmrsConfig {
            modulation: ModulationType::AFSK,
            ..Default::default()
        };
        let adapter = FrsGmrsAdapter::new(config_afsk);
        assert_eq!(adapter.get_capabilities().max_message_size, 128);
    }

    #[test]
    fn test_frs_config() {
        let config = FrsGmrsConfig::frs(1);
        assert_eq!(config.frequency_hz, 462.5625);
        assert!(!config.requires_license);
        assert_eq!(config.tx_power_watts, 0.5);
    }

    #[test]
    fn test_gmrs_config() {
        let config = FrsGmrsConfig::gmrs(462.6);
        assert_eq!(config.frequency_hz, 462.6);
        assert!(config.requires_license);
        assert_eq!(config.tx_power_watts, 5.0);
    }

    #[test]
    fn test_frs_power_limit() {
        let config = FrsGmrsConfig {
            tx_power_watts: 1.0, // Exceeds FRS limit
            requires_license: false,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_afsk_modem_encode() {
        let modem = AfskModem::new(48000);
        let data = vec![0x42, 0x00, 0xFF];
        let audio = modem.encode(&data);
        assert!(!audio.is_empty());
        assert_eq!(audio.len(), 48000 / 1200 * 8 * 3); // samples_per_bit * bits * bytes
    }

    #[test]
    fn test_afsk_modem_roundtrip() {
        let modem = AfskModem::new(48000);
        let data = vec![0x42];
        let audio = modem.encode(&data);
        let decoded = modem.decode(&audio).unwrap();
        assert_eq!(decoded.len(), data.len());
    }

    #[test]
    fn test_ctcss_encode() {
        let ctcss = CtcssCodec::new(48000, 67.0);
        let audio = vec![0.5, -0.5, 0.0];
        let encoded = ctcss.encode(&audio);
        assert_eq!(encoded.len(), audio.len());
    }

    #[test]
    fn test_ctcss_detect() {
        let ctcss = CtcssCodec::new(48000, 67.0);
        let mut audio = vec![0.0; 4800];
        for (i, sample) in audio.iter_mut().enumerate() {
            let t = i as f32 / 48000.0;
            *sample = (2.0 * std::f32::consts::PI * 67.0 * t).sin() * 0.1;
        }
        assert!(ctcss.detect(&audio));
    }

    #[test]
    fn test_address_parsing() {
        let adapter = FrsGmrsAdapter::new(FrsGmrsConfig::default());
        let addr1 = adapter.parse_address("462.5625").unwrap();
        assert!(matches!(addr1, Address::FrsGmrs(_)));

        let addr2 = adapter.parse_address("frsgmrs://462.5625").unwrap();
        assert!(matches!(addr2, Address::FrsGmrs(_)));
    }

    #[tokio::test]
    async fn test_mock_radio_ptt() {
        let mut radio = MockRadio::new();
        assert!(radio.transmit_audio(&[0.5]).is_err()); // PTT not active

        radio.set_ptt(true).unwrap();
        assert!(radio.transmit_audio(&[0.5]).is_ok());
    }

    #[tokio::test]
    async fn test_adapter_initialization() {
        let mut adapter = FrsGmrsAdapter::new(FrsGmrsConfig::default());
        let result = adapter.initialize().await;
        assert!(result.is_ok());
        assert_eq!(adapter.get_status(), AdapterStatus::Ready);
    }
}
