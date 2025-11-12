use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use myriadmesh_crypto::identity::NodeIdentity;

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

impl Config {
    /// Load configuration from file or use defaults
    pub fn load(config_path: Option<PathBuf>, data_dir: Option<PathBuf>) -> Result<Self> {
        let config_path = config_path.unwrap_or_else(|| Self::default_config_path());
        let data_dir = data_dir.unwrap_or_else(|| Self::default_data_dir());

        if !config_path.exists() {
            anyhow::bail!(
                "Configuration file not found: {}\nRun with --init to create a new configuration",
                config_path.display()
            );
        }

        let contents = fs::read_to_string(&config_path)
            .context("Failed to read configuration file")?;

        let mut config: Config = serde_yaml::from_str(&contents)
            .context("Failed to parse configuration file")?;

        config.config_file_path = config_path;
        config.data_directory = data_dir;

        Ok(config)
    }

    /// Create a new default configuration
    pub fn create_default(config_path: Option<PathBuf>, data_dir: Option<PathBuf>) -> Result<Self> {
        let config_path = config_path.unwrap_or_else(|| Self::default_config_path());
        let data_dir = data_dir.unwrap_or_else(|| Self::default_data_dir());

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
                    },
                    bluetooth: AdapterConfig {
                        enabled: false,
                        auto_start: false,
                    },
                    bluetooth_le: AdapterConfig {
                        enabled: false,
                        auto_start: false,
                    },
                    cellular: AdapterConfig {
                        enabled: false,
                        auto_start: false,
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
