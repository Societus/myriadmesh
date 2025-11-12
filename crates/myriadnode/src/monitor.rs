use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration, Instant};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::config::MonitoringConfig;
use crate::storage::Storage;
use myriadmesh_network::AdapterManager;

/// Network performance monitor
pub struct NetworkMonitor {
    config: MonitoringConfig,
    adapter_manager: Arc<RwLock<AdapterManager>>,
    storage: Arc<RwLock<Storage>>,
    ping_task: Option<JoinHandle<()>>,
    throughput_task: Option<JoinHandle<()>>,
    reliability_task: Option<JoinHandle<()>>,
}

impl NetworkMonitor {
    pub fn new(
        config: MonitoringConfig,
        adapter_manager: Arc<RwLock<AdapterManager>>,
        storage: Arc<RwLock<Storage>>,
    ) -> Self {
        Self {
            config,
            adapter_manager,
            storage,
            ping_task: None,
            throughput_task: None,
            reliability_task: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting network monitoring tasks...");

        // Start ping monitor
        let ping_interval = self.config.ping_interval_secs;
        let adapter_manager = Arc::clone(&self.adapter_manager);
        let storage = Arc::clone(&self.storage);
        self.ping_task = Some(tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(ping_interval));
            loop {
                ticker.tick().await;
                if let Err(e) = Self::run_ping_tests(&adapter_manager, &storage).await {
                    warn!("Ping test failed: {}", e);
                }
            }
        }));
        debug!("Ping monitor started (interval: {}s)", ping_interval);

        // Start throughput monitor
        let throughput_interval = self.config.throughput_interval_secs;
        let adapter_manager = Arc::clone(&self.adapter_manager);
        let storage = Arc::clone(&self.storage);
        self.throughput_task = Some(tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(throughput_interval));
            loop {
                ticker.tick().await;
                if let Err(e) = Self::run_throughput_tests(&adapter_manager, &storage).await {
                    warn!("Throughput test failed: {}", e);
                }
            }
        }));
        debug!("Throughput monitor started (interval: {}s)", throughput_interval);

        // Start reliability monitor
        let reliability_interval = self.config.reliability_interval_secs;
        let adapter_manager = Arc::clone(&self.adapter_manager);
        let storage = Arc::clone(&self.storage);
        self.reliability_task = Some(tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(reliability_interval));
            loop {
                ticker.tick().await;
                if let Err(e) = Self::run_reliability_tests(&adapter_manager, &storage).await {
                    warn!("Reliability test failed: {}", e);
                }
            }
        }));
        debug!("Reliability monitor started (interval: {}s)", reliability_interval);

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping network monitoring tasks...");

        if let Some(task) = self.ping_task.take() {
            task.abort();
        }

        if let Some(task) = self.throughput_task.take() {
            task.abort();
        }

        if let Some(task) = self.reliability_task.take() {
            task.abort();
        }

        Ok(())
    }

    async fn run_ping_tests(
        adapter_manager: &Arc<RwLock<AdapterManager>>,
        _storage: &Arc<RwLock<Storage>>,
    ) -> Result<()> {
        debug!("Running ping tests...");

        let manager = adapter_manager.read().await;
        let adapter_ids = manager.adapter_ids();

        for adapter_id in adapter_ids {
            let start = Instant::now();

            // Attempt a simple connection test
            match manager.get_adapter(&adapter_id) {
                Some(adapter) => {
                    let status = adapter.read().await.get_status();
                    let latency = start.elapsed();

                    debug!(
                        "Adapter '{}': status={:?}, latency={:?}",
                        adapter_id, status, latency
                    );

                    // TODO: Store metrics in database
                }
                None => {
                    warn!("Adapter '{}' not found during ping test", adapter_id);
                }
            }
        }

        Ok(())
    }

    async fn run_throughput_tests(
        adapter_manager: &Arc<RwLock<AdapterManager>>,
        _storage: &Arc<RwLock<Storage>>,
    ) -> Result<()> {
        debug!("Running throughput tests...");

        let manager = adapter_manager.read().await;
        let adapter_ids = manager.adapter_ids();

        for adapter_id in adapter_ids {
            match manager.get_adapter(&adapter_id) {
                Some(adapter) => {
                    let adapter_guard = adapter.read().await;
                    let capabilities = adapter_guard.get_capabilities();

                    debug!(
                        "Adapter '{}': bandwidth={} bps, latency={} ms",
                        adapter_id, capabilities.typical_bandwidth_bps, capabilities.typical_latency_ms
                    );

                    // TODO: Perform actual throughput test by sending test frames
                    // TODO: Store metrics in database
                }
                None => {
                    warn!("Adapter '{}' not found during throughput test", adapter_id);
                }
            }
        }

        Ok(())
    }

    async fn run_reliability_tests(
        adapter_manager: &Arc<RwLock<AdapterManager>>,
        _storage: &Arc<RwLock<Storage>>,
    ) -> Result<()> {
        debug!("Running reliability tests...");

        let manager = adapter_manager.read().await;
        let adapter_ids = manager.adapter_ids();

        for adapter_id in adapter_ids {
            match manager.get_adapter(&adapter_id) {
                Some(adapter) => {
                    let adapter_guard = adapter.read().await;
                    let status = adapter_guard.get_status();
                    let capabilities = adapter_guard.get_capabilities();

                    debug!(
                        "Adapter '{}': status={:?}, reliability={}",
                        adapter_id, status, capabilities.reliability
                    );

                    // TODO: Perform packet loss test
                    // TODO: Store metrics in database
                }
                None => {
                    warn!("Adapter '{}' not found during reliability test", adapter_id);
                }
            }
        }

        Ok(())
    }
}
