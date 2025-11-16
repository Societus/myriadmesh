use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration, Instant};
use tracing::{debug, error, info, warn};

use crate::scoring::{AdapterMetrics, AdapterScorer, ScoringWeights};
use myriadmesh_network::AdapterManager;

// Re-export for ergonomic imports in tests and other modules
pub use crate::config::FailoverConfig;

/// Failover event types
#[derive(Debug, Clone)]
pub enum FailoverEvent {
    /// Adapter switched due to better score
    AdapterSwitch {
        from: String,
        to: String,
        reason: String,
    },
    /// Adapter failed threshold checks
    ThresholdViolation {
        adapter: String,
        metric: String,
        value: f64,
        threshold: f64,
    },
    /// Adapter became unavailable
    AdapterDown { adapter: String, reason: String },
    /// Adapter recovered and is now available
    AdapterRecovered { adapter: String },
}

/// Adapter health status
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Failed,
}

/// Tracks adapter health and metrics over time
#[derive(Debug, Clone)]
struct AdapterHealth {
    #[allow(dead_code)]
    adapter_id: String,
    status: HealthStatus,
    consecutive_failures: u32,
    last_check: Instant,
    current_metrics: Option<AdapterMetrics>,
    baseline_latency: Option<f64>,
}

impl AdapterHealth {
    fn new(adapter_id: String) -> Self {
        Self {
            adapter_id,
            status: HealthStatus::Healthy,
            consecutive_failures: 0,
            last_check: Instant::now(),
            current_metrics: None,
            baseline_latency: None,
        }
    }

    fn record_success(&mut self, metrics: AdapterMetrics) {
        self.consecutive_failures = 0;
        self.status = HealthStatus::Healthy;
        self.last_check = Instant::now();

        // Update baseline latency (exponential moving average)
        if let Some(baseline) = self.baseline_latency {
            self.baseline_latency = Some(baseline * 0.9 + metrics.latency_ms * 0.1);
        } else {
            self.baseline_latency = Some(metrics.latency_ms);
        }

        self.current_metrics = Some(metrics);
    }

    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.last_check = Instant::now();

        if self.consecutive_failures >= 3 {
            self.status = HealthStatus::Failed;
        } else if self.consecutive_failures >= 1 {
            self.status = HealthStatus::Degraded;
        }
    }

    fn is_latency_degraded(&self, threshold_multiplier: f32) -> bool {
        if let (Some(baseline), Some(metrics)) = (self.baseline_latency, &self.current_metrics) {
            metrics.latency_ms > baseline * threshold_multiplier as f64
        } else {
            false
        }
    }
}

/// Automatic failover manager
pub struct FailoverManager {
    config: FailoverConfig,
    adapter_manager: Arc<RwLock<AdapterManager>>,
    scorer: AdapterScorer,
    adapter_health: Arc<RwLock<HashMap<String, AdapterHealth>>>,
    current_primary: Arc<RwLock<Option<String>>>,
    event_log: Arc<RwLock<Vec<FailoverEvent>>>,
    // RESOURCE M4: Task handle management for graceful shutdown
    shutdown_tx: broadcast::Sender<()>,
    monitor_task: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl FailoverManager {
    pub fn new(
        config: FailoverConfig,
        adapter_manager: Arc<RwLock<AdapterManager>>,
        scoring_weights: ScoringWeights,
    ) -> Self {
        // RESOURCE M4: Create shutdown channel for graceful task termination
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        Self {
            config,
            adapter_manager,
            scorer: AdapterScorer::new(scoring_weights),
            adapter_health: Arc::new(RwLock::new(HashMap::new())),
            current_primary: Arc::new(RwLock::new(None)),
            event_log: Arc::new(RwLock::new(Vec::new())),
            shutdown_tx,
            monitor_task: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the failover monitoring loop
    pub async fn start(&self) -> Result<()> {
        if !self.config.auto_failover {
            info!("Auto-failover disabled");
            return Ok(());
        }

        info!("Starting automatic failover monitoring...");

        let config = self.config.clone();
        let adapter_manager = Arc::clone(&self.adapter_manager);
        let scorer = self.scorer.clone();
        let adapter_health = Arc::clone(&self.adapter_health);
        let current_primary = Arc::clone(&self.current_primary);
        let event_log = Arc::clone(&self.event_log);
        // RESOURCE M4: Subscribe to shutdown channel
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // RESOURCE M4: Spawn monitor task with shutdown handling
        let handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(10)); // Check every 10 seconds

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("Failover monitor shutting down");
                        break;
                    }
                    _ = ticker.tick() => {
                        if let Err(e) = Self::check_and_failover(
                            &config,
                            &adapter_manager,
                            &scorer,
                            &adapter_health,
                            &current_primary,
                            &event_log,
                        )
                        .await
                        {
                            error!("Failover check failed: {}", e);
                        }
                    }
                }
            }
        });

        // RESOURCE M4: Store task handle
        *self.monitor_task.write().await = Some(handle);

        Ok(())
    }

    /// Gracefully shutdown the failover monitor and wait for task to complete
    /// RESOURCE M4: Prevents task handle leaks and ensures cleanup
    pub async fn shutdown(&self) {
        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        // Wait for monitor task to complete
        if let Some(handle) = self.monitor_task.write().await.take() {
            let _ = handle.await;
        }
    }

    /// Check adapter health and perform failover if needed
    async fn check_and_failover(
        config: &FailoverConfig,
        adapter_manager: &Arc<RwLock<AdapterManager>>,
        scorer: &AdapterScorer,
        adapter_health: &Arc<RwLock<HashMap<String, AdapterHealth>>>,
        current_primary: &Arc<RwLock<Option<String>>>,
        event_log: &Arc<RwLock<Vec<FailoverEvent>>>,
    ) -> Result<()> {
        let manager = adapter_manager.read().await;
        let adapter_ids = manager.adapter_ids();

        // Collect metrics for all adapters
        let mut all_metrics = HashMap::new();
        let mut health_map = adapter_health.write().await;

        for adapter_id in &adapter_ids {
            // Initialize health tracking if new adapter
            health_map
                .entry(adapter_id.clone())
                .or_insert_with(|| AdapterHealth::new(adapter_id.clone()));

            if let Some(adapter) = manager.get_adapter(adapter_id) {
                let adapter_guard = adapter.read().await;
                let capabilities = adapter_guard.get_capabilities();
                let status = adapter_guard.get_status();

                // Check if adapter is available
                if !matches!(status, myriadmesh_network::adapter::AdapterStatus::Ready) {
                    if let Some(health) = health_map.get_mut(adapter_id) {
                        health.record_failure();

                        if health.status == HealthStatus::Failed {
                            let event = FailoverEvent::AdapterDown {
                                adapter: adapter_id.clone(),
                                reason: format!("Status: {:?}", status),
                            };
                            Self::log_event(event_log, event).await;
                        }
                    }
                    continue;
                }

                // Get metrics from adapter manager
                let metrics = if let Some(_adapter_metrics) = manager.get_metrics(adapter_id) {
                    // Convert network metrics to scoring metrics
                    AdapterMetrics {
                        latency_ms: capabilities.typical_latency_ms,
                        bandwidth_bps: capabilities.typical_bandwidth_bps,
                        reliability: capabilities.reliability,
                        power_consumption: match capabilities.power_consumption {
                            myriadmesh_network::types::PowerConsumption::None => 0.0,
                            myriadmesh_network::types::PowerConsumption::VeryLow => 0.1,
                            myriadmesh_network::types::PowerConsumption::Low => 0.3,
                            myriadmesh_network::types::PowerConsumption::Medium => 0.5,
                            myriadmesh_network::types::PowerConsumption::High => 0.7,
                            myriadmesh_network::types::PowerConsumption::VeryHigh => 0.9,
                        },
                        privacy_level: Self::estimate_privacy_level(adapter_id),
                    }
                } else {
                    // Fallback to capability-based metrics
                    AdapterMetrics {
                        latency_ms: capabilities.typical_latency_ms,
                        bandwidth_bps: capabilities.typical_bandwidth_bps,
                        reliability: capabilities.reliability,
                        power_consumption: 0.5,
                        privacy_level: Self::estimate_privacy_level(adapter_id),
                    }
                };

                // Check threshold violations
                if let Some(health) = health_map.get_mut(adapter_id) {
                    if health.is_latency_degraded(config.latency_threshold_multiplier) {
                        let event = FailoverEvent::ThresholdViolation {
                            adapter: adapter_id.clone(),
                            metric: "latency".to_string(),
                            value: metrics.latency_ms,
                            threshold: health.baseline_latency.unwrap_or(0.0)
                                * config.latency_threshold_multiplier as f64,
                        };
                        Self::log_event(event_log, event).await;
                        health.record_failure();
                    } else {
                        health.record_success(metrics.clone());
                    }
                }

                all_metrics.insert(adapter_id.clone(), metrics);
            }
        }

        drop(health_map);
        drop(manager);

        // Only failover if we have multiple adapters
        if all_metrics.len() < 2 {
            return Ok(());
        }

        // Calculate scores for all healthy adapters
        let scores = scorer.rank_adapters(all_metrics);

        if scores.is_empty() {
            warn!("No healthy adapters available for failover");
            return Ok(());
        }

        let best = &scores[0];
        let mut primary = current_primary.write().await;

        // Check if we should switch
        let should_switch = if let Some(current) = &*primary {
            // Find current adapter's score
            let current_score = scores
                .iter()
                .find(|s| &s.adapter_id == current)
                .map(|s| s.total_score);

            if let Some(current_score) = current_score {
                // Switch if best adapter is significantly better (>10% improvement)
                best.total_score > current_score * 1.10
            } else {
                // Current adapter not in healthy list, definitely switch
                true
            }
        } else {
            // No primary set, pick the best
            true
        };

        if should_switch {
            let from = primary.clone().unwrap_or_else(|| "none".to_string());
            let to = best.adapter_id.clone();

            info!(
                "Failover: switching primary adapter from '{}' to '{}' (score: {:.3})",
                from, to, best.total_score
            );

            *primary = Some(to.clone());

            let event = FailoverEvent::AdapterSwitch {
                from,
                to,
                reason: format!("Better score: {:.3}", best.total_score),
            };
            Self::log_event(event_log, event).await;
        }

        Ok(())
    }

    /// Estimate privacy level based on adapter type
    fn estimate_privacy_level(adapter_id: &str) -> f64 {
        if adapter_id.contains("i2p") {
            0.95
        } else if adapter_id.contains("bluetooth") && !adapter_id.contains("_le") {
            0.85
        } else if adapter_id.contains("bluetooth_le") {
            0.70
        } else if adapter_id.contains("ethernet") || adapter_id.contains("wifi") {
            0.15
        } else if adapter_id.contains("cellular") {
            0.10
        } else {
            0.50 // Default medium privacy
        }
    }

    /// Log a failover event
    async fn log_event(event_log: &Arc<RwLock<Vec<FailoverEvent>>>, event: FailoverEvent) {
        debug!("Failover event: {:?}", event);
        let mut log = event_log.write().await;
        log.push(event);

        // Keep only last 100 events
        let len = log.len();
        if len > 100 {
            log.drain(0..len - 100);
        }
    }

    /// Get the current primary adapter
    pub async fn get_primary_adapter(&self) -> Option<String> {
        self.current_primary.read().await.clone()
    }

    /// Get recent failover events
    pub async fn get_recent_events(&self, count: usize) -> Vec<FailoverEvent> {
        let log = self.event_log.read().await;
        log.iter().rev().take(count).cloned().collect()
    }

    /// Force a failover to a specific adapter
    pub async fn force_failover(&self, adapter_id: String) -> Result<()> {
        let manager = self.adapter_manager.read().await;

        if manager.get_adapter(&adapter_id).is_none() {
            anyhow::bail!("Adapter '{}' not found", adapter_id);
        }

        let mut primary = self.current_primary.write().await;
        let from = primary.clone().unwrap_or_else(|| "none".to_string());

        info!("Forced failover from '{}' to '{}'", from, adapter_id);

        *primary = Some(adapter_id.clone());

        let event = FailoverEvent::AdapterSwitch {
            from,
            to: adapter_id,
            reason: "Manual override".to_string(),
        };
        Self::log_event(&self.event_log, event).await;

        Ok(())
    }

    /// Get health status of all adapters
    pub async fn get_adapter_health(&self) -> HashMap<String, HealthStatus> {
        let health = self.adapter_health.read().await;
        health
            .iter()
            .map(|(id, h)| (id.clone(), h.status.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_health_tracking() {
        let mut health = AdapterHealth::new("test".to_string());

        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.consecutive_failures, 0);

        // Record failure
        health.record_failure();
        assert_eq!(health.status, HealthStatus::Degraded);
        assert_eq!(health.consecutive_failures, 1);

        // More failures
        health.record_failure();
        health.record_failure();
        assert_eq!(health.status, HealthStatus::Failed);
        assert_eq!(health.consecutive_failures, 3);

        // Recovery
        let metrics = AdapterMetrics {
            latency_ms: 50.0,
            bandwidth_bps: 10_000_000,
            reliability: 0.95,
            power_consumption: 0.3,
            privacy_level: 0.5,
        };
        health.record_success(metrics);
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.consecutive_failures, 0);
    }

    #[test]
    fn test_latency_degradation_detection() {
        let mut health = AdapterHealth::new("test".to_string());

        // Set baseline
        let baseline_metrics = AdapterMetrics {
            latency_ms: 50.0,
            bandwidth_bps: 10_000_000,
            reliability: 0.95,
            power_consumption: 0.3,
            privacy_level: 0.5,
        };
        health.record_success(baseline_metrics);

        // Normal latency - check before recording
        let normal_metrics = AdapterMetrics {
            latency_ms: 60.0, // 20% increase from baseline 50
            bandwidth_bps: 10_000_000,
            reliability: 0.95,
            power_consumption: 0.3,
            privacy_level: 0.5,
        };
        // Set current metrics manually for testing
        health.current_metrics = Some(normal_metrics.clone());
        assert!(!health.is_latency_degraded(5.0)); // 60 < 50*5, OK

        // Degraded latency - simulate checking a spike
        let degraded_metrics = AdapterMetrics {
            latency_ms: 300.0, // 6x increase from baseline ~50
            bandwidth_bps: 10_000_000,
            reliability: 0.95,
            power_consumption: 0.3,
            privacy_level: 0.5,
        };
        health.current_metrics = Some(degraded_metrics.clone());
        assert!(health.is_latency_degraded(5.0)); // 300 > 50*5, degraded!
    }

    #[test]
    fn test_privacy_level_estimation() {
        assert_eq!(FailoverManager::estimate_privacy_level("i2p"), 0.95);
        assert_eq!(FailoverManager::estimate_privacy_level("bluetooth"), 0.85);
        assert_eq!(
            FailoverManager::estimate_privacy_level("bluetooth_le"),
            0.70
        );
        assert_eq!(FailoverManager::estimate_privacy_level("ethernet"), 0.15);
        assert_eq!(FailoverManager::estimate_privacy_level("cellular"), 0.10);
        assert_eq!(FailoverManager::estimate_privacy_level("unknown"), 0.50);
    }
}
