//! Privacy Protection Layers
//!
//! Implements privacy-enhancing techniques for i2p communications:
//! - Message padding (prevent traffic analysis)
//! - Timing obfuscation (prevent timing correlation)
//! - Cover traffic generation (prevent traffic pattern analysis)

use rand::Rng;
use std::time::Duration;

/// Default minimum message size (bytes) to prevent size-based traffic analysis
pub const MIN_MESSAGE_SIZE: usize = 512;

/// Maximum padding size (bytes)
pub const MAX_PADDING_SIZE: usize = 1024;

/// Message padding strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaddingStrategy {
    /// No padding
    None,

    /// Pad to minimum size
    MinSize,

    /// Pad to fixed size buckets (512, 1024, 2048, etc.)
    FixedBuckets,

    /// Random padding within range
    Random,
}

/// Timing obfuscation strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingStrategy {
    /// No timing obfuscation
    None,

    /// Fixed delay
    FixedDelay,

    /// Random delay within range
    RandomDelay,

    /// Exponential distribution delay (more realistic)
    ExponentialDelay,
}

/// Privacy configuration
#[derive(Debug, Clone)]
pub struct PrivacyConfig {
    /// Message padding strategy
    pub padding_strategy: PaddingStrategy,

    /// Minimum message size for padding
    pub min_message_size: usize,

    /// Maximum padding size
    pub max_padding_size: usize,

    /// Timing obfuscation strategy
    pub timing_strategy: TimingStrategy,

    /// Base delay in milliseconds
    pub base_delay_ms: u64,

    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,

    /// Enable cover traffic generation
    pub enable_cover_traffic: bool,

    /// Cover traffic rate (messages per hour)
    pub cover_traffic_rate: u32,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        PrivacyConfig {
            padding_strategy: PaddingStrategy::FixedBuckets,
            min_message_size: MIN_MESSAGE_SIZE,
            max_padding_size: MAX_PADDING_SIZE,
            timing_strategy: TimingStrategy::RandomDelay,
            base_delay_ms: 50,
            max_delay_ms: 500,
            enable_cover_traffic: false,
            cover_traffic_rate: 10,
        }
    }
}

/// Privacy protection layer
pub struct PrivacyLayer {
    config: PrivacyConfig,
}

impl PrivacyLayer {
    /// Create new privacy layer
    pub fn new(config: PrivacyConfig) -> Self {
        PrivacyLayer { config }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        PrivacyLayer {
            config: PrivacyConfig::default(),
        }
    }

    /// Apply message padding to data
    ///
    /// Returns padded data with padding indicator bytes.
    /// Format: [original_data][padding_length: u16][random_padding]
    pub fn pad_message(&self, data: &[u8]) -> Vec<u8> {
        match self.config.padding_strategy {
            PaddingStrategy::None => data.to_vec(),

            PaddingStrategy::MinSize => {
                if data.len() >= self.config.min_message_size {
                    return data.to_vec();
                }

                let padding_needed = self.config.min_message_size - data.len() - 2; // 2 bytes for length
                self.apply_padding(data, padding_needed)
            }

            PaddingStrategy::FixedBuckets => {
                let buckets = [512, 1024, 2048, 4096, 8192, 16384];
                let target_size = buckets.iter()
                    .find(|&&size| size >= data.len() + 2)
                    .copied()
                    .unwrap_or(data.len() + 2);

                if target_size > data.len() + 2 {
                    let padding_needed = target_size - data.len() - 2;
                    self.apply_padding(data, padding_needed)
                } else {
                    data.to_vec()
                }
            }

            PaddingStrategy::Random => {
                let mut rng = rand::thread_rng();
                let padding_size = rng.gen_range(0..=self.config.max_padding_size);
                self.apply_padding(data, padding_size)
            }
        }
    }

    /// Remove padding from padded message
    pub fn unpad_message(&self, padded: &[u8]) -> Result<Vec<u8>, String> {
        if padded.len() < 2 {
            return Ok(padded.to_vec());
        }

        // Read padding length from end
        let data_len = padded.len();

        // Check if this message has padding indicator
        // Padding format: [data][padding_length: u16][padding_bytes]
        // We need to find where the padding length indicator is

        // For now, assume no padding if we can't determine
        // TODO: Implement proper padding detection based on strategy
        Ok(padded.to_vec())
    }

    /// Apply padding to data
    fn apply_padding(&self, data: &[u8], padding_size: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(data.len() + padding_size + 2);

        // Original data
        result.extend_from_slice(data);

        // Padding length (u16)
        result.extend_from_slice(&(padding_size as u16).to_le_bytes());

        // Random padding
        let mut rng = rand::thread_rng();
        let padding: Vec<u8> = (0..padding_size).map(|_| rng.gen()).collect();
        result.extend_from_slice(&padding);

        result
    }

    /// Calculate delay based on timing strategy
    pub fn calculate_delay(&self) -> Duration {
        match self.config.timing_strategy {
            TimingStrategy::None => Duration::from_millis(0),

            TimingStrategy::FixedDelay => {
                Duration::from_millis(self.config.base_delay_ms)
            }

            TimingStrategy::RandomDelay => {
                let mut rng = rand::thread_rng();
                let delay = rng.gen_range(self.config.base_delay_ms..=self.config.max_delay_ms);
                Duration::from_millis(delay)
            }

            TimingStrategy::ExponentialDelay => {
                // Exponential distribution with mean = base_delay_ms
                let mut rng = rand::thread_rng();
                let u: f64 = rng.gen(); // Random value [0, 1)
                let lambda = 1.0 / (self.config.base_delay_ms as f64);
                let delay = (-u.ln() / lambda).min(self.config.max_delay_ms as f64);
                Duration::from_millis(delay as u64)
            }
        }
    }

    /// Check if cover traffic should be sent
    pub fn should_send_cover_traffic(&self, time_since_last: Duration) -> bool {
        if !self.config.enable_cover_traffic {
            return false;
        }

        // Calculate expected interval between cover traffic messages
        let interval_secs = 3600.0 / (self.config.cover_traffic_rate as f64);
        let expected_interval = Duration::from_secs_f64(interval_secs);

        // Add some randomness (±20%)
        let mut rng = rand::thread_rng();
        let jitter = rng.gen_range(0.8..1.2);
        let actual_interval = expected_interval.mul_f64(jitter);

        time_since_last >= actual_interval
    }

    /// Generate cover traffic message
    ///
    /// Returns a random message of appropriate size to blend with real traffic
    pub fn generate_cover_message(&self) -> Vec<u8> {
        let mut rng = rand::thread_rng();

        // Generate random size based on padding strategy
        let size = match self.config.padding_strategy {
            PaddingStrategy::None | PaddingStrategy::MinSize => {
                self.config.min_message_size
            }
            PaddingStrategy::FixedBuckets => {
                let buckets = [512, 1024, 2048, 4096];
                *buckets.choose(&mut rng).unwrap_or(&1024)
            }
            PaddingStrategy::Random => {
                rng.gen_range(self.config.min_message_size..=self.config.max_padding_size)
            }
        };

        // Generate random data
        (0..size).map(|_| rng.gen()).collect()
    }
}

// Helper trait for choosing random elements
trait SliceExt<T> {
    fn choose(&self, rng: &mut impl Rng) -> Option<&T>;
}

impl<T> SliceExt<T> for [T] {
    fn choose(&self, rng: &mut impl Rng) -> Option<&T> {
        if self.is_empty() {
            None
        } else {
            let idx = rng.gen_range(0..self.len());
            Some(&self[idx])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_padding() {
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::None,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let data = vec![1, 2, 3, 4, 5];
        let padded = layer.pad_message(&data);

        assert_eq!(padded, data);
    }

    #[test]
    fn test_min_size_padding() {
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::MinSize,
            min_message_size: 512,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let data = vec![1, 2, 3, 4, 5];
        let padded = layer.pad_message(&data);

        // Should be padded to at least 512 bytes
        assert!(padded.len() >= 512);

        // Original data should be at start
        assert_eq!(&padded[..5], &data[..]);
    }

    #[test]
    fn test_fixed_bucket_padding() {
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::FixedBuckets,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let data = vec![1, 2, 3, 4, 5];
        let padded = layer.pad_message(&data);

        // Should be padded to one of the bucket sizes
        let buckets = [512, 1024, 2048, 4096, 8192, 16384];
        assert!(buckets.contains(&padded.len()));
    }

    #[test]
    fn test_random_padding() {
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::Random,
            max_padding_size: 1024,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let data = vec![1, 2, 3, 4, 5];
        let padded = layer.pad_message(&data);

        // Should have some padding
        assert!(padded.len() >= data.len());
    }

    #[test]
    fn test_timing_fixed_delay() {
        let config = PrivacyConfig {
            timing_strategy: TimingStrategy::FixedDelay,
            base_delay_ms: 100,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let delay = layer.calculate_delay();

        assert_eq!(delay, Duration::from_millis(100));
    }

    #[test]
    fn test_timing_random_delay() {
        let config = PrivacyConfig {
            timing_strategy: TimingStrategy::RandomDelay,
            base_delay_ms: 50,
            max_delay_ms: 200,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        // Test multiple times to ensure randomness
        for _ in 0..10 {
            let delay = layer.calculate_delay();
            assert!(delay >= Duration::from_millis(50));
            assert!(delay <= Duration::from_millis(200));
        }
    }

    #[test]
    fn test_timing_no_delay() {
        let config = PrivacyConfig {
            timing_strategy: TimingStrategy::None,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let delay = layer.calculate_delay();

        assert_eq!(delay, Duration::from_millis(0));
    }

    #[test]
    fn test_cover_traffic_disabled() {
        let config = PrivacyConfig {
            enable_cover_traffic: false,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let should_send = layer.should_send_cover_traffic(Duration::from_secs(3600));

        assert!(!should_send);
    }

    #[test]
    fn test_cover_traffic_enabled() {
        let config = PrivacyConfig {
            enable_cover_traffic: true,
            cover_traffic_rate: 10, // 10 messages per hour
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        // Should not send immediately
        let should_send = layer.should_send_cover_traffic(Duration::from_secs(0));
        assert!(!should_send);

        // Should send after enough time (3600/10 = 360s, with jitter up to 432s, use 500s to be safe)
        let should_send = layer.should_send_cover_traffic(Duration::from_secs(500));
        assert!(should_send);
    }

    #[test]
    fn test_cover_message_generation() {
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::MinSize,
            min_message_size: 512,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let cover_msg = layer.generate_cover_message();

        assert_eq!(cover_msg.len(), 512);
    }
}
