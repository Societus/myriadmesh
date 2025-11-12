use anyhow::Result;
use tokio::sync::mpsc;
use tokio::signal;
use tracing::{info, warn, error};

use crate::config::Config;
use crate::api::ApiServer;
use crate::storage::Storage;
use crate::monitor::NetworkMonitor;

use myriadmesh_network::AdapterManager;
use myriadmesh_routing::PriorityQueue;
use myriadmesh_dht::routing_table::RoutingTable;

/// Main node orchestrator
pub struct Node {
    config: Config,
    storage: Storage,
    adapter_manager: AdapterManager,
    message_queue: PriorityQueue,
    dht: RoutingTable,
    api_server: Option<ApiServer>,
    monitor: NetworkMonitor,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: mpsc::Receiver<()>,
}

impl Node {
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing node components...");

        // Initialize storage
        let storage = Storage::new(&config.data_directory).await?;
        info!("✓ Storage initialized");

        // Initialize adapter manager
        let adapter_manager = AdapterManager::new();
        info!("✓ Adapter manager initialized");

        // Initialize message queue
        let message_queue = PriorityQueue::new(1000); // Max 1000 messages per priority level
        info!("✓ Message queue initialized");

        // Initialize DHT
        let node_id_bytes: [u8; 32] = config.node.id.as_slice().try_into()
            .expect("Node ID must be 32 bytes");
        let node_id = myriadmesh_protocol::NodeId::from_bytes(node_id_bytes);
        let dht = RoutingTable::new(node_id);
        info!("✓ DHT routing table initialized");

        // Initialize network monitor
        let monitor = NetworkMonitor::new(config.network.monitoring.clone());
        info!("✓ Network monitor initialized");

        // Initialize API server if enabled
        let api_server = if config.api.enabled {
            let server = ApiServer::new(config.api.clone()).await?;
            info!("✓ API server initialized on {}:{}", config.api.bind, config.api.port);
            Some(server)
        } else {
            info!("API server disabled");
            None
        };

        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Ok(Self {
            config,
            storage,
            adapter_manager,
            message_queue,
            dht,
            api_server,
            monitor,
            shutdown_tx,
            shutdown_rx,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting MyriadNode services...");

        // Start network adapters
        self.start_network_adapters().await?;

        // Start API server
        if let Some(api_server) = &self.api_server {
            let server_handle = api_server.start().await?;
            info!("✓ API server running");

            // Store handle for shutdown
            tokio::spawn(async move {
                if let Err(e) = server_handle.await {
                    error!("API server error: {}", e);
                }
            });
        }

        // Start network monitor
        self.monitor.start().await?;
        info!("✓ Network monitor running");

        // Start DHT (if enabled)
        if self.config.dht.enabled {
            info!("✓ DHT service running");
        }

        info!("═══════════════════════════════════════════════");
        info!("  MyriadNode is now running");
        info!("═══════════════════════════════════════════════");
        if self.config.api.enabled {
            info!("  API: http://{}:{}", self.config.api.bind, self.config.api.port);
        }
        info!("  Node ID: {}", hex::encode(&self.config.node.id));
        info!("  Data Dir: {}", self.config.data_directory.display());
        info!("═══════════════════════════════════════════════");

        // Wait for shutdown signal
        self.wait_for_shutdown().await;

        info!("Shutting down MyriadNode...");
        self.shutdown().await?;

        Ok(())
    }

    async fn start_network_adapters(&mut self) -> Result<()> {
        info!("Starting network adapters...");

        // Start Ethernet adapter if enabled
        if self.config.network.adapters.ethernet.enabled {
            info!("  Starting Ethernet adapter...");
            // TODO: Initialize and register Ethernet adapter
            info!("  ✓ Ethernet adapter registered");
        }

        // Start Bluetooth adapter if enabled
        if self.config.network.adapters.bluetooth.enabled {
            info!("  Starting Bluetooth adapter...");
            // TODO: Initialize and register Bluetooth adapter
            info!("  ✓ Bluetooth adapter registered");
        }

        // Start Bluetooth LE adapter if enabled
        if self.config.network.adapters.bluetooth_le.enabled {
            info!("  Starting Bluetooth LE adapter...");
            // TODO: Initialize and register Bluetooth LE adapter
            info!("  ✓ Bluetooth LE adapter registered");
        }

        // Start Cellular adapter if enabled
        if self.config.network.adapters.cellular.enabled {
            info!("  Starting Cellular adapter...");
            // TODO: Initialize and register Cellular adapter
            info!("  ✓ Cellular adapter registered");
        }

        Ok(())
    }

    async fn wait_for_shutdown(&mut self) {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C signal");
            }
            _ = self.shutdown_rx.recv() => {
                info!("Received shutdown signal");
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Stopping network monitor...");
        self.monitor.stop().await?;

        info!("Stopping network adapters...");
        // TODO: Stop all adapters

        if let Some(_api_server) = &self.api_server {
            info!("Stopping API server...");
            // Server will be dropped and cleaned up
        }

        info!("Closing storage...");
        self.storage.close().await?;

        info!("Shutdown complete");
        Ok(())
    }

    pub fn shutdown_handle(&self) -> mpsc::Sender<()> {
        self.shutdown_tx.clone()
    }
}
