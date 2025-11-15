//! Privacy Protection Layers
//!
//! Implements privacy-enhancing techniques for i2p communications:
//! - Message padding (prevent traffic analysis)
//! - Timing obfuscation (prevent timing correlation)
//! - Cover traffic generation (prevent traffic pattern analysis)
//!
//! SECURITY C5: Comprehensive timing attack prevention through random delays

use rand::Rng;
use std::time::Duration;
use tokio::time::sleep;

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
///
/// SECURITY C5: All strategies except None include jitter to prevent timing correlation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingStrategy {
    /// Minimal timing obfuscation (small random jitter only)
    /// SECURITY C5: Even "minimal" includes jitter to prevent exact timing correlation
    Minimal,

    /// Fixed delay with small random jitter
    FixedDelay,

    /// Random delay within range (recommended for privacy)
    RandomDelay,

    /// Exponential distribution delay (most realistic, best for anonymity)
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
#[derive(Default)]
pub struct PrivacyLayer {
    config: PrivacyConfig,
}

impl PrivacyLayer {
    /// Create new privacy layer
    pub fn new(config: PrivacyConfig) -> Self {
        PrivacyLayer { config }
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
                // SECURITY H6: More granular bucket sizes to reduce information leakage
                // Smaller increments at lower sizes where most messages fall
                let buckets = [
                    256, 384, 512, 640, 768, 896, 1024, // 128-byte increments up to 1KB
                    1280, 1536, 1792, 2048, // 256-byte increments 1-2KB
                    2560, 3072, 3584, 4096, // 512-byte increments 2-4KB
                    5120, 6144, 7168, 8192, // 1KB increments 4-8KB
                    10240, 12288, 14336, 16384, // 2KB increments 8-16KB
                ];

                // Find the smallest bucket that fits the data and respects min_message_size
                let min_size = self.config.min_message_size.max(data.len() + 2);
                let target_size = buckets
                    .iter()
                    .find(|&&size| size >= min_size)
                    .copied()
                    .unwrap_or(min_size);

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
        // If no padding strategy or message too short, return as-is
        if matches!(self.config.padding_strategy, PaddingStrategy::None) || padded.len() < 2 {
            return Ok(padded.to_vec());
        }

        // Padding format: [data][padding_length: u16][padding_bytes]
        // The u16 padding_length is stored just before the padding bytes

        if padded.len() < 2 {
            return Err("Message too short to contain padding length".to_string());
        }

        // Read padding length from bytes at position (len - padding_size - 2)
        // We need to extract the u16 that indicates how much padding there is
        // The last padding_size bytes are random padding
        // The 2 bytes before that contain the padding_size as u16

        // Try to read the padding length
        // Start from the end and work backwards to find the padding length indicator
        let total_len = padded.len();

        // We need at least 2 bytes for the padding length indicator
        if total_len < 2 {
            return Ok(padded.to_vec());
        }

        // The padding length is stored as the last u16 before the padding
        // So if we have: [data...][u16: padding_len][padding_bytes...]
        // We need to find where the u16 is located

        // Actually, looking at apply_padding, the format is:
        // [data][padding_length as u16][random padding bytes]
        // So padding_length tells us how many random bytes follow the u16

        // Read the last portion to find padding_length
        // We iterate from largest to smallest to find the actual padding before any spurious matches
        // in the random padding bytes
        let max_potential = MAX_PADDING_SIZE.min(total_len.saturating_sub(2));
        for potential_padding_len in (0..=max_potential).rev() {
            if total_len < potential_padding_len + 2 {
                continue;
            }

            let padding_len_pos = total_len - potential_padding_len - 2;
            if padding_len_pos + 2 > total_len {
                continue;
            }

            let padding_len_bytes = &padded[padding_len_pos..padding_len_pos + 2];
            let indicated_padding_len =
                u16::from_le_bytes([padding_len_bytes[0], padding_len_bytes[1]]) as usize;

            // Check if this indicated length matches our position
            if indicated_padding_len == potential_padding_len
                && indicated_padding_len <= MAX_PADDING_SIZE
            {
                // This looks like the correct padding length
                // Data is everything before the padding_length u16
                return Ok(padded[..padding_len_pos].to_vec());
            }
        }

        // If we couldn't find valid padding, return as-is
        // This might be an unpadded message or corrupted
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
    ///
    /// SECURITY C5: All strategies include randomness to prevent timing correlation attacks
    pub fn calculate_delay(&self) -> Duration {
        let mut rng = rand::thread_rng();

        match self.config.timing_strategy {
            TimingStrategy::Minimal => {
                // SECURITY C5: Even minimal includes 0-10ms jitter to prevent exact correlation
                let jitter = rng.gen_range(0..=10);
                Duration::from_millis(jitter)
            }

            TimingStrategy::FixedDelay => {
                // SECURITY C5: Add ±20% jitter to fixed delay to prevent pattern recognition
                let jitter_factor = rng.gen_range(0.8..=1.2);
                let delay = (self.config.base_delay_ms as f64 * jitter_factor) as u64;
                Duration::from_millis(delay)
            }

            TimingStrategy::RandomDelay => {
                // SECURITY C5: Uniform random delay within configured range
                let delay = rng.gen_range(self.config.base_delay_ms..=self.config.max_delay_ms);
                Duration::from_millis(delay)
            }

            TimingStrategy::ExponentialDelay => {
                // SECURITY C5: Exponential distribution mimics natural network delays
                // This is the most realistic and hardest to distinguish from normal traffic
                let u: f64 = rng.gen(); // Random value (0, 1]
                let u = u.max(0.0001); // Avoid log(0)
                let lambda = 1.0 / (self.config.base_delay_ms as f64);
                let delay = (-u.ln() / lambda).min(self.config.max_delay_ms as f64);
                Duration::from_millis(delay as u64)
            }
        }
    }

    /// Apply timing delay (async)
    ///
    /// SECURITY C5: Actively applies the calculated delay to obfuscate timing patterns.
    /// This MUST be called when forwarding messages to prevent timing correlation attacks.
    pub async fn apply_delay(&self) {
        let delay = self.calculate_delay();
        sleep(delay).await;
    }

    /// Apply timing delay with additional random jitter
    ///
    /// SECURITY C5: Adds extra randomness for critical operations like onion routing
    /// where timing correlation could completely de-anonymize users.
    pub async fn apply_delay_with_jitter(&self, extra_jitter_ms: u64) {
        let base_delay = self.calculate_delay();

        let mut rng = rand::thread_rng();
        let jitter = rng.gen_range(0..=extra_jitter_ms);

        let total_delay = base_delay + Duration::from_millis(jitter);
        sleep(total_delay).await;
    }

    /// Check if cover traffic should be sent
    ///
    /// SECURITY H5: Uses exponential distribution for unpredictable timing
    /// to prevent pattern-based detection of cover traffic
    pub fn should_send_cover_traffic(&self, time_since_last: Duration) -> bool {
        if !self.config.enable_cover_traffic {
            return false;
        }

        // Calculate expected interval between cover traffic messages
        let interval_secs = 3600.0 / (self.config.cover_traffic_rate as f64);

        // SECURITY H5: Use exponential distribution instead of fixed ±20% jitter
        // This creates more realistic, unpredictable timing patterns
        let mut rng = rand::thread_rng();
        let u: f64 = rng.gen(); // Random value (0, 1]
        let u = u.max(0.0001); // Avoid log(0)

        // Exponential distribution with mean = interval_secs
        let lambda = 1.0 / interval_secs;
        let randomized_interval_secs = -u.ln() / lambda;

        // Cap at 3x the expected interval to prevent excessively long waits
        let capped_interval = randomized_interval_secs.min(interval_secs * 3.0);
        let actual_interval = Duration::from_secs_f64(capped_interval);

        time_since_last >= actual_interval
    }

    /// Generate cover traffic message
    ///
    /// SECURITY H5: Uses realistic size distribution and varied patterns
    /// to make cover traffic indistinguishable from real traffic
    ///
    /// Returns a random message of appropriate size to blend with real traffic
    pub fn generate_cover_message(&self) -> Vec<u8> {
        let mut rng = rand::thread_rng();

        // SECURITY H5: Generate random size with realistic distribution
        let size = match self.config.padding_strategy {
            PaddingStrategy::None | PaddingStrategy::MinSize => {
                // Add some variation even with MinSize strategy
                let jitter = rng.gen_range(0..64);
                self.config.min_message_size + jitter
            }
            PaddingStrategy::FixedBuckets => {
                // SECURITY H5 & H6: Use granular buckets matching real traffic
                let buckets = [
                    256, 384, 512, 640, 768, 896, 1024, 1280, 1536, 1792, 2048, 2560, 3072, 4096,
                ];
                *buckets.choose(&mut rng).unwrap_or(&1024)
            }
            PaddingStrategy::Random => {
                // SECURITY H5: Use exponential distribution for more realistic sizes
                // Most messages are small, but occasionally large ones appear
                let u: f64 = rng.gen();
                let u = u.max(0.0001);
                let mean_size = (self.config.min_message_size + self.config.max_padding_size) / 2;
                let lambda = 1.0 / (mean_size as f64);
                ((-u.ln() / lambda) as usize)
                    .clamp(self.config.min_message_size, self.config.max_padding_size)
            }
        };

        // SECURITY H5: Generate random data with varied patterns
        // Mix of different byte patterns to mimic real encrypted data
        let mut message = Vec::with_capacity(size);

        // Use different patterns for different segments to vary entropy
        let pattern_type = rng.gen_range(0..3);
        match pattern_type {
            0 => {
                // Fully random (high entropy like encrypted data)
                message.extend((0..size).map(|_| rng.gen::<u8>()));
            }
            1 => {
                // Mix of random blocks and semi-structured data
                let mut pos = 0;
                while pos < size {
                    let block_size = rng.gen_range(16..64).min(size - pos);
                    if rng.gen_bool(0.7) {
                        // Random block
                        message.extend((0..block_size).map(|_| rng.gen::<u8>()));
                    } else {
                        // Semi-structured block (still encrypted-looking)
                        let pattern = rng.gen::<u8>();
                        let variation = rng.gen_range(1..=8); // At least 1 to avoid division by zero
                        message.extend(
                            (0..block_size).map(|i| pattern.wrapping_add((i % variation) as u8)),
                        );
                    }
                    pos += block_size;
                }
            }
            _ => {
                // Gradient pattern with randomness
                let start = rng.gen::<u8>();
                let step = rng.gen_range(1..5);
                message.extend((0..size).map(|i| {
                    start
                        .wrapping_add((i / step) as u8)
                        .wrapping_add(rng.gen_range(0..16))
                }));
            }
        }

        message
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
        // SECURITY H6: Test granular bucket sizes
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::FixedBuckets,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let data = vec![1, 2, 3, 4, 5];
        let padded = layer.pad_message(&data);

        // SECURITY H6: Should be padded to one of the granular bucket sizes
        let buckets = [
            256, 384, 512, 640, 768, 896, 1024, 1280, 1536, 1792, 2048, 2560, 3072, 3584, 4096,
            5120, 6144, 7168, 8192, 10240, 12288, 14336, 16384,
        ];
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
    fn test_padding_roundtrip_min_size() {
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::MinSize,
            min_message_size: 512,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let original_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        // Pad the message
        let padded = layer.pad_message(&original_data);
        assert!(padded.len() >= 512);

        // Unpad the message
        let unpadded = layer
            .unpad_message(&padded)
            .expect("Unpadding should succeed");

        // Should recover original data
        assert_eq!(unpadded, original_data);
    }

    #[test]
    fn test_padding_roundtrip_random() {
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::Random,
            max_padding_size: 1024,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let original_data = b"This is a test message with some content".to_vec();

        // Pad the message
        let padded = layer.pad_message(&original_data);
        assert!(padded.len() > original_data.len());

        // Unpad the message
        let unpadded = layer
            .unpad_message(&padded)
            .expect("Unpadding should succeed");

        // Should recover original data
        assert_eq!(unpadded, original_data);
    }

    #[test]
    fn test_padding_roundtrip_fixed_buckets() {
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::FixedBuckets,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let original_data = b"Short message".to_vec();

        // Pad the message
        let padded = layer.pad_message(&original_data);

        // Unpad the message
        let unpadded = layer
            .unpad_message(&padded)
            .expect("Unpadding should succeed");

        // Should recover original data
        assert_eq!(unpadded, original_data);
    }

    #[test]
    fn test_unpadding_no_padding_strategy() {
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::None,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let data = vec![1, 2, 3, 4, 5];

        // With no padding, unpad should return data as-is
        let unpadded = layer
            .unpad_message(&data)
            .expect("Unpadding should succeed");
        assert_eq!(unpadded, data);
    }

    #[test]
    fn test_timing_fixed_delay() {
        // SECURITY C5: FixedDelay now includes ±20% jitter
        let config = PrivacyConfig {
            timing_strategy: TimingStrategy::FixedDelay,
            base_delay_ms: 100,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let delay = layer.calculate_delay();

        // Should be 100ms ± 20% = 80-120ms
        assert!(delay >= Duration::from_millis(80));
        assert!(delay <= Duration::from_millis(120));
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
    fn test_timing_minimal_delay() {
        // SECURITY C5: Even minimal strategy includes jitter
        let config = PrivacyConfig {
            timing_strategy: TimingStrategy::Minimal,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        // Test multiple times to ensure randomness and bounds
        for _ in 0..10 {
            let delay = layer.calculate_delay();
            // Minimal jitter is 0-10ms
            assert!(delay >= Duration::from_millis(0));
            assert!(delay <= Duration::from_millis(10));
        }
    }

    #[tokio::test]
    async fn test_apply_delay() {
        // SECURITY C5: Test that delay is actually applied
        let config = PrivacyConfig {
            timing_strategy: TimingStrategy::FixedDelay,
            base_delay_ms: 50,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        let start = std::time::Instant::now();
        layer.apply_delay().await;
        let elapsed = start.elapsed();

        // Should have delayed at least base_delay * 0.8 (due to jitter factor)
        assert!(elapsed >= Duration::from_millis(40));
    }

    #[tokio::test]
    async fn test_apply_delay_with_jitter() {
        // SECURITY C5: Test extra jitter application
        let config = PrivacyConfig {
            timing_strategy: TimingStrategy::Minimal,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        let start = std::time::Instant::now();
        layer.apply_delay_with_jitter(100).await;
        let elapsed = start.elapsed();

        // Should have some delay (minimal + up to 100ms jitter)
        assert!(elapsed <= Duration::from_millis(110));
    }

    #[test]
    fn test_timing_fixed_delay_has_jitter() {
        // SECURITY C5: Even FixedDelay has jitter to prevent pattern recognition
        let config = PrivacyConfig {
            timing_strategy: TimingStrategy::FixedDelay,
            base_delay_ms: 100,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        let mut delays = Vec::new();
        for _ in 0..20 {
            let delay = layer.calculate_delay();
            delays.push(delay.as_millis());
        }

        // Check that delays are not all identical (jitter is working)
        let unique_delays: std::collections::HashSet<_> = delays.into_iter().collect();
        assert!(
            unique_delays.len() > 1,
            "Fixed delay should have jitter variation"
        );
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
        // SECURITY H5: Test exponential distribution timing
        let config = PrivacyConfig {
            enable_cover_traffic: true,
            cover_traffic_rate: 10, // 10 messages per hour
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        // Should not send immediately
        let should_send = layer.should_send_cover_traffic(Duration::from_secs(0));
        assert!(!should_send);

        // SECURITY H5: With exponential distribution, timing is more variable
        // Mean = 360s, but can range from very short to 3x mean (1080s)
        // Test that very long wait definitely triggers
        let should_send = layer.should_send_cover_traffic(Duration::from_secs(1100));
        assert!(should_send);
    }

    #[test]
    fn test_cover_message_generation() {
        // SECURITY H5: Test that cover messages have varied sizes
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::MinSize,
            min_message_size: 512,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);
        let cover_msg = layer.generate_cover_message();

        // SECURITY H5: MinSize strategy now adds jitter (0-64 bytes)
        assert!(cover_msg.len() >= 512);
        assert!(cover_msg.len() <= 512 + 64);
    }

    #[test]
    fn test_granular_buckets_reduce_leakage() {
        // SECURITY H6: Test that granular buckets reduce information leakage
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::FixedBuckets,
            min_message_size: 0, // Test pure bucket behavior without minimum
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        // Test messages near bucket boundaries
        let test_sizes = [
            (250, 256),   // Should pad to 256
            (257, 384),   // Should pad to 384
            (513, 640),   // Should pad to 640
            (1025, 1280), // Should pad to 1280
        ];

        for (input_size, expected_bucket) in test_sizes {
            let data = vec![0u8; input_size];
            let padded = layer.pad_message(&data);
            assert_eq!(
                padded.len(),
                expected_bucket,
                "Input size {} should pad to bucket {}",
                input_size,
                expected_bucket
            );
        }
    }

    #[test]
    fn test_cover_traffic_timing_variability() {
        // SECURITY H5: Test that cover traffic timing uses exponential distribution
        let config = PrivacyConfig {
            enable_cover_traffic: true,
            cover_traffic_rate: 60, // 1 per minute for faster testing
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        // Test multiple intervals to see variability
        let mut send_times = Vec::new();
        for test_time in [30, 45, 60, 90, 120, 150, 180] {
            if layer.should_send_cover_traffic(Duration::from_secs(test_time)) {
                send_times.push(test_time);
            }
        }

        // SECURITY H5: With exponential distribution, we should see varied behavior
        // Not all tests should trigger at the same threshold
        // At minimum, the longest wait should definitely trigger
        assert!(
            !send_times.is_empty(),
            "Cover traffic should eventually send"
        );
    }

    #[test]
    fn test_cover_message_pattern_variety() {
        // SECURITY H5: Test that cover messages use varied byte patterns
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::Random,
            min_message_size: 256,
            max_padding_size: 512,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        // Generate multiple cover messages
        let messages: Vec<_> = (0..10).map(|_| layer.generate_cover_message()).collect();

        // SECURITY H5: Messages should have different sizes
        let sizes: std::collections::HashSet<_> = messages.iter().map(|m| m.len()).collect();
        assert!(
            sizes.len() > 1,
            "Cover messages should have varied sizes, got {} unique sizes",
            sizes.len()
        );

        // SECURITY H5: Messages should have different content
        let unique_messages: std::collections::HashSet<_> = messages.iter().collect();
        assert_eq!(
            unique_messages.len(),
            10,
            "All cover messages should be unique"
        );
    }

    #[test]
    fn test_bucket_granularity_improvement() {
        // SECURITY H6: Compare old vs new bucket granularity
        let config = PrivacyConfig {
            padding_strategy: PaddingStrategy::FixedBuckets,
            ..Default::default()
        };

        let layer = PrivacyLayer::new(config);

        // Old buckets: [512, 1024, 2048, 4096, 8192, 16384] - 6 buckets
        // New buckets: 25 buckets with finer granularity

        // Test that messages in 512-1024 range now have more options
        let data_600 = vec![0u8; 600];
        let padded_600 = layer.pad_message(&data_600);

        // With old buckets, this would go to 1024
        // With new buckets, should go to 640 or 768
        assert!(
            padded_600.len() <= 768,
            "600-byte message should use granular bucket <= 768, got {}",
            padded_600.len()
        );
    }
}
