//! Adapter manager for coordinating multiple network adapters

use crate::adapter::{NetworkAdapter, NetworkError, Result};
use dashmap::DashMap;
use myriadmesh_protocol::{Frame, NodeId};
use myriadmesh_protocol::types::AdapterType;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Received frame with metadata
#[derive(Debug, Clone)]
pub struct ReceivedFrame {
    /// The frame itself
    pub frame: Frame,

    /// NodeId of the sender
    pub sender: NodeId,

    /// Adapter type that received this frame
    pub adapter_type: AdapterType,

    /// Adapter ID that received this frame
    pub adapter_id: String,
}

/// Configuration for the adapter manager
#[derive(Debug, Clone)]
pub struct ManagerConfig {
    /// Buffer size for incoming frames channel
    pub incoming_buffer_size: usize,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            incoming_buffer_size: 1000,
        }
    }
}

/// Manages multiple network adapters
pub struct AdapterManager {
    /// Registered adapters, keyed by adapter ID
    adapters: DashMap<String, Arc<dyn NetworkAdapter>>,

    /// Channel for incoming frames from all adapters
    incoming_tx: mpsc::Sender<ReceivedFrame>,
    incoming_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<ReceivedFrame>>>,

    /// Configuration
    config: ManagerConfig,
}

impl AdapterManager {
    /// Create a new adapter manager
    pub fn new(config: ManagerConfig) -> Self {
        let (tx, rx) = mpsc::channel(config.incoming_buffer_size);

        Self {
            adapters: DashMap::new(),
            incoming_tx: tx,
            incoming_rx: Arc::new(tokio::sync::Mutex::new(rx)),
            config,
        }
    }

    /// Register a new network adapter
    ///
    /// Returns the adapter ID for later reference.
    pub fn register_adapter(&self, adapter: Arc<dyn NetworkAdapter>) -> String {
        let info = adapter.info();
        let adapter_id = info.id.clone();

        // Store the adapter
        self.adapters.insert(adapter_id.clone(), adapter.clone());

        // Spawn a task to receive frames from this adapter
        let incoming_tx = self.incoming_tx.clone();
        let adapter_clone = adapter.clone();
        let adapter_id_clone = adapter_id.clone();

        tokio::spawn(async move {
            loop {
                match adapter_clone.receive().await {
                    Ok((sender, frame)) => {
                        let received = ReceivedFrame {
                            frame,
                            sender,
                            adapter_type: adapter_clone.adapter_type(),
                            adapter_id: adapter_id_clone.clone(),
                        };

                        if incoming_tx.send(received).await.is_err() {
                            // Channel closed, stop receiving
                            break;
                        }
                    }
                    Err(e) => {
                        // Log error and continue
                        eprintln!("Adapter {} receive error: {}", adapter_id_clone, e);
                        // Could implement backoff here if needed
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });

        adapter_id
    }

    /// Unregister an adapter by ID
    pub fn unregister_adapter(&self, adapter_id: &str) -> Option<Arc<dyn NetworkAdapter>> {
        self.adapters.remove(adapter_id).map(|(_, v)| v)
    }

    /// Get an adapter by ID
    pub fn get_adapter(&self, adapter_id: &str) -> Option<Arc<dyn NetworkAdapter>> {
        self.adapters.get(adapter_id).map(|v| v.clone())
    }

    /// Get all adapters of a specific type
    pub fn get_adapters_by_type(&self, adapter_type: AdapterType) -> Vec<Arc<dyn NetworkAdapter>> {
        self.adapters
            .iter()
            .filter(|entry| entry.value().adapter_type() == adapter_type)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all registered adapters
    pub fn get_all_adapters(&self) -> Vec<Arc<dyn NetworkAdapter>> {
        self.adapters.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Send a frame to a specific peer using a specific adapter
    pub async fn send_to(&self, adapter_id: &str, peer: &NodeId, frame: &Frame) -> Result<()> {
        let adapter = self
            .get_adapter(adapter_id)
            .ok_or(NetworkError::AdapterUnavailable)?;

        adapter.send_to(peer, frame).await
    }

    /// Send a frame to a specific peer using the best available adapter
    ///
    /// This will try adapters in order of preference (currently just first available).
    pub async fn send_to_any(&self, peer: &NodeId, frame: &Frame) -> Result<()> {
        let adapters = self.get_all_adapters();

        if adapters.is_empty() {
            return Err(NetworkError::AdapterUnavailable);
        }

        // Try each adapter until one succeeds
        let mut last_error = None;
        for adapter in adapters {
            if adapter.is_available().await {
                match adapter.send_to(peer, frame).await {
                    Ok(()) => return Ok(()),
                    Err(e) => last_error = Some(e),
                }
            }
        }

        Err(last_error.unwrap_or(NetworkError::AdapterUnavailable))
    }

    /// Broadcast a frame on a specific adapter
    pub async fn broadcast_on(&self, adapter_id: &str, frame: &Frame) -> Result<()> {
        let adapter = self
            .get_adapter(adapter_id)
            .ok_or(NetworkError::AdapterUnavailable)?;

        adapter.broadcast(frame).await
    }

    /// Broadcast a frame on all available adapters
    pub async fn broadcast_all(&self, frame: &Frame) -> Result<()> {
        let adapters = self.get_all_adapters();

        if adapters.is_empty() {
            return Err(NetworkError::AdapterUnavailable);
        }

        let mut success_count = 0;
        let mut last_error = None;

        for adapter in adapters {
            if adapter.is_available().await {
                match adapter.broadcast(frame).await {
                    Ok(()) => success_count += 1,
                    Err(e) => last_error = Some(e),
                }
            }
        }

        if success_count > 0 {
            Ok(())
        } else {
            Err(last_error.unwrap_or(NetworkError::AdapterUnavailable))
        }
    }

    /// Receive the next frame from any adapter
    ///
    /// This will block until a frame is available.
    pub async fn receive(&self) -> Option<ReceivedFrame> {
        let mut rx = self.incoming_rx.lock().await;
        rx.recv().await
    }

    /// Get the number of registered adapters
    pub fn adapter_count(&self) -> usize {
        self.adapters.len()
    }

    /// Shutdown all adapters
    pub async fn shutdown_all(&self) -> Result<()> {
        let adapters = self.get_all_adapters();

        for adapter in adapters {
            if let Err(e) = adapter.shutdown().await {
                eprintln!("Error shutting down adapter {}: {}", adapter.info().id, e);
            }
        }

        Ok(())
    }
}

impl fmt::Debug for AdapterManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdapterManager")
            .field("adapter_count", &self.adapter_count())
            .field("config", &self.config)
            .finish()
    }
}

use std::fmt;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{AdapterInfo, AdapterStats};
    use myriadmesh_protocol::frame::{FrameHeader, MAGIC_BYTES, PROTOCOL_VERSION};

    /// Mock adapter for testing
    #[derive(Debug)]
    struct MockAdapter {
        id: String,
        adapter_type: AdapterType,
    }

    impl MockAdapter {
        fn new(id: &str, adapter_type: AdapterType) -> Self {
            Self {
                id: id.to_string(),
                adapter_type,
            }
        }
    }

    #[async_trait::async_trait]
    impl NetworkAdapter for MockAdapter {
        fn info(&self) -> AdapterInfo {
            AdapterInfo {
                id: self.id.clone(),
                adapter_type: self.adapter_type,
                name: format!("Mock {}", self.id),
                mtu: 1500,
                available: true,
                address: None,
            }
        }

        async fn send_to(&self, _peer: &NodeId, _frame: &Frame) -> Result<()> {
            Ok(())
        }

        async fn broadcast(&self, _frame: &Frame) -> Result<()> {
            Ok(())
        }

        async fn receive(&self) -> Result<(NodeId, Frame)> {
            // Sleep forever to simulate waiting for frames
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            Err(NetworkError::Timeout)
        }

        async fn is_available(&self) -> bool {
            true
        }

        async fn stats(&self) -> AdapterStats {
            AdapterStats::default()
        }

        async fn shutdown(&self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_register_adapter() {
        let manager = AdapterManager::new(ManagerConfig::default());
        let adapter = Arc::new(MockAdapter::new("test1", AdapterType::Ethernet));

        let id = manager.register_adapter(adapter);
        assert_eq!(id, "test1");
        assert_eq!(manager.adapter_count(), 1);
    }

    #[tokio::test]
    async fn test_unregister_adapter() {
        let manager = AdapterManager::new(ManagerConfig::default());
        let adapter = Arc::new(MockAdapter::new("test1", AdapterType::Ethernet));

        let id = manager.register_adapter(adapter);
        assert_eq!(manager.adapter_count(), 1);

        let removed = manager.unregister_adapter(&id);
        assert!(removed.is_some());
        assert_eq!(manager.adapter_count(), 0);
    }

    #[tokio::test]
    async fn test_get_adapters_by_type() {
        let manager = AdapterManager::new(ManagerConfig::default());

        manager.register_adapter(Arc::new(MockAdapter::new("eth1", AdapterType::Ethernet)));
        manager.register_adapter(Arc::new(MockAdapter::new("eth2", AdapterType::Ethernet)));
        manager.register_adapter(Arc::new(MockAdapter::new("bt1", AdapterType::Bluetooth)));

        let ethernet_adapters = manager.get_adapters_by_type(AdapterType::Ethernet);
        assert_eq!(ethernet_adapters.len(), 2);

        let bluetooth_adapters = manager.get_adapters_by_type(AdapterType::Bluetooth);
        assert_eq!(bluetooth_adapters.len(), 1);
    }

    #[tokio::test]
    async fn test_send_to() {
        let manager = AdapterManager::new(ManagerConfig::default());
        let adapter = Arc::new(MockAdapter::new("test1", AdapterType::Ethernet));
        let id = manager.register_adapter(adapter);

        let frame = Frame {
            header: FrameHeader {
                magic: MAGIC_BYTES,
                version: PROTOCOL_VERSION,
                flags: 0,
                payload_length: 0,
                checksum: 0,
            },
            payload: vec![],
        };

        let peer = NodeId::from_bytes([1; 32]);
        let result = manager.send_to(&id, &peer, &frame).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_broadcast_all() {
        let manager = AdapterManager::new(ManagerConfig::default());

        manager.register_adapter(Arc::new(MockAdapter::new("eth1", AdapterType::Ethernet)));
        manager.register_adapter(Arc::new(MockAdapter::new("eth2", AdapterType::Ethernet)));

        let frame = Frame {
            header: FrameHeader {
                magic: MAGIC_BYTES,
                version: PROTOCOL_VERSION,
                flags: 0,
                payload_length: 0,
                checksum: 0,
            },
            payload: vec![],
        };

        let result = manager.broadcast_all(&frame).await;
        assert!(result.is_ok());
    }
}
