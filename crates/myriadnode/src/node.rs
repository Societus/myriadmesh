use anyhow::Result;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

use crate::api::ApiServer;
use crate::backhaul::{BackhaulConfig, BackhaulDetector};
use crate::config::Config;
use crate::failover::FailoverManager;
use crate::heartbeat::HeartbeatService;
use crate::monitor::NetworkMonitor;
use crate::scoring::ScoringWeights;
use crate::storage::Storage;

use myriadmesh_appliance::{ApplianceManager, ApplianceManagerConfig, MessageCacheConfig};
use myriadmesh_crypto::identity::NodeIdentity;
use myriadmesh_dht::routing_table::RoutingTable;
use myriadmesh_ledger::ChainSync;
use myriadmesh_network::{adapters::*, AdapterManager, NetworkAdapter};
use myriadmesh_protocol::types::NODE_ID_SIZE;
use myriadmesh_routing::PriorityQueue;
use std::collections::HashMap;
use std::fs;

/// Main node orchestrator
pub struct Node {
    config: Config,
    identity: Arc<NodeIdentity>, // SECURITY C3: Node identity for adapter authentication
    storage: Arc<RwLock<Storage>>,
    adapter_manager: Arc<RwLock<AdapterManager>>,
    #[allow(dead_code)]
    message_queue: PriorityQueue,
    #[allow(dead_code)]
    dht: RoutingTable,
    #[allow(dead_code)]
    ledger: Arc<RwLock<ChainSync>>,
    api_server: Option<ApiServer>,
    appliance_manager: Option<Arc<ApplianceManager>>,
    monitor: NetworkMonitor,
    failover_manager: Arc<FailoverManager>,
    heartbeat_service: Arc<HeartbeatService>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: mpsc::Receiver<()>,
}

impl Node {
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing node components...");

        // Initialize storage
        let storage = Arc::new(RwLock::new(Storage::new(&config.data_directory).await?));
        info!("✓ Storage initialized");

        // Initialize adapter manager
        let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));
        info!("✓ Adapter manager initialized");

        // Initialize message queue
        let message_queue = PriorityQueue::new(1000); // Max 1000 messages per priority level
        info!("✓ Message queue initialized");

        // TODO: When Router is fully integrated, set up ledger confirmation callback:
        // router.set_confirmation_callback(Arc::new(move |msg_id, src, dest, is_local| {
        //     // Create MESSAGE ledger entry for successful routing
        //     // This records message delivery confirmations in the blockchain
        //     // See: myriadmesh-ledger/src/entry.rs - MessageEntry
        // }));

        // Initialize DHT
        // SECURITY C6: NodeID is now 64 bytes for collision resistance
        let node_id_bytes: [u8; NODE_ID_SIZE] = config
            .node
            .id
            .as_slice()
            .try_into()
            .expect("Node ID must be 64 bytes");
        let node_id = myriadmesh_protocol::NodeId::from_bytes(node_id_bytes);
        let dht = RoutingTable::new(node_id);
        info!("✓ DHT routing table initialized");

        // Initialize ledger
        let ledger_dir = config.data_directory.join("ledger");
        fs::create_dir_all(&ledger_dir)?;
        let ledger_config = myriadmesh_ledger::StorageConfig::new(&ledger_dir)
            .with_keep_blocks(config.ledger.keep_blocks);
        let ledger_storage = myriadmesh_ledger::LedgerStorage::new(ledger_config)?;
        let consensus_manager =
            myriadmesh_ledger::ConsensusManager::new(config.ledger.min_reputation);
        let chain_sync = ChainSync::new(ledger_storage, consensus_manager);
        let ledger = Arc::new(RwLock::new(chain_sync));
        info!(
            "✓ Ledger initialized (keep_blocks: {})",
            config.ledger.keep_blocks
        );

        // Initialize network monitor
        let monitor = NetworkMonitor::new(
            config.network.monitoring.clone(),
            Arc::clone(&adapter_manager),
            Arc::clone(&storage),
        );
        info!("✓ Network monitor initialized");

        // Initialize failover manager
        let scoring_weights = match config.network.scoring.mode.as_str() {
            "battery" => ScoringWeights::battery_optimized(),
            "performance" => ScoringWeights::performance_optimized(),
            "reliability" => ScoringWeights::reliability_optimized(),
            "privacy" => ScoringWeights::privacy_optimized(),
            _ => ScoringWeights::default(),
        };
        let failover_manager = Arc::new(FailoverManager::new(
            config.network.failover.clone(),
            Arc::clone(&adapter_manager),
            scoring_weights,
        ));
        info!(
            "✓ Failover manager initialized (mode: {})",
            config.network.scoring.mode
        );

        // Load node identity
        myriadmesh_crypto::init()?;
        let key_dir = config.data_directory.join("keys");
        let private_key_path = key_dir.join("node.key");
        let public_key_path = key_dir.join("node.pub");

        let identity = if private_key_path.exists() && public_key_path.exists() {
            let secret_bytes = fs::read(&private_key_path)?;
            let public_bytes = fs::read(&public_key_path)?;
            NodeIdentity::from_bytes(&public_bytes, &secret_bytes)?
        } else {
            warn!("Node identity keys not found, generating new identity");
            let new_identity = NodeIdentity::generate()?;
            fs::create_dir_all(&key_dir)?;
            fs::write(&private_key_path, new_identity.export_secret_key())?;
            fs::write(&public_key_path, new_identity.export_public_key())?;
            new_identity
        };
        let identity = Arc::new(identity);
        info!("✓ Node identity loaded");

        // Initialize backhaul detector
        let backhaul_detector = Arc::new(BackhaulDetector::new(BackhaulConfig {
            allow_backhaul_mesh: false,
            check_interval_secs: 300,
        }));
        info!("✓ Backhaul detector initialized");

        // Build adapter configs map
        let mut adapter_configs = HashMap::new();
        adapter_configs.insert(
            "ethernet".to_string(),
            config.network.adapters.ethernet.clone(),
        );
        adapter_configs.insert(
            "bluetooth".to_string(),
            config.network.adapters.bluetooth.clone(),
        );
        adapter_configs.insert(
            "bluetooth_le".to_string(),
            config.network.adapters.bluetooth_le.clone(),
        );
        adapter_configs.insert(
            "cellular".to_string(),
            config.network.adapters.cellular.clone(),
        );

        // Initialize heartbeat service
        let heartbeat_service = Arc::new(HeartbeatService::new(
            crate::heartbeat::HeartbeatConfig {
                enabled: config.heartbeat.enabled,
                interval_secs: config.heartbeat.interval_secs,
                timeout_secs: config.heartbeat.timeout_secs,
                include_geolocation: config.heartbeat.include_geolocation,
                store_remote_geolocation: config.heartbeat.store_remote_geolocation,
                max_nodes: config.heartbeat.max_nodes,
            },
            node_id,
            Arc::clone(&identity),
            Arc::clone(&adapter_manager),
            Arc::clone(&backhaul_detector),
            adapter_configs,
        ));
        info!(
            "✓ Heartbeat service initialized (interval: {}s)",
            config.heartbeat.interval_secs
        );

        // Initialize ApplianceManager if appliance mode is enabled
        let appliance_manager = if config.appliance.enabled {
            info!("Initializing appliance mode...");

            // Convert sodiumoxide secret key to ed25519-dalek SigningKey
            let secret_key_bytes = identity.export_secret_key();
            let signing_key = ed25519_dalek::SigningKey::from_bytes(
                secret_key_bytes
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Invalid secret key length"))?,
            );

            let appliance_config = ApplianceManagerConfig {
                node_id: hex::encode(&config.node.id),
                max_paired_devices: config.appliance.max_paired_devices,
                message_caching: config.appliance.message_caching,
                relay_enabled: config.appliance.enable_relay,
                bridge_enabled: config.appliance.enable_bridge,
                require_pairing_approval: config.appliance.require_pairing_approval,
                pairing_methods: config.appliance.pairing_methods.clone(),
                cache_config: MessageCacheConfig {
                    max_messages_per_device: config.appliance.max_cache_messages_per_device,
                    max_total_messages: config.appliance.max_total_cache_messages,
                },
                data_directory: config.data_directory.join("appliance"),
            };

            let manager = Arc::new(ApplianceManager::new(appliance_config, signing_key).await?);
            info!(
                "✓ Appliance manager initialized (max devices: {}, cache: {})",
                config.appliance.max_paired_devices, config.appliance.max_total_cache_messages
            );
            Some(manager)
        } else {
            info!("Appliance mode disabled");
            None
        };

        // Initialize UpdateCoordinator if updates are enabled
        let update_coordinator = if config.updates.enabled {
            info!("Initializing update coordinator...");

            // Clone the identity for the update coordinator
            let update_identity =
                NodeIdentity::from_keypair(identity.public_key, identity.secret_key.clone());

            let coordinator = Arc::new(myriadmesh_updates::UpdateCoordinator::new(Arc::new(
                update_identity,
            )));

            info!(
                "✓ Update coordinator initialized (auto_install: {}, verification: {}h)",
                config.updates.auto_install, config.updates.verification_period_hours
            );
            Some(coordinator)
        } else {
            info!("Update system disabled");
            None
        };

        // Initialize API server if enabled
        let api_server = if config.api.enabled {
            let server = ApiServer::new(
                config.api.clone(),
                Arc::clone(&adapter_manager),
                Arc::clone(&heartbeat_service),
                Arc::clone(&failover_manager),
                Arc::clone(&ledger),
                appliance_manager.clone(),
                update_coordinator.clone(),
                Some(Arc::clone(&storage)),
                config.node.name.clone(),
                hex::encode(&config.node.id),
            )
            .await?;
            info!(
                "✓ API server initialized on {}:{}",
                config.api.bind, config.api.port
            );
            Some(server)
        } else {
            info!("API server disabled");
            None
        };

        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Ok(Self {
            config,
            identity,
            storage,
            adapter_manager,
            message_queue,
            dht,
            ledger,
            api_server,
            appliance_manager,
            monitor,
            failover_manager,
            heartbeat_service,
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

        // Start failover manager
        self.failover_manager.start().await?;
        if self.config.network.failover.auto_failover {
            info!("✓ Automatic failover enabled");
        } else {
            info!("  Automatic failover disabled");
        }

        // Start heartbeat service
        self.heartbeat_service.start().await?;
        if self.config.heartbeat.enabled {
            info!("✓ Heartbeat service running (NodeMap discovery enabled)");
        } else {
            info!("  Heartbeat service disabled");
        }

        // Start DHT (if enabled)
        if self.config.dht.enabled {
            info!("✓ DHT service running");
        }

        info!("═══════════════════════════════════════════════");
        info!("  MyriadNode is now running");
        info!("═══════════════════════════════════════════════");
        if self.config.api.enabled {
            info!(
                "  API: http://{}:{}",
                self.config.api.bind, self.config.api.port
            );
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
            match self.initialize_ethernet_adapter().await {
                Ok(()) => info!("  ✓ Ethernet adapter registered"),
                Err(e) => warn!("  ✗ Ethernet adapter failed: {}", e),
            }
        }

        // Start Bluetooth adapter if enabled
        if self.config.network.adapters.bluetooth.enabled {
            info!("  Starting Bluetooth adapter...");
            match self.initialize_bluetooth_adapter().await {
                Ok(()) => info!("  ✓ Bluetooth adapter registered"),
                Err(e) => warn!("  ✗ Bluetooth adapter failed: {}", e),
            }
        }

        // Start Bluetooth LE adapter if enabled
        if self.config.network.adapters.bluetooth_le.enabled {
            info!("  Starting Bluetooth LE adapter...");
            match self.initialize_ble_adapter().await {
                Ok(()) => info!("  ✓ Bluetooth LE adapter registered"),
                Err(e) => warn!("  ✗ Bluetooth LE adapter failed: {}", e),
            }
        }

        // Start Cellular adapter if enabled
        if self.config.network.adapters.cellular.enabled {
            info!("  Starting Cellular adapter...");
            match self.initialize_cellular_adapter().await {
                Ok(()) => info!("  ✓ Cellular adapter registered"),
                Err(e) => warn!("  ✗ Cellular adapter failed: {}", e),
            }
        }

        Ok(())
    }

    async fn initialize_ethernet_adapter(&mut self) -> Result<()> {
        let config = EthernetConfig::default();

        // SECURITY C3: Pass identity for authenticated UDP
        let mut adapter = EthernetAdapter::new(Arc::clone(&self.identity), config);

        adapter.initialize().await?;

        // Check for backhaul status before auto-starting
        if self.config.network.adapters.ethernet.auto_start {
            let backhaul_config = BackhaulConfig {
                allow_backhaul_mesh: self.config.network.adapters.ethernet.allow_backhaul_mesh,
                check_interval_secs: 300,
            };
            let detector = BackhaulDetector::new(backhaul_config);

            // Try to detect all backhaul interfaces
            match detector.detect_all_backhauls() {
                Ok(backhauls) => {
                    if !backhauls.is_empty() {
                        info!("  Detected backhaul interfaces: {:?}", backhauls);

                        if !self.config.network.adapters.ethernet.allow_backhaul_mesh {
                            warn!("  Ethernet adapter may be backhaul - not auto-starting");
                            warn!("  Set 'allow_backhaul_mesh: true' to override");
                        } else {
                            adapter.start().await?;
                        }
                    } else {
                        adapter.start().await?;
                    }
                }
                Err(e) => {
                    warn!("  Failed to detect backhaul status: {}", e);
                    // Start anyway if detection fails
                    adapter.start().await?;
                }
            }
        }

        let mut manager = self.adapter_manager.write().await;
        manager
            .register_adapter("ethernet".to_string(), Box::new(adapter))
            .await?;

        Ok(())
    }

    async fn initialize_bluetooth_adapter(&mut self) -> Result<()> {
        let config = BluetoothConfig::default();
        let mut adapter = BluetoothAdapter::new(config);

        adapter.initialize().await?;

        if self.config.network.adapters.bluetooth.auto_start {
            adapter.start().await?;
        }

        let mut manager = self.adapter_manager.write().await;
        manager
            .register_adapter("bluetooth".to_string(), Box::new(adapter))
            .await?;

        Ok(())
    }

    async fn initialize_ble_adapter(&mut self) -> Result<()> {
        let config = BleConfig::default();
        let mut adapter = BleAdapter::new(config);

        adapter.initialize().await?;

        if self.config.network.adapters.bluetooth_le.auto_start {
            adapter.start().await?;
        }

        let mut manager = self.adapter_manager.write().await;
        manager
            .register_adapter("bluetooth_le".to_string(), Box::new(adapter))
            .await?;

        Ok(())
    }

    async fn initialize_cellular_adapter(&mut self) -> Result<()> {
        let config = CellularConfig::default();
        let mut adapter = CellularAdapter::new(config);

        adapter.initialize().await?;

        if self.config.network.adapters.cellular.auto_start {
            adapter.start().await?;
        }

        let mut manager = self.adapter_manager.write().await;
        manager
            .register_adapter("cellular".to_string(), Box::new(adapter))
            .await?;

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
        {
            let mut manager = self.adapter_manager.write().await;
            if let Err(e) = manager.stop_all().await {
                error!("Failed to stop adapters: {}", e);
            }
        }

        if let Some(_api_server) = &self.api_server {
            info!("Stopping API server...");
            // Server will be dropped and cleaned up
        }

        if let Some(appliance_manager) = &self.appliance_manager {
            info!("Stopping appliance manager...");
            appliance_manager.shutdown().await;
        }

        info!("Closing storage...");
        {
            let storage = self.storage.write().await;
            storage.close().await?;
        }

        info!("Shutdown complete");
        Ok(())
    }

    pub fn shutdown_handle(&self) -> mpsc::Sender<()> {
        self.shutdown_tx.clone()
    }
}
