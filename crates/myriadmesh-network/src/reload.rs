//! Hot-reloadable adapter registry
//!
//! Allows updating individual network adapters without taking down
//! the entire node. Coordinates graceful connection draining and
//! atomic adapter swapping.

use crate::adapter::{AdapterStatus, NetworkAdapter};
use crate::error::{NetworkError, Result};
use crate::version_tracking::SemanticVersion;
use myriadmesh_protocol::types::AdapterType;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

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
}

impl AdapterRegistry {
    /// Create a new adapter registry
    pub fn new() -> Self {
        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            connection_counts: Arc::new(RwLock::new(HashMap::new())),
            previous_versions: Arc::new(RwLock::new(HashMap::new())),
        }
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
    /// 1. Load new adapter in parallel
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

        // Save current version for rollback
        {
            let metadata = self.metadata.read().await;
            if let Some(meta) = metadata.get(&adapter_type) {
                let mut previous = self.previous_versions.write().await;
                previous.insert(adapter_type, meta.version.clone());
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
            previous
                .get(&adapter_type)
                .cloned()
                .ok_or_else(|| NetworkError::InitializationFailed("No previous version".to_string()))?
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
        adapter_type: AdapterType,
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
}
