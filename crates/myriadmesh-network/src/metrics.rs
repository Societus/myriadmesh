//! Network adapter performance metrics

use std::time::{Duration, Instant};

/// Performance metrics for a network adapter
#[derive(Debug, Clone)]
pub struct AdapterMetrics {
    /// Average latency in milliseconds
    pub latency_ms: f64,

    /// Measured bandwidth in bits per second
    pub bandwidth_bps: u64,

    /// Reliability (successful sends / total attempts)
    pub reliability: f64,

    /// Availability (time adapter was available)
    pub availability: f64,

    /// Cost per megabyte (USD)
    pub cost_per_mb: f64,

    /// Total messages sent
    pub messages_sent: u64,

    /// Total messages received
    pub messages_received: u64,

    /// Total send failures
    pub send_failures: u64,

    /// Total bytes sent
    pub bytes_sent: u64,

    /// Total bytes received
    pub bytes_received: u64,

    /// Last update timestamp
    pub last_updated: Instant,
}

impl AdapterMetrics {
    /// Create new metrics
    pub fn new() -> Self {
        AdapterMetrics {
            latency_ms: 0.0,
            bandwidth_bps: 0,
            reliability: 1.0,
            availability: 1.0,
            cost_per_mb: 0.0,
            messages_sent: 0,
            messages_received: 0,
            send_failures: 0,
            bytes_sent: 0,
            bytes_received: 0,
            last_updated: Instant::now(),
        }
    }

    /// Record a successful send
    pub fn record_send(&mut self, bytes: usize, latency: Duration) {
        self.messages_sent += 1;
        self.bytes_sent += bytes as u64;

        // Update moving average for latency
        let new_latency = latency.as_secs_f64() * 1000.0;
        if self.messages_sent == 1 {
            self.latency_ms = new_latency;
        } else {
            // Exponential moving average (alpha = 0.2)
            self.latency_ms = self.latency_ms * 0.8 + new_latency * 0.2;
        }

        // Update bandwidth estimate
        let total_time = self.last_updated.elapsed().as_secs_f64();
        if total_time > 0.0 {
            self.bandwidth_bps =
                ((self.bytes_sent + self.bytes_received) as f64 * 8.0 / total_time) as u64;
        }

        // Update reliability
        let total_attempts = self.messages_sent + self.send_failures;
        self.reliability = self.messages_sent as f64 / total_attempts as f64;

        self.last_updated = Instant::now();
    }

    /// Record a failed send
    pub fn record_send_failure(&mut self) {
        self.send_failures += 1;

        // Update reliability
        let total_attempts = self.messages_sent + self.send_failures;
        self.reliability = self.messages_sent as f64 / total_attempts as f64;

        self.last_updated = Instant::now();
    }

    /// Record a received message
    pub fn record_receive(&mut self, bytes: usize) {
        self.messages_received += 1;
        self.bytes_received += bytes as u64;

        // Update bandwidth estimate
        let total_time = self.last_updated.elapsed().as_secs_f64();
        if total_time > 0.0 {
            self.bandwidth_bps =
                ((self.bytes_sent + self.bytes_received) as f64 * 8.0 / total_time) as u64;
        }

        self.last_updated = Instant::now();
    }

    /// Get current throughput in messages per second
    pub fn throughput(&self) -> f64 {
        let elapsed = self.last_updated.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            (self.messages_sent + self.messages_received) as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Get send success rate
    pub fn send_success_rate(&self) -> f64 {
        let total = self.messages_sent + self.send_failures;
        if total > 0 {
            self.messages_sent as f64 / total as f64
        } else {
            1.0
        }
    }
}

impl Default for AdapterMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_metrics() {
        let metrics = AdapterMetrics::new();
        assert_eq!(metrics.messages_sent, 0);
        assert_eq!(metrics.messages_received, 0);
        assert_eq!(metrics.reliability, 1.0);
    }

    #[test]
    fn test_record_send() {
        let mut metrics = AdapterMetrics::new();

        metrics.record_send(100, Duration::from_millis(10));
        assert_eq!(metrics.messages_sent, 1);
        assert_eq!(metrics.bytes_sent, 100);
        assert!(metrics.latency_ms > 0.0);
        assert_eq!(metrics.reliability, 1.0);
    }

    #[test]
    fn test_record_send_failure() {
        let mut metrics = AdapterMetrics::new();

        metrics.record_send(100, Duration::from_millis(10));
        metrics.record_send_failure();

        assert_eq!(metrics.messages_sent, 1);
        assert_eq!(metrics.send_failures, 1);
        assert_eq!(metrics.reliability, 0.5);
    }

    #[test]
    fn test_record_receive() {
        let mut metrics = AdapterMetrics::new();

        metrics.record_receive(100);
        assert_eq!(metrics.messages_received, 1);
        assert_eq!(metrics.bytes_received, 100);
    }

    #[test]
    fn test_send_success_rate() {
        let mut metrics = AdapterMetrics::new();

        metrics.record_send(100, Duration::from_millis(10));
        metrics.record_send(100, Duration::from_millis(10));
        metrics.record_send_failure();

        assert_eq!(metrics.send_success_rate(), 2.0 / 3.0);
    }
}
