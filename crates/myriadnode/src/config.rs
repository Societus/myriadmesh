use anyhow::{Context, Result};
use myriadmesh_crypto::identity::NodeIdentity;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub node: NodeConfig,
    pub api: ApiConfig,
    pub dht: DhtConfig,
    pub network: NetworkConfig,
    pub security: SecurityConfig,
    pub i2p: I2pConfig,
    pub routing: RoutingConfig,
    pub logging: LoggingConfig,
    pub heartbeat: HeartbeatConfig,
    pub appliance: ApplianceConfig,

    #[serde(skip)]
    config_file_path: PathBuf,
    #[serde(skip)]
    pub data_directory: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    #[serde(with = "hex_bytes")]
    pub id: Vec<u8>,
    pub name: String,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub enabled: bool,
    pub bind: String,
    pub port: u16,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhtConfig {
    pub enabled: bool,
    pub bootstrap_nodes: Vec<String>,
    pub port: u16,
    pub cache_messages: bool,
    pub cache_ttl_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub adapters: AdapterConfigs,
    pub monitoring: MonitoringConfig,
    pub failover: FailoverConfig,
    pub scoring: ScoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfigs {
    pub ethernet: AdapterConfig,
    pub bluetooth: AdapterConfig,
    pub bluetooth_le: AdapterConfig,
    pub cellular: AdapterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    pub enabled: bool,
    #[serde(default)]
    pub auto_start: bool,
    /// Allow mesh networking on backhaul interfaces (IP adapters only)
    #[serde(default)]
    pub allow_backhaul_mesh: bool,
    /// Allow heartbeat broadcasting on this adapter
    #[serde(default = "default_allow_heartbeat")]
    pub allow_heartbeat: bool,
    /// Override heartbeat interval for this adapter (optional)
    #[serde(default)]
    pub heartbeat_interval_override: Option<u64>,
}

fn default_allow_heartbeat() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub ping_interval_secs: u64,
    pub throughput_interval_secs: u64,
    pub reliability_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    pub auto_failover: bool,
    pub latency_threshold_multiplier: f32,
    pub loss_threshold: f32,
    pub retry_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
    pub mode: String, // "default", "battery", "performance", "reliability", "privacy"
    pub weight_latency: f64,
    pub weight_bandwidth: f64,
    pub weight_reliability: f64,
    pub weight_power: f64,
    pub weight_privacy: f64,
    #[serde(default = "default_recalculation_interval")]
    pub recalculation_interval_secs: u64,
}

fn default_recalculation_interval() -> u64 {
    60 // Recalculate scores every minute
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub require_signatures: bool,
    pub trusted_nodes_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I2pConfig {
    pub enabled: bool,
    pub sam_host: String,
    pub sam_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub max_hops: u32,
    pub store_and_forward: bool,
    pub message_ttl_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    pub enabled: bool,
    pub interval_secs: u64,
    pub timeout_secs: u64,
    pub include_geolocation: bool,
    pub store_remote_geolocation: bool,
    pub max_nodes: usize,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: 60,
            timeout_secs: 300,
            include_geolocation: false,
            store_remote_geolocation: false,
            max_nodes: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplianceConfig {
    /// Enable appliance mode (gateway/caching for mobile devices)
    pub enabled: bool,
    /// Maximum number of devices that can pair with this appliance
    pub max_paired_devices: usize,
    /// Enable message caching for paired devices
    pub message_caching: bool,
    /// Maximum number of messages to cache per device
    pub max_cache_messages_per_device: usize,
    /// Maximum total cached messages across all devices
    pub max_total_cache_messages: usize,
    /// Enable relay/proxy functionality for paired devices
    pub enable_relay: bool,
    /// Enable bridge functionality (connect different network segments)
    pub enable_bridge: bool,
    /// Require manual approval for pairing requests
    pub require_pairing_approval: bool,
    /// Pairing methods supported: "qr_code", "pin"
    pub pairing_methods: Vec<String>,
    /// mDNS service advertisement for local discovery
    pub mdns_enabled: bool,
    /// Publish appliance availability to DHT
    pub dht_advertisement: bool,
}

impl Default for ApplianceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_paired_devices: 10,
            message_caching: true,
            max_cache_messages_per_device: 1000,
            max_total_cache_messages: 10000,
            enable_relay: true,
            enable_bridge: true,
            require_pairing_approval: true,
            pairing_methods: vec!["qr_code".to_string(), "pin".to_string()],
            mdns_enabled: true,
            dht_advertisement: true,
        }
    }
}

impl Config {
    /// Load configuration from file or use defaults
    pub fn load(config_path: Option<PathBuf>, data_dir: Option<PathBuf>) -> Result<Self> {
        let config_path = config_path.unwrap_or_else(Self::default_config_path);
        let data_dir = data_dir.unwrap_or_else(Self::default_data_dir);

        if !config_path.exists() {
            anyhow::bail!(
                "Configuration file not found: {}\nRun with --init to create a new configuration",
                config_path.display()
            );
        }

        let contents =
            fs::read_to_string(&config_path).context("Failed to read configuration file")?;

        let mut config: Config =
            serde_yaml::from_str(&contents).context("Failed to parse configuration file")?;

        config.config_file_path = config_path;
        config.data_directory = data_dir;

        Ok(config)
    }

    /// Create a new default configuration
    pub fn create_default(config_path: Option<PathBuf>, data_dir: Option<PathBuf>) -> Result<Self> {
        let config_path = config_path.unwrap_or_else(Self::default_config_path);
        let data_dir = data_dir.unwrap_or_else(Self::default_data_dir);

        // Create directories
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(&data_dir)?;

        // Initialize crypto library
        myriadmesh_crypto::init()?;

        // Generate node identity
        let identity = NodeIdentity::generate()?;
        let node_id = identity.node_id.as_bytes().to_vec();

        // Save identity keys
        let key_dir = data_dir.join("keys");
        fs::create_dir_all(&key_dir)?;

        let private_key_path = key_dir.join("node.key");
        let public_key_path = key_dir.join("node.pub");

        fs::write(&private_key_path, identity.export_secret_key())?;
        fs::write(&public_key_path, identity.export_public_key())?;

        // Create default config
        let config = Config {
            node: NodeConfig {
                id: node_id,
                name: format!("myriad-{}", hex::encode(&identity.node_id.as_bytes()[..4])),
                primary: true,
            },
            api: ApiConfig {
                enabled: true,
                bind: "127.0.0.1".to_string(),
                port: 8080,
                auth: AuthConfig {
                    enabled: false,
                    token: None,
                },
            },
            dht: DhtConfig {
                enabled: true,
                bootstrap_nodes: vec![],
                port: 4001,
                cache_messages: true,
                cache_ttl_days: 7,
            },
            network: NetworkConfig {
                adapters: AdapterConfigs {
                    ethernet: AdapterConfig {
                        enabled: true,
                        auto_start: true,
                        allow_backhaul_mesh: false,
                        allow_heartbeat: true,
                        heartbeat_interval_override: None,
                    },
                    bluetooth: AdapterConfig {
                        enabled: false,
                        auto_start: false,
                        allow_backhaul_mesh: false,
                        allow_heartbeat: true,
                        heartbeat_interval_override: None,
                    },
                    bluetooth_le: AdapterConfig {
                        enabled: false,
                        auto_start: false,
                        allow_backhaul_mesh: false,
                        allow_heartbeat: true,
                        heartbeat_interval_override: None,
                    },
                    cellular: AdapterConfig {
                        enabled: false,
                        auto_start: false,
                        allow_backhaul_mesh: false,
                        allow_heartbeat: false, // Don't use cellular for heartbeats
                        heartbeat_interval_override: None,
                    },
                },
                monitoring: MonitoringConfig {
                    ping_interval_secs: 300,
                    throughput_interval_secs: 1800,
                    reliability_interval_secs: 3600,
                },
                failover: FailoverConfig {
                    auto_failover: true,
                    latency_threshold_multiplier: 5.0,
                    loss_threshold: 0.25,
                    retry_attempts: 3,
                },
                scoring: ScoringConfig {
                    mode: "default".to_string(),
                    weight_latency: 0.25,
                    weight_bandwidth: 0.20,
                    weight_reliability: 0.30,
                    weight_power: 0.10,
                    weight_privacy: 0.15,
                    recalculation_interval_secs: 60,
                },
            },
            security: SecurityConfig {
                require_signatures: true,
                trusted_nodes_only: false,
            },
            i2p: I2pConfig {
                enabled: true,
                sam_host: "127.0.0.1".to_string(),
                sam_port: 7656,
            },
            routing: RoutingConfig {
                max_hops: 10,
                store_and_forward: true,
                message_ttl_days: 7,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: Some(data_dir.join("logs").join("myriadnode.log")),
            },
            heartbeat: HeartbeatConfig {
                enabled: true,
                interval_secs: 60,
                timeout_secs: 300,
                include_geolocation: false,
                store_remote_geolocation: false,
                max_nodes: 1000,
            },
            appliance: ApplianceConfig::default(),
            config_file_path: config_path.clone(),
            data_directory: data_dir,
        };

        // Save configuration
        let yaml = serde_yaml::to_string(&config)?;
        fs::write(&config_path, yaml)?;

        Ok(config)
    }

    pub fn config_path(&self) -> &Path {
        &self.config_file_path
    }

    fn default_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("myriadnode")
            .join("config.yaml")
    }

    fn default_data_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("myriadnode")
    }
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(s).map_err(serde::de::Error::custom)
    }
}
