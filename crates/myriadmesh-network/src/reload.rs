//! Hot-reloadable adapter registry
//!
//! Allows updating individual network adapters without taking down
//! the entire node. Coordinates graceful connection draining and
//! atomic adapter swapping.

use crate::adapter::NetworkAdapter;
use crate::error::{NetworkError, Result};
use crate::version_tracking::SemanticVersion;
use myriadmesh_protocol::types::AdapterType;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Health metrics for monitoring adapter performance
#[derive(Debug, Clone)]
pub struct HealthMetrics {
    /// Total operations attempted
    pub total_operations: u64,

    /// Successful operations
    pub successful_operations: u64,

    /// Failed operations
    pub failed_operations: u64,

    /// Total latency in milliseconds
    pub total_latency_ms: u64,

    /// Number of latency samples
    pub latency_samples: u64,

    /// Number of crashes/panics
    pub crash_count: u32,

    /// When monitoring started
    pub started_at: Instant,
}

impl HealthMetrics {
    /// Create new health metrics
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            total_latency_ms: 0,
            latency_samples: 0,
            crash_count: 0,
            started_at: Instant::now(),
        }
    }

    /// Calculate success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            1.0 // No operations yet = assume healthy
        } else {
            self.successful_operations as f64 / self.total_operations as f64
        }
    }

    /// Calculate average latency in milliseconds
    pub fn average_latency_ms(&self) -> f64 {
        if self.latency_samples == 0 {
            0.0
        } else {
            self.total_latency_ms as f64 / self.latency_samples as f64
        }
    }

    /// Get error count
    pub fn error_count(&self) -> u64 {
        self.failed_operations
    }

    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Record a successful operation with latency
    pub fn record_success(&mut self, latency_ms: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.total_latency_ms += latency_ms;
        self.latency_samples += 1;
    }

    /// Record a failed operation
    pub fn record_failure(&mut self) {
        self.total_operations += 1;
        self.failed_operations += 1;
    }

    /// Record a crash
    pub fn record_crash(&mut self) {
        self.crash_count += 1;
    }
}

impl Default for HealthMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Degradation thresholds for triggering rollback
#[derive(Debug, Clone)]
pub struct DegradationThresholds {
    /// Maximum acceptable drop in success rate (e.g., 0.10 for 10%)
    pub max_success_rate_drop: f64,

    /// Maximum acceptable increase in latency (e.g., 0.50 for 50%)
    pub max_latency_increase: f64,

    /// Maximum acceptable error rate multiplier (e.g., 2.0 for 2x errors)
    pub max_error_rate_multiplier: f64,

    /// Any crashes trigger rollback
    pub crash_triggers_rollback: bool,

    /// Minimum operations before evaluation
    pub min_operations: u64,
}

impl Default for DegradationThresholds {
    fn default() -> Self {
        Self {
            max_success_rate_drop: 0.10,    // 10% drop triggers rollback
            max_latency_increase: 0.50,     // 50% increase triggers rollback
            max_error_rate_multiplier: 2.0, // 2x errors triggers rollback
            crash_triggers_rollback: true,  // Any crash triggers rollback
            min_operations: 10,             // Need at least 10 ops to evaluate
        }
    }
}

/// Health monitor for detecting adapter degradation after updates
pub struct AdapterHealthMonitor {
    /// Baseline metrics (before update)
    baseline: Arc<RwLock<HashMap<AdapterType, HealthMetrics>>>,

    /// Current metrics (after update)
    current: Arc<RwLock<HashMap<AdapterType, HealthMetrics>>>,

    /// Degradation thresholds
    thresholds: DegradationThresholds,

    /// Monitoring enabled flag
    monitoring_enabled: Arc<RwLock<HashMap<AdapterType, bool>>>,
}

impl AdapterHealthMonitor {
    /// Create new health monitor
    pub fn new(thresholds: DegradationThresholds) -> Self {
        Self {
            baseline: Arc::new(RwLock::new(HashMap::new())),
            current: Arc::new(RwLock::new(HashMap::new())),
            thresholds,
            monitoring_enabled: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Capture baseline metrics before update
    pub async fn capture_baseline(&self, adapter_type: AdapterType, metrics: HealthMetrics) {
        let mut baseline = self.baseline.write().await;
        baseline.insert(adapter_type, metrics);
    }

    /// Start monitoring after update
    pub async fn start_monitoring(&self, adapter_type: AdapterType) {
        let mut current = self.current.write().await;
        current.insert(adapter_type, HealthMetrics::new());

        let mut enabled = self.monitoring_enabled.write().await;
        enabled.insert(adapter_type, true);
    }

    /// Stop monitoring
    pub async fn stop_monitoring(&self, adapter_type: AdapterType) {
        let mut enabled = self.monitoring_enabled.write().await;
        enabled.insert(adapter_type, false);
    }

    /// Record a successful operation
    pub async fn record_success(&self, adapter_type: AdapterType, latency_ms: u64) {
        let enabled = {
            let monitoring = self.monitoring_enabled.read().await;
            monitoring.get(&adapter_type).copied().unwrap_or(false)
        };

        if enabled {
            let mut current = self.current.write().await;
            if let Some(metrics) = current.get_mut(&adapter_type) {
                metrics.record_success(latency_ms);
            }
        }
    }

    /// Record a failed operation
    pub async fn record_failure(&self, adapter_type: AdapterType) {
        let enabled = {
            let monitoring = self.monitoring_enabled.read().await;
            monitoring.get(&adapter_type).copied().unwrap_or(false)
        };

        if enabled {
            let mut current = self.current.write().await;
            if let Some(metrics) = current.get_mut(&adapter_type) {
                metrics.record_failure();
            }
        }
    }

    /// Record a crash
    pub async fn record_crash(&self, adapter_type: AdapterType) {
        let enabled = {
            let monitoring = self.monitoring_enabled.read().await;
            monitoring.get(&adapter_type).copied().unwrap_or(false)
        };

        if enabled {
            let mut current = self.current.write().await;
            if let Some(metrics) = current.get_mut(&adapter_type) {
                metrics.record_crash();
            }
        }
    }

    /// Check if adapter has degraded and should be rolled back
    pub async fn is_degraded(&self, adapter_type: AdapterType) -> (bool, String) {
        let baseline = self.baseline.read().await;
        let current = self.current.read().await;

        let baseline_metrics = match baseline.get(&adapter_type) {
            Some(m) => m,
            None => return (false, "No baseline metrics".to_string()),
        };

        let current_metrics = match current.get(&adapter_type) {
            Some(m) => m,
            None => return (false, "No current metrics".to_string()),
        };

        // Need minimum operations for valid comparison
        if current_metrics.total_operations < self.thresholds.min_operations {
            return (false, "Insufficient operations for evaluation".to_string());
        }

        // Check for crashes
        if self.thresholds.crash_triggers_rollback && current_metrics.crash_count > 0 {
            return (
                true,
                format!("Crashes detected: {} crashes", current_metrics.crash_count),
            );
        }

        // Check success rate drop
        let baseline_success = baseline_metrics.success_rate();
        let current_success = current_metrics.success_rate();
        let success_drop = baseline_success - current_success;

        if success_drop > self.thresholds.max_success_rate_drop {
            return (
                true,
                format!(
                    "Success rate dropped {:.1}% (baseline: {:.1}%, current: {:.1}%)",
                    success_drop * 100.0,
                    baseline_success * 100.0,
                    current_success * 100.0
                ),
            );
        }

        // Check latency increase
        let baseline_latency = baseline_metrics.average_latency_ms();
        let current_latency = current_metrics.average_latency_ms();

        if baseline_latency > 0.0 {
            let latency_increase = (current_latency - baseline_latency) / baseline_latency;

            if latency_increase > self.thresholds.max_latency_increase {
                return (
                    true,
                    format!(
                        "Latency increased {:.1}% (baseline: {:.1}ms, current: {:.1}ms)",
                        latency_increase * 100.0,
                        baseline_latency,
                        current_latency
                    ),
                );
            }
        }

        // Check error rate increase
        let baseline_errors = baseline_metrics.error_count();
        let current_errors = current_metrics.error_count();

        if baseline_errors > 0 {
            let error_multiplier = current_errors as f64 / baseline_errors as f64;

            if error_multiplier > self.thresholds.max_error_rate_multiplier {
                return (
                    true,
                    format!(
                        "Error rate increased {:.1}x (baseline: {} errors, current: {} errors)",
                        error_multiplier, baseline_errors, current_errors
                    ),
                );
            }
        }

        (false, "No degradation detected".to_string())
    }

    /// Get current metrics for an adapter
    pub async fn get_current_metrics(&self, adapter_type: AdapterType) -> Option<HealthMetrics> {
        let current = self.current.read().await;
        current.get(&adapter_type).cloned()
    }

    /// Get baseline metrics for an adapter
    pub async fn get_baseline_metrics(&self, adapter_type: AdapterType) -> Option<HealthMetrics> {
        let baseline = self.baseline.read().await;
        baseline.get(&adapter_type).cloned()
    }
}

/// Status of adapter loading/reloading
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterLoadStatus {
    /// Adapter is active and operational
    Active,

    /// Adapter is draining connections before reload
    Draining,

    /// Adapter is currently being reloaded
    Reloading,

    /// Adapter load failed
    Failed,

    /// Adapter is not loaded
    Unloaded,
}

/// Metadata about a loaded adapter
#[derive(Debug, Clone)]
pub struct AdapterMetadata {
    /// Adapter type
    pub adapter_type: AdapterType,

    /// Current version
    pub version: SemanticVersion,

    /// Library name
    pub library: String,

    /// When adapter was loaded
    pub loaded_at: u64,

    /// Number of times reloaded
    pub reload_count: u32,

    /// Current status
    pub status: AdapterLoadStatus,

    /// Number of active connections
    pub active_connections: u32,
}

/// Historical version entry for rollback
#[derive(Debug, Clone)]
pub struct HistoricalVersion {
    /// Version number
    pub version: SemanticVersion,

    /// Library name/path
    pub library: String,

    /// When this version was active
    pub active_at: u64,

    /// When this version was replaced
    pub replaced_at: u64,

    /// Metadata from when it was running
    pub metadata: AdapterMetadata,

    /// Path to preserved binary (for real rollback)
    /// In production, this would point to a preserved .so/.dll file
    pub binary_path: Option<String>,
}

/// Configuration for rollback history
#[derive(Debug, Clone)]
pub struct RollbackHistoryConfig {
    /// Maximum number of versions to keep per adapter
    pub max_history_depth: usize,

    /// Whether to preserve binaries on disk
    pub preserve_binaries: bool,

    /// Directory for storing historical binaries
    pub binary_storage_path: Option<String>,
}

impl Default for RollbackHistoryConfig {
    fn default() -> Self {
        Self {
            max_history_depth: 5,      // Keep last 5 versions
            preserve_binaries: false,  // Disabled by default (requires disk space)
            binary_storage_path: None, // No storage path by default
        }
    }
}

/// Rollback history manager
pub struct RollbackHistory {
    /// Historical versions per adapter
    history: Arc<RwLock<HashMap<AdapterType, Vec<HistoricalVersion>>>>,

    /// Configuration
    config: RollbackHistoryConfig,
}

impl RollbackHistory {
    /// Create new rollback history manager
    pub fn new(config: RollbackHistoryConfig) -> Self {
        Self {
            history: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Add a version to history when it's being replaced
    pub async fn archive_version(&self, adapter_type: AdapterType, metadata: AdapterMetadata) {
        let mut history = self.history.write().await;
        let versions = history.entry(adapter_type).or_insert_with(Vec::new);

        let historical = HistoricalVersion {
            version: metadata.version.clone(),
            library: metadata.library.clone(),
            active_at: metadata.loaded_at,
            replaced_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: metadata.clone(),
            binary_path: None, // TODO: Implement binary preservation
        };

        versions.push(historical);

        // Enforce max history depth
        if versions.len() > self.config.max_history_depth {
            let removed = versions.remove(0);
            log::debug!(
                "Removing old version {} from history for {:?}",
                removed.version,
                adapter_type
            );

            // TODO: Clean up preserved binary if it exists
            if let Some(path) = removed.binary_path {
                log::debug!("Would delete preserved binary at: {}", path);
            }
        }
    }

    /// Get all historical versions for an adapter
    pub async fn get_history(&self, adapter_type: AdapterType) -> Vec<HistoricalVersion> {
        let history = self.history.read().await;
        history.get(&adapter_type).cloned().unwrap_or_default()
    }

    /// Get a specific historical version
    pub async fn get_version(
        &self,
        adapter_type: AdapterType,
        version: &SemanticVersion,
    ) -> Option<HistoricalVersion> {
        let history = self.history.read().await;
        history
            .get(&adapter_type)?
            .iter()
            .find(|v| &v.version == version)
            .cloned()
    }

    /// Get the most recent historical version (for simple rollback)
    pub async fn get_previous_version(
        &self,
        adapter_type: AdapterType,
    ) -> Option<HistoricalVersion> {
        let history = self.history.read().await;
        history.get(&adapter_type)?.last().cloned()
    }

    /// Get the Nth most recent version (0 = most recent, 1 = second most recent, etc.)
    pub async fn get_nth_previous_version(
        &self,
        adapter_type: AdapterType,
        n: usize,
    ) -> Option<HistoricalVersion> {
        let history = self.history.read().await;
        let versions = history.get(&adapter_type)?;

        if versions.is_empty() || n >= versions.len() {
            return None;
        }

        // Get from the end (most recent first)
        let index = versions.len() - 1 - n;
        versions.get(index).cloned()
    }

    /// Clear all history for an adapter
    pub async fn clear_history(&self, adapter_type: AdapterType) {
        let mut history = self.history.write().await;
        if let Some(versions) = history.remove(&adapter_type) {
            log::info!(
                "Cleared {} historical versions for {:?}",
                versions.len(),
                adapter_type
            );

            // TODO: Clean up preserved binaries
            for version in versions {
                if let Some(path) = version.binary_path {
                    log::debug!("Would delete preserved binary at: {}", path);
                }
            }
        }
    }

    /// Get total number of versions in history for an adapter
    pub async fn history_depth(&self, adapter_type: AdapterType) -> usize {
        let history = self.history.read().await;
        history.get(&adapter_type).map(|v| v.len()).unwrap_or(0)
    }

    /// Get configuration
    pub fn config(&self) -> &RollbackHistoryConfig {
        &self.config
    }
}

/// Registry for managing hot-reloadable adapters
pub struct AdapterRegistry {
    /// Loaded adapters
    adapters: Arc<RwLock<HashMap<AdapterType, Box<dyn NetworkAdapter>>>>,

    /// Adapter metadata
    metadata: Arc<RwLock<HashMap<AdapterType, AdapterMetadata>>>,

    /// Connection counters
    connection_counts: Arc<RwLock<HashMap<AdapterType, u32>>>,

    /// Previous versions for rollback
    previous_versions: Arc<RwLock<HashMap<AdapterType, SemanticVersion>>>,

    /// Health monitor for automatic rollback
    health_monitor: Option<Arc<AdapterHealthMonitor>>,

    /// Auto-rollback enabled flag
    auto_rollback_enabled: Arc<RwLock<HashMap<AdapterType, bool>>>,

    /// Rollback history manager
    rollback_history: Option<Arc<RollbackHistory>>,
}

impl AdapterRegistry {
    /// Create a new adapter registry
    pub fn new() -> Self {
        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            connection_counts: Arc::new(RwLock::new(HashMap::new())),
            previous_versions: Arc::new(RwLock::new(HashMap::new())),
            health_monitor: None,
            auto_rollback_enabled: Arc::new(RwLock::new(HashMap::new())),
            rollback_history: None,
        }
    }

    /// Create a new adapter registry with health monitoring enabled
    pub fn with_health_monitoring(thresholds: DegradationThresholds) -> Self {
        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            connection_counts: Arc::new(RwLock::new(HashMap::new())),
            previous_versions: Arc::new(RwLock::new(HashMap::new())),
            health_monitor: Some(Arc::new(AdapterHealthMonitor::new(thresholds))),
            auto_rollback_enabled: Arc::new(RwLock::new(HashMap::new())),
            rollback_history: None,
        }
    }

    /// Create a new adapter registry with full features (health monitoring + rollback history)
    pub fn with_full_features(
        thresholds: DegradationThresholds,
        history_config: RollbackHistoryConfig,
    ) -> Self {
        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            connection_counts: Arc::new(RwLock::new(HashMap::new())),
            previous_versions: Arc::new(RwLock::new(HashMap::new())),
            health_monitor: Some(Arc::new(AdapterHealthMonitor::new(thresholds))),
            auto_rollback_enabled: Arc::new(RwLock::new(HashMap::new())),
            rollback_history: Some(Arc::new(RollbackHistory::new(history_config))),
        }
    }

    /// Enable automatic rollback for an adapter
    pub async fn enable_auto_rollback(&self, adapter_type: AdapterType) {
        let mut enabled = self.auto_rollback_enabled.write().await;
        enabled.insert(adapter_type, true);
    }

    /// Disable automatic rollback for an adapter
    pub async fn disable_auto_rollback(&self, adapter_type: AdapterType) {
        let mut enabled = self.auto_rollback_enabled.write().await;
        enabled.insert(adapter_type, false);
    }

    /// Check if auto-rollback is enabled for an adapter
    pub async fn is_auto_rollback_enabled(&self, adapter_type: AdapterType) -> bool {
        let enabled = self.auto_rollback_enabled.read().await;
        enabled.get(&adapter_type).copied().unwrap_or(false)
    }

    /// Register a new adapter
    pub async fn register_adapter(
        &self,
        adapter_type: AdapterType,
        adapter: Box<dyn NetworkAdapter>,
        version: SemanticVersion,
        library: String,
    ) -> Result<()> {
        let mut adapters = self.adapters.write().await;
        let mut metadata = self.metadata.write().await;

        let meta = AdapterMetadata {
            adapter_type,
            version,
            library,
            loaded_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            reload_count: 0,
            status: AdapterLoadStatus::Active,
            active_connections: 0,
        };

        adapters.insert(adapter_type, adapter);
        metadata.insert(adapter_type, meta);

        Ok(())
    }

    /// Hot reload a specific adapter
    ///
    /// This performs a graceful reload:
    /// 1. Archive current version to rollback history
    /// 2. Set old adapter to draining
    /// 3. Wait for connections to finish (with timeout)
    /// 4. Swap adapters atomically
    /// 5. Initialize and start new adapter
    pub async fn hot_reload_adapter(
        &self,
        adapter_type: AdapterType,
        new_adapter: Box<dyn NetworkAdapter>,
        new_version: SemanticVersion,
    ) -> Result<()> {
        log::info!(
            "Starting hot reload of {:?} to version {}",
            adapter_type,
            new_version
        );

        // Archive current version to rollback history
        {
            let metadata = self.metadata.read().await;
            if let Some(meta) = metadata.get(&adapter_type) {
                // Save to previous_versions for simple rollback
                let mut previous = self.previous_versions.write().await;
                previous.insert(adapter_type, meta.version.clone());

                // Archive to rollback history if enabled
                if let Some(history) = &self.rollback_history {
                    history.archive_version(adapter_type, meta.clone()).await;
                    log::debug!(
                        "Archived version {} to rollback history for {:?}",
                        meta.version,
                        adapter_type
                    );
                }
            }
        }

        // Set adapter to draining
        {
            let mut metadata = self.metadata.write().await;
            if let Some(meta) = metadata.get_mut(&adapter_type) {
                meta.status = AdapterLoadStatus::Draining;
                log::info!(
                    "Draining {:?}, {} active connections",
                    adapter_type,
                    meta.active_connections
                );
            }
        }

        // Wait for connections to drain (with timeout)
        let drain_timeout = Duration::from_secs(30);
        self.drain_adapter(adapter_type, drain_timeout).await?;

        // Set to reloading status
        {
            let mut metadata = self.metadata.write().await;
            if let Some(meta) = metadata.get_mut(&adapter_type) {
                meta.status = AdapterLoadStatus::Reloading;
            }
        }

        // Stop old adapter
        {
            let mut adapters = self.adapters.write().await;
            if let Some(mut old_adapter) = adapters.remove(&adapter_type) {
                // Gracefully stop old adapter
                if let Err(e) = old_adapter.stop().await {
                    log::error!("Error stopping old adapter {:?}: {}", adapter_type, e);
                }
            }
        }

        // Insert new adapter
        {
            let mut adapters = self.adapters.write().await;
            adapters.insert(adapter_type, new_adapter);
        }

        // Initialize new adapter
        {
            let mut adapters = self.adapters.write().await;
            if let Some(adapter) = adapters.get_mut(&adapter_type) {
                adapter.initialize().await.map_err(|e| {
                    log::error!("Failed to initialize new adapter {:?}: {}", adapter_type, e);
                    NetworkError::InitializationFailed(e.to_string())
                })?;

                adapter.start().await.map_err(|e| {
                    log::error!("Failed to start new adapter {:?}: {}", adapter_type, e);
                    NetworkError::InitializationFailed(e.to_string())
                })?;
            }
        }

        // Update metadata
        {
            let mut metadata = self.metadata.write().await;
            if let Some(meta) = metadata.get_mut(&adapter_type) {
                meta.version = new_version;
                meta.loaded_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                meta.reload_count += 1;
                meta.status = AdapterLoadStatus::Active;
                meta.active_connections = 0;
            }
        }

        log::info!("Hot reload of {:?} completed successfully", adapter_type);
        Ok(())
    }

    /// Drain connections from an adapter
    async fn drain_adapter(&self, adapter_type: AdapterType, timeout: Duration) -> Result<()> {
        let start = Instant::now();

        loop {
            // Check connection count
            let connections = {
                let counts = self.connection_counts.read().await;
                counts.get(&adapter_type).copied().unwrap_or(0)
            };

            if connections == 0 {
                log::info!("All connections drained from {:?}", adapter_type);
                return Ok(());
            }

            if start.elapsed() > timeout {
                log::warn!(
                    "Timeout draining {:?}, {} connections remaining - proceeding anyway",
                    adapter_type,
                    connections
                );
                return Ok(());
            }

            // Wait a bit before checking again
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Increment connection counter for an adapter
    pub async fn increment_connections(&self, adapter_type: AdapterType) {
        let mut counts = self.connection_counts.write().await;
        *counts.entry(adapter_type).or_insert(0) += 1;

        // Update metadata
        let mut metadata = self.metadata.write().await;
        if let Some(meta) = metadata.get_mut(&adapter_type) {
            meta.active_connections += 1;
        }
    }

    /// Decrement connection counter for an adapter
    pub async fn decrement_connections(&self, adapter_type: AdapterType) {
        let mut counts = self.connection_counts.write().await;
        if let Some(count) = counts.get_mut(&adapter_type) {
            *count = count.saturating_sub(1);
        }

        // Update metadata
        let mut metadata = self.metadata.write().await;
        if let Some(meta) = metadata.get_mut(&adapter_type) {
            meta.active_connections = meta.active_connections.saturating_sub(1);
        }
    }

    /// Get active connection count for an adapter
    pub async fn get_active_connections(&self, adapter_type: AdapterType) -> u32 {
        let counts = self.connection_counts.read().await;
        counts.get(&adapter_type).copied().unwrap_or(0)
    }

    /// Get adapter metadata
    pub async fn get_metadata(&self, adapter_type: AdapterType) -> Option<AdapterMetadata> {
        let metadata = self.metadata.read().await;
        metadata.get(&adapter_type).cloned()
    }

    /// Get all adapter metadata
    pub async fn get_all_metadata(&self) -> Vec<AdapterMetadata> {
        let metadata = self.metadata.read().await;
        metadata.values().cloned().collect()
    }

    /// Rollback to previous version
    pub async fn rollback_adapter(&self, adapter_type: AdapterType) -> Result<()> {
        let previous_version = {
            let previous = self.previous_versions.read().await;
            previous.get(&adapter_type).cloned().ok_or_else(|| {
                NetworkError::InitializationFailed("No previous version".to_string())
            })?
        };

        log::warn!(
            "Rolling back {:?} to version {}",
            adapter_type,
            previous_version
        );

        // Note: In a real implementation, we would need to:
        // 1. Load the previous version's binary/library
        // 2. Create an adapter instance
        // 3. Call hot_reload_adapter with the old version

        // For now, this is a placeholder showing the interface
        Err(NetworkError::InitializationFailed(
            "Rollback not yet implemented - requires binary/library loading".to_string(),
        ))
    }

    /// Check for degradation and trigger automatic rollback if needed
    ///
    /// Returns Ok(true) if rollback was triggered, Ok(false) if no rollback needed,
    /// or Err if rollback failed.
    pub async fn check_and_rollback(&self, adapter_type: AdapterType) -> Result<bool> {
        // Check if auto-rollback is enabled
        if !self.is_auto_rollback_enabled(adapter_type).await {
            return Ok(false);
        }

        // Check if we have a health monitor
        let monitor = match &self.health_monitor {
            Some(m) => m,
            None => return Ok(false),
        };

        // Check for degradation
        let (degraded, reason) = monitor.is_degraded(adapter_type).await;

        if degraded {
            log::error!(
                "Degradation detected for {:?}: {}. Triggering automatic rollback.",
                adapter_type,
                reason
            );

            // Disable auto-rollback temporarily to prevent rollback loops
            self.disable_auto_rollback(adapter_type).await;

            // Attempt rollback
            match self.rollback_adapter(adapter_type).await {
                Ok(_) => {
                    log::info!("Automatic rollback succeeded for {:?}", adapter_type);
                    Ok(true)
                }
                Err(e) => {
                    log::error!("Automatic rollback failed for {:?}: {}", adapter_type, e);
                    Err(e)
                }
            }
        } else {
            Ok(false)
        }
    }

    /// Record a successful operation for health monitoring
    pub async fn record_success(&self, adapter_type: AdapterType, latency_ms: u64) {
        if let Some(monitor) = &self.health_monitor {
            monitor.record_success(adapter_type, latency_ms).await;
        }
    }

    /// Record a failed operation for health monitoring
    pub async fn record_failure(&self, adapter_type: AdapterType) {
        if let Some(monitor) = &self.health_monitor {
            monitor.record_failure(adapter_type).await;
        }
    }

    /// Record a crash for health monitoring
    pub async fn record_crash(&self, adapter_type: AdapterType) {
        if let Some(monitor) = &self.health_monitor {
            monitor.record_crash(adapter_type).await;
        }
    }

    /// Get health monitor reference
    pub fn get_health_monitor(&self) -> Option<Arc<AdapterHealthMonitor>> {
        self.health_monitor.clone()
    }

    /// Get rollback history reference
    pub fn get_rollback_history(&self) -> Option<Arc<RollbackHistory>> {
        self.rollback_history.clone()
    }

    /// Get historical versions for an adapter
    pub async fn get_version_history(&self, adapter_type: AdapterType) -> Vec<HistoricalVersion> {
        match &self.rollback_history {
            Some(history) => history.get_history(adapter_type).await,
            None => Vec::new(),
        }
    }

    /// Check if adapter is in draining state
    pub async fn is_draining(&self, adapter_type: AdapterType) -> bool {
        let metadata = self.metadata.read().await;
        metadata
            .get(&adapter_type)
            .map(|m| m.status == AdapterLoadStatus::Draining)
            .unwrap_or(false)
    }

    /// Get adapter reference
    pub async fn get_adapter(
        &self,
        _adapter_type: AdapterType,
    ) -> Option<Arc<RwLock<Box<dyn NetworkAdapter>>>> {
        // This would need adjustment to the actual implementation
        // For now, returning None as we can't easily clone trait objects
        None
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_counting() {
        let registry = AdapterRegistry::new();

        assert_eq!(
            registry.get_active_connections(AdapterType::Ethernet).await,
            0
        );

        registry.increment_connections(AdapterType::Ethernet).await;
        assert_eq!(
            registry.get_active_connections(AdapterType::Ethernet).await,
            1
        );

        registry.increment_connections(AdapterType::Ethernet).await;
        assert_eq!(
            registry.get_active_connections(AdapterType::Ethernet).await,
            2
        );

        registry.decrement_connections(AdapterType::Ethernet).await;
        assert_eq!(
            registry.get_active_connections(AdapterType::Ethernet).await,
            1
        );
    }

    #[tokio::test]
    async fn test_drain_no_connections() {
        let registry = AdapterRegistry::new();

        // Should complete immediately with no connections
        let result = registry
            .drain_adapter(AdapterType::Ethernet, Duration::from_secs(1))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_metadata_tracking() {
        let registry = AdapterRegistry::new();

        let version = SemanticVersion::new(1, 0, 0);

        // Initially no metadata
        assert!(registry.get_metadata(AdapterType::Ethernet).await.is_none());

        // After setting metadata
        {
            let mut metadata = registry.metadata.write().await;
            metadata.insert(
                AdapterType::Ethernet,
                AdapterMetadata {
                    adapter_type: AdapterType::Ethernet,
                    version: version.clone(),
                    library: "tokio".to_string(),
                    loaded_at: 12345,
                    reload_count: 0,
                    status: AdapterLoadStatus::Active,
                    active_connections: 0,
                },
            );
        }

        let meta = registry.get_metadata(AdapterType::Ethernet).await;
        assert!(meta.is_some());
        assert_eq!(meta.unwrap().version, version);
    }

    #[tokio::test]
    async fn test_health_metrics_success_rate() {
        let mut metrics = HealthMetrics::new();

        // Initially 100% success (no operations)
        assert_eq!(metrics.success_rate(), 1.0);

        // Record some successes
        metrics.record_success(10);
        metrics.record_success(15);
        metrics.record_success(20);

        assert_eq!(metrics.success_rate(), 1.0);
        assert_eq!(metrics.total_operations, 3);

        // Record a failure
        metrics.record_failure();

        assert_eq!(metrics.success_rate(), 0.75); // 3/4 = 75%
        assert_eq!(metrics.total_operations, 4);
    }

    #[tokio::test]
    async fn test_health_metrics_latency() {
        let mut metrics = HealthMetrics::new();

        // Initially 0 latency
        assert_eq!(metrics.average_latency_ms(), 0.0);

        // Record latencies
        metrics.record_success(10);
        metrics.record_success(20);
        metrics.record_success(30);

        assert_eq!(metrics.average_latency_ms(), 20.0); // (10+20+30)/3 = 20
    }

    #[tokio::test]
    async fn test_health_monitor_no_degradation() {
        let monitor = AdapterHealthMonitor::new(DegradationThresholds::default());

        // Capture baseline
        let mut baseline = HealthMetrics::new();
        baseline.record_success(10);
        baseline.record_success(12);
        baseline.record_success(11);
        monitor
            .capture_baseline(AdapterType::Ethernet, baseline)
            .await;

        // Start monitoring
        monitor.start_monitoring(AdapterType::Ethernet).await;

        // Record similar performance
        for _ in 0..10 {
            monitor.record_success(AdapterType::Ethernet, 11).await;
        }

        // Should not be degraded
        let (degraded, reason) = monitor.is_degraded(AdapterType::Ethernet).await;
        assert!(!degraded, "Should not be degraded: {}", reason);
    }

    #[tokio::test]
    async fn test_health_monitor_success_rate_degradation() {
        let monitor = AdapterHealthMonitor::new(DegradationThresholds::default());

        // Capture baseline with 100% success
        let mut baseline = HealthMetrics::new();
        for _ in 0..10 {
            baseline.record_success(10);
        }
        monitor
            .capture_baseline(AdapterType::Ethernet, baseline)
            .await;

        // Start monitoring
        monitor.start_monitoring(AdapterType::Ethernet).await;

        // Record degraded performance (50% success rate)
        for _ in 0..5 {
            monitor.record_success(AdapterType::Ethernet, 10).await;
        }
        for _ in 0..5 {
            monitor.record_failure(AdapterType::Ethernet).await;
        }

        // Should be degraded (50% drop is > 10% threshold)
        let (degraded, reason) = monitor.is_degraded(AdapterType::Ethernet).await;
        assert!(degraded, "Should be degraded due to success rate drop");
        assert!(reason.contains("Success rate dropped"));
    }

    #[tokio::test]
    async fn test_health_monitor_latency_degradation() {
        let monitor = AdapterHealthMonitor::new(DegradationThresholds::default());

        // Capture baseline with 10ms latency
        let mut baseline = HealthMetrics::new();
        for _ in 0..10 {
            baseline.record_success(10);
        }
        monitor
            .capture_baseline(AdapterType::Ethernet, baseline)
            .await;

        // Start monitoring
        monitor.start_monitoring(AdapterType::Ethernet).await;

        // Record 100% increase in latency (20ms)
        for _ in 0..10 {
            monitor.record_success(AdapterType::Ethernet, 20).await;
        }

        // Should be degraded (100% increase is > 50% threshold)
        let (degraded, reason) = monitor.is_degraded(AdapterType::Ethernet).await;
        assert!(degraded, "Should be degraded due to latency increase");
        assert!(reason.contains("Latency increased"));
    }

    #[tokio::test]
    async fn test_health_monitor_crash_degradation() {
        let monitor = AdapterHealthMonitor::new(DegradationThresholds::default());

        // Capture baseline
        let baseline = HealthMetrics::new();
        monitor
            .capture_baseline(AdapterType::Ethernet, baseline)
            .await;

        // Start monitoring
        monitor.start_monitoring(AdapterType::Ethernet).await;

        // Record operations then a crash
        for _ in 0..10 {
            monitor.record_success(AdapterType::Ethernet, 10).await;
        }
        monitor.record_crash(AdapterType::Ethernet).await;

        // Should be degraded due to crash
        let (degraded, reason) = monitor.is_degraded(AdapterType::Ethernet).await;
        assert!(degraded, "Should be degraded due to crash");
        assert!(reason.contains("Crashes detected"));
    }

    #[tokio::test]
    async fn test_health_monitor_insufficient_operations() {
        let monitor = AdapterHealthMonitor::new(DegradationThresholds::default());

        // Capture baseline
        let mut baseline = HealthMetrics::new();
        baseline.record_success(10);
        monitor
            .capture_baseline(AdapterType::Ethernet, baseline)
            .await;

        // Start monitoring
        monitor.start_monitoring(AdapterType::Ethernet).await;

        // Record only a few operations (less than min_operations threshold of 10)
        for _ in 0..5 {
            monitor.record_failure(AdapterType::Ethernet).await;
        }

        // Should not be degraded yet (insufficient data)
        let (degraded, _) = monitor.is_degraded(AdapterType::Ethernet).await;
        assert!(
            !degraded,
            "Should not evaluate with insufficient operations"
        );
    }

    #[tokio::test]
    async fn test_auto_rollback_disabled_by_default() {
        let registry = AdapterRegistry::new();

        assert!(
            !registry
                .is_auto_rollback_enabled(AdapterType::Ethernet)
                .await
        );
    }

    #[tokio::test]
    async fn test_enable_auto_rollback() {
        let registry = AdapterRegistry::new();

        registry.enable_auto_rollback(AdapterType::Ethernet).await;
        assert!(
            registry
                .is_auto_rollback_enabled(AdapterType::Ethernet)
                .await
        );

        registry.disable_auto_rollback(AdapterType::Ethernet).await;
        assert!(
            !registry
                .is_auto_rollback_enabled(AdapterType::Ethernet)
                .await
        );
    }

    #[tokio::test]
    async fn test_check_and_rollback_no_monitor() {
        let registry = AdapterRegistry::new();

        // Enable auto-rollback
        registry.enable_auto_rollback(AdapterType::Ethernet).await;

        // Should return false (no health monitor)
        let result = registry.check_and_rollback(AdapterType::Ethernet).await;
        assert!(result.is_ok());
        assert!(!result.unwrap(), "Should not rollback without monitor");
    }

    #[tokio::test]
    async fn test_check_and_rollback_disabled() {
        let registry = AdapterRegistry::with_health_monitoring(DegradationThresholds::default());

        // Auto-rollback disabled by default
        let result = registry.check_and_rollback(AdapterType::Ethernet).await;
        assert!(result.is_ok());
        assert!(!result.unwrap(), "Should not rollback when disabled");
    }

    #[tokio::test]
    async fn test_record_operations_with_monitor() {
        let registry = AdapterRegistry::with_health_monitoring(DegradationThresholds::default());

        // Should not panic when recording operations
        registry.record_success(AdapterType::Ethernet, 10).await;
        registry.record_failure(AdapterType::Ethernet).await;
        registry.record_crash(AdapterType::Ethernet).await;

        // Verify monitor is accessible
        assert!(registry.get_health_monitor().is_some());
    }

    #[tokio::test]
    async fn test_record_operations_without_monitor() {
        let registry = AdapterRegistry::new();

        // Should not panic when recording operations without monitor
        registry.record_success(AdapterType::Ethernet, 10).await;
        registry.record_failure(AdapterType::Ethernet).await;
        registry.record_crash(AdapterType::Ethernet).await;

        // Verify no monitor
        assert!(registry.get_health_monitor().is_none());
    }

    #[tokio::test]
    async fn test_rollback_history_archiving() {
        let config = RollbackHistoryConfig::default();
        let history = RollbackHistory::new(config);

        // Initially empty
        assert_eq!(history.history_depth(AdapterType::Ethernet).await, 0);

        // Archive a version
        let metadata = AdapterMetadata {
            adapter_type: AdapterType::Ethernet,
            version: SemanticVersion::new(1, 0, 0),
            library: "test-lib".to_string(),
            loaded_at: 1000,
            reload_count: 0,
            status: AdapterLoadStatus::Active,
            active_connections: 0,
        };

        history
            .archive_version(AdapterType::Ethernet, metadata.clone())
            .await;

        // Should have 1 entry
        assert_eq!(history.history_depth(AdapterType::Ethernet).await, 1);

        // Get the version
        let versions = history.get_history(AdapterType::Ethernet).await;
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, SemanticVersion::new(1, 0, 0));
    }

    #[tokio::test]
    async fn test_rollback_history_max_depth() {
        let config = RollbackHistoryConfig {
            max_history_depth: 3,
            preserve_binaries: false,
            binary_storage_path: None,
        };
        let history = RollbackHistory::new(config);

        // Archive 5 versions
        for i in 0..5 {
            let metadata = AdapterMetadata {
                adapter_type: AdapterType::Ethernet,
                version: SemanticVersion::new(1, i, 0),
                library: format!("test-lib-{}", i),
                loaded_at: 1000 + i as u64,
                reload_count: i,
                status: AdapterLoadStatus::Active,
                active_connections: 0,
            };

            history
                .archive_version(AdapterType::Ethernet, metadata)
                .await;
        }

        // Should only keep last 3
        assert_eq!(history.history_depth(AdapterType::Ethernet).await, 3);

        // Verify the kept versions are the most recent ones (v1.2.0, v1.3.0, v1.4.0)
        let versions = history.get_history(AdapterType::Ethernet).await;
        assert_eq!(versions[0].version, SemanticVersion::new(1, 2, 0));
        assert_eq!(versions[1].version, SemanticVersion::new(1, 3, 0));
        assert_eq!(versions[2].version, SemanticVersion::new(1, 4, 0));
    }

    #[tokio::test]
    async fn test_rollback_history_get_specific_version() {
        let history = RollbackHistory::new(RollbackHistoryConfig::default());

        // Archive multiple versions
        for i in 0..3 {
            let metadata = AdapterMetadata {
                adapter_type: AdapterType::Ethernet,
                version: SemanticVersion::new(1, i, 0),
                library: format!("test-lib-{}", i),
                loaded_at: 1000 + i as u64,
                reload_count: i,
                status: AdapterLoadStatus::Active,
                active_connections: 0,
            };

            history
                .archive_version(AdapterType::Ethernet, metadata)
                .await;
        }

        // Get specific version
        let v1_1_0 = history
            .get_version(AdapterType::Ethernet, &SemanticVersion::new(1, 1, 0))
            .await;
        assert!(v1_1_0.is_some());
        assert_eq!(v1_1_0.unwrap().version, SemanticVersion::new(1, 1, 0));

        // Non-existent version
        let v9_9_9 = history
            .get_version(AdapterType::Ethernet, &SemanticVersion::new(9, 9, 9))
            .await;
        assert!(v9_9_9.is_none());
    }

    #[tokio::test]
    async fn test_rollback_history_get_nth_version() {
        let history = RollbackHistory::new(RollbackHistoryConfig::default());

        // Archive 3 versions
        for i in 0..3 {
            let metadata = AdapterMetadata {
                adapter_type: AdapterType::Ethernet,
                version: SemanticVersion::new(1, i, 0),
                library: format!("test-lib-{}", i),
                loaded_at: 1000 + i as u64,
                reload_count: i,
                status: AdapterLoadStatus::Active,
                active_connections: 0,
            };

            history
                .archive_version(AdapterType::Ethernet, metadata)
                .await;
        }

        // Get most recent (n=0) should be v1.2.0
        let most_recent = history
            .get_nth_previous_version(AdapterType::Ethernet, 0)
            .await;
        assert_eq!(most_recent.unwrap().version, SemanticVersion::new(1, 2, 0));

        // Get second most recent (n=1) should be v1.1.0
        let second = history
            .get_nth_previous_version(AdapterType::Ethernet, 1)
            .await;
        assert_eq!(second.unwrap().version, SemanticVersion::new(1, 1, 0));

        // Get third most recent (n=2) should be v1.0.0
        let third = history
            .get_nth_previous_version(AdapterType::Ethernet, 2)
            .await;
        assert_eq!(third.unwrap().version, SemanticVersion::new(1, 0, 0));

        // n=3 should be out of range
        let fourth = history
            .get_nth_previous_version(AdapterType::Ethernet, 3)
            .await;
        assert!(fourth.is_none());
    }

    #[tokio::test]
    async fn test_rollback_history_clear() {
        let history = RollbackHistory::new(RollbackHistoryConfig::default());

        // Archive some versions
        for i in 0..3 {
            let metadata = AdapterMetadata {
                adapter_type: AdapterType::Ethernet,
                version: SemanticVersion::new(1, i, 0),
                library: format!("test-lib-{}", i),
                loaded_at: 1000 + i as u64,
                reload_count: i,
                status: AdapterLoadStatus::Active,
                active_connections: 0,
            };

            history
                .archive_version(AdapterType::Ethernet, metadata)
                .await;
        }

        assert_eq!(history.history_depth(AdapterType::Ethernet).await, 3);

        // Clear history
        history.clear_history(AdapterType::Ethernet).await;
        assert_eq!(history.history_depth(AdapterType::Ethernet).await, 0);
    }

    #[tokio::test]
    async fn test_registry_with_rollback_history() {
        let registry = AdapterRegistry::with_full_features(
            DegradationThresholds::default(),
            RollbackHistoryConfig::default(),
        );

        // Should have both health monitor and rollback history
        assert!(registry.get_health_monitor().is_some());
        assert!(registry.get_rollback_history().is_some());
    }
}
