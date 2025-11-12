//! Network adapter manager

use crate::adapter::{NetworkAdapter, AdapterStatus};
use crate::error::{NetworkError, Result};
use crate::metrics::AdapterMetrics;
use crate::types::{Address, AdapterCapabilities};
use myriadmesh_protocol::types::AdapterType;
use myriadmesh_protocol::Frame;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Unique identifier for an adapter instance
pub type AdapterId = String;

/// Adapter manager for managing multiple network adapters
pub struct AdapterManager {
    /// Registered adapters
    adapters: HashMap<AdapterId, Arc<RwLock<Box<dyn NetworkAdapter>>>>,

    /// Adapter performance metrics
    metrics: HashMap<AdapterId, AdapterMetrics>,

    /// Adapter capabilities cache
    capabilities: HashMap<AdapterId, AdapterCapabilities>,
}

impl AdapterManager {
    /// Create a new adapter manager
    pub fn new() -> Self {
        AdapterManager {
            adapters: HashMap::new(),
            metrics: HashMap::new(),
            capabilities: HashMap::new(),
        }
    }

    /// Register a new adapter
    pub async fn register_adapter(
        &mut self,
        id: AdapterId,
        mut adapter: Box<dyn NetworkAdapter>,
    ) -> Result<()> {
        // Check if already registered
        if self.adapters.contains_key(&id) {
            return Err(NetworkError::AdapterAlreadyRegistered(id));
        }

        // Initialize and start adapter
        adapter
            .initialize()
            .await
            .map_err(|e| NetworkError::InitializationFailed(e.to_string()))?;

        adapter
            .start()
            .await
            .map_err(|e| NetworkError::InitializationFailed(e.to_string()))?;

        // Cache capabilities
        let capabilities = adapter.get_capabilities().clone();
        self.capabilities.insert(id.clone(), capabilities);

        // Initialize metrics
        self.metrics.insert(id.clone(), AdapterMetrics::new());

        // Store adapter
        self.adapters
            .insert(id, Arc::new(RwLock::new(adapter)));

        Ok(())
    }

    /// Unregister an adapter
    pub async fn unregister_adapter(&mut self, id: &str) -> Result<()> {
        if let Some(adapter) = self.adapters.remove(id) {
            let mut adapter = adapter.write().await;
            adapter.stop().await?;
            self.metrics.remove(id);
            self.capabilities.remove(id);
            Ok(())
        } else {
            Err(NetworkError::AdapterNotFound(id.to_string()))
        }
    }

    /// Get adapter by ID
    pub fn get_adapter(&self, id: &str) -> Option<Arc<RwLock<Box<dyn NetworkAdapter>>>> {
        self.adapters.get(id).cloned()
    }

    /// Get adapter capabilities
    pub fn get_capabilities(&self, id: &str) -> Option<&AdapterCapabilities> {
        self.capabilities.get(id)
    }

    /// Get adapter metrics
    pub fn get_metrics(&self, id: &str) -> Option<&AdapterMetrics> {
        self.metrics.get(id)
    }

    /// Get mutable adapter metrics
    pub fn get_metrics_mut(&mut self, id: &str) -> Option<&mut AdapterMetrics> {
        self.metrics.get_mut(id)
    }

    /// Get all adapter IDs
    pub fn adapter_ids(&self) -> Vec<AdapterId> {
        self.adapters.keys().cloned().collect()
    }

    /// Get number of registered adapters
    pub fn adapter_count(&self) -> usize {
        self.adapters.len()
    }

    /// Check if any adapters are available
    pub fn has_adapters(&self) -> bool {
        !self.adapters.is_empty()
    }

    /// Select best adapter for sending a frame
    pub fn select_best_adapter(&self, frame: &Frame, priority: u8) -> Option<AdapterId> {
        if self.adapters.is_empty() {
            return None;
        }

        let mut best_adapter = None;
        let mut best_score = f64::MIN;

        for (id, _adapter) in &self.adapters {
            if let Some(caps) = self.capabilities.get(id) {
                // Check if adapter can handle message size
                if frame.size() > caps.max_message_size {
                    continue;
                }

                // Calculate score
                let score = caps.calculate_score(frame.size(), priority);

                // Adjust score based on current metrics
                if let Some(metrics) = self.metrics.get(id) {
                    let adjusted_score = score * metrics.reliability;

                    if adjusted_score > best_score {
                        best_score = adjusted_score;
                        best_adapter = Some(id.clone());
                    }
                }
            }
        }

        best_adapter
    }

    /// Find adapter by type
    pub fn find_adapter_by_type(&self, adapter_type: AdapterType) -> Option<AdapterId> {
        for (id, caps) in &self.capabilities {
            if caps.adapter_type == adapter_type {
                return Some(id.clone());
            }
        }
        None
    }

    /// Get adapters by status
    pub async fn get_adapters_by_status(&self, status: AdapterStatus) -> Vec<AdapterId> {
        let mut result = Vec::new();

        for (id, adapter) in &self.adapters {
            let adapter = adapter.read().await;
            if adapter.get_status() == status {
                result.push(id.clone());
            }
        }

        result
    }

    /// Health check all adapters
    pub async fn health_check_all(&mut self) -> HashMap<AdapterId, AdapterStatus> {
        let mut statuses = HashMap::new();

        for (id, adapter) in &self.adapters {
            let adapter = adapter.read().await;
            let status = adapter.get_status();
            statuses.insert(id.clone(), status);
        }

        statuses
    }

    /// Stop all adapters
    pub async fn stop_all(&mut self) -> Result<()> {
        let ids: Vec<_> = self.adapters.keys().cloned().collect();

        for id in ids {
            if let Err(e) = self.unregister_adapter(&id).await {
                eprintln!("Error stopping adapter {}: {}", id, e);
            }
        }

        Ok(())
    }
}

impl Default for AdapterManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock adapter for testing
    struct MockAdapter {
        status: AdapterStatus,
        capabilities: AdapterCapabilities,
    }

    #[async_trait::async_trait]
    impl NetworkAdapter for MockAdapter {
        async fn initialize(&mut self) -> Result<()> {
            self.status = AdapterStatus::Ready;
            Ok(())
        }

        async fn start(&mut self) -> Result<()> {
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            self.status = AdapterStatus::ShuttingDown;
            Ok(())
        }

        async fn send(&self, _destination: &Address, _frame: &Frame) -> Result<()> {
            Ok(())
        }

        async fn receive(&self, _timeout_ms: u64) -> Result<(Address, Frame)> {
            Err(NetworkError::ReceiveFailed("Not implemented".to_string()))
        }

        async fn discover_peers(&self) -> Result<Vec<crate::adapter::PeerInfo>> {
            Ok(Vec::new())
        }

        fn get_status(&self) -> AdapterStatus {
            self.status
        }

        fn get_capabilities(&self) -> &AdapterCapabilities {
            &self.capabilities
        }

        async fn test_connection(&self, _destination: &Address) -> Result<crate::adapter::TestResults> {
            Ok(crate::adapter::TestResults {
                success: true,
                rtt_ms: Some(10.0),
                error: None,
            })
        }

        fn get_local_address(&self) -> Option<Address> {
            None
        }

        fn parse_address(&self, addr_str: &str) -> Result<Address> {
            Ok(Address::Unknown(addr_str.to_string()))
        }

        fn supports_address(&self, _address: &Address) -> bool {
            true
        }
    }

    fn create_mock_adapter() -> MockAdapter {
        use crate::types::PowerConsumption;

        MockAdapter {
            status: AdapterStatus::Uninitialized,
            capabilities: AdapterCapabilities {
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
            },
        }
    }

    #[tokio::test]
    async fn test_register_adapter() {
        let mut manager = AdapterManager::new();
        let adapter = Box::new(create_mock_adapter());

        manager
            .register_adapter("test".to_string(), adapter)
            .await
            .unwrap();

        assert_eq!(manager.adapter_count(), 1);
        assert!(manager.has_adapters());
    }

    #[tokio::test]
    async fn test_unregister_adapter() {
        let mut manager = AdapterManager::new();
        let adapter = Box::new(create_mock_adapter());

        manager
            .register_adapter("test".to_string(), adapter)
            .await
            .unwrap();

        manager.unregister_adapter("test").await.unwrap();
        assert_eq!(manager.adapter_count(), 0);
    }

    #[tokio::test]
    async fn test_find_adapter_by_type() {
        let mut manager = AdapterManager::new();
        let adapter = Box::new(create_mock_adapter());

        manager
            .register_adapter("test".to_string(), adapter)
            .await
            .unwrap();

        let found = manager.find_adapter_by_type(AdapterType::Ethernet);
        assert!(found.is_some());
        assert_eq!(found.unwrap(), "test");
    }
}
