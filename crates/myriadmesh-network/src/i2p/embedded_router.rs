//! Embedded i2pd router management
//!
//! Provides automatic i2pd process management with zero configuration required.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum I2pRouterError {
    #[error("Failed to start i2pd: {0}")]
    StartupFailed(String),

    #[error("I2pd not found: {0}")]
    BinaryNotFound(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Router not ready after {0:?}")]
    TimeoutError(Duration),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, I2pRouterError>;

/// I2P router configuration
#[derive(Debug, Clone)]
pub struct I2pRouterConfig {
    /// Data directory for i2p router state
    pub data_dir: PathBuf,

    /// SAM API port (default: 7656)
    pub sam_port: u16,

    /// Enable IPv6 support
    pub enable_ipv6: bool,

    /// Bandwidth limit in KB/s (None = unlimited)
    pub bandwidth_limit_kbps: Option<u32>,

    /// Number of transit tunnels to support
    pub transit_tunnels: u32,

    /// Path to i2pd binary (auto-detect if None)
    pub i2pd_binary: Option<PathBuf>,
}

impl Default for I2pRouterConfig {
    fn default() -> Self {
        I2pRouterConfig {
            data_dir: Self::default_data_dir(),
            sam_port: 7656,
            enable_ipv6: false,
            bandwidth_limit_kbps: Some(1024), // 1 MB/s
            transit_tunnels: 50,
            i2pd_binary: None,
        }
    }
}

impl I2pRouterConfig {
    fn default_data_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("myriadmesh")
            .join("i2p")
    }

    /// Generate i2pd configuration file contents
    fn generate_config(&self) -> String {
        let mut config = format!(
            r#"# Auto-generated i2pd configuration for MyriadMesh
# Generated at: {}

# Data directory
datadir = {}

# Logging
log = file
loglevel = warn
logfile = i2pd.log

# Network
ipv4 = true
ipv6 = {}
notransit = false

# SAM API (for MyriadMesh)
[sam]
enabled = true
address = 127.0.0.1
port = {}

# Disable unused services
[http]
enabled = false

[httpproxy]
enabled = false

[socksproxy]
enabled = false

[upnp]
enabled = false

# Performance
[limits]
transittunnels = {}
"#,
            chrono::Utc::now(),
            self.data_dir.display(),
            self.enable_ipv6,
            self.sam_port,
            self.transit_tunnels,
        );

        if let Some(limit) = self.bandwidth_limit_kbps {
            config.push_str(&format!("ntcpsoft = {}\nntcphard = {}\n", limit, limit));
        }

        config
    }
}

/// Embedded i2pd router process
pub struct EmbeddedI2pRouter {
    process: Child,
    config: I2pRouterConfig,
    ready: Arc<AtomicBool>,
}

impl std::fmt::Debug for EmbeddedI2pRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddedI2pRouter")
            .field("config", &self.config)
            .field("ready", &self.ready.load(Ordering::SeqCst))
            .finish()
    }
}

impl EmbeddedI2pRouter {
    /// Start an embedded i2pd router
    pub fn start(config: I2pRouterConfig) -> Result<Self> {
        // Create data directory
        fs::create_dir_all(&config.data_dir)?;

        // Generate configuration
        let config_content = config.generate_config();
        let config_path = config.data_dir.join("i2pd.conf");
        fs::write(&config_path, config_content)?;

        // Find i2pd binary
        let i2pd_binary = if let Some(path) = &config.i2pd_binary {
            path.clone()
        } else {
            Self::find_i2pd_binary()?
        };

        // Start i2pd process
        let process = Command::new(&i2pd_binary)
            .arg("--conf")
            .arg(&config_path)
            .arg("--datadir")
            .arg(&config.data_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| I2pRouterError::StartupFailed(e.to_string()))?;

        let ready = Arc::new(AtomicBool::new(false));

        let mut router = EmbeddedI2pRouter {
            process,
            config,
            ready,
        };

        // Monitor startup in background
        router.monitor_startup();

        Ok(router)
    }

    /// Find i2pd binary in system PATH
    fn find_i2pd_binary() -> Result<PathBuf> {
        // Try common binary names
        for name in &["i2pd", "i2pd.exe"] {
            if let Ok(output) = Command::new("which").arg(name).output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Ok(PathBuf::from(path));
                    }
                }
            }
        }

        Err(I2pRouterError::BinaryNotFound(
            "i2pd not found in PATH. Please install i2pd or provide binary path".to_string(),
        ))
    }

    /// Monitor router startup output
    fn monitor_startup(&mut self) {
        let ready = self.ready.clone();

        if let Some(stderr) = self.process.stderr.take() {
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(|r| r.ok()) {
                    // Check for startup completion indicators
                    if line.contains("SAM session created")
                        || line.contains("SAM bridge started")
                        || line.contains("Router started")
                    {
                        ready.store(true, Ordering::SeqCst);
                    }
                }
            });
        }
    }

    /// Wait for router to be ready
    pub fn wait_ready(&self, timeout: Duration) -> Result<()> {
        let start = Instant::now();

        while !self.ready.load(Ordering::SeqCst) {
            if start.elapsed() > timeout {
                return Err(I2pRouterError::TimeoutError(timeout));
            }

            // Also check if we can connect to SAM port
            if Self::check_sam_available(self.config.sam_port) {
                self.ready.store(true, Ordering::SeqCst);
                return Ok(());
            }

            std::thread::sleep(Duration::from_millis(500));
        }

        Ok(())
    }

    /// Check if SAM port is available
    fn check_sam_available(port: u16) -> bool {
        std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok()
    }

    /// Get SAM port
    pub fn sam_port(&self) -> u16 {
        self.config.sam_port
    }

    /// Check if router is ready
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// Stop the router
    pub fn stop(&mut self) -> Result<()> {
        self.process.kill()?;
        self.process.wait()?;
        Ok(())
    }
}

impl Drop for EmbeddedI2pRouter {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// I2P router mode (system or embedded)
#[derive(Debug)]
pub enum I2pRouterMode {
    /// Using system-installed i2p router
    System { sam_port: u16 },

    /// Using embedded i2pd process
    Embedded { router: EmbeddedI2pRouter },
}

impl I2pRouterMode {
    /// Initialize i2p router (try system first, then embedded)
    pub async fn initialize(config: I2pRouterConfig) -> Result<Self> {
        // Try to connect to existing system router
        if Self::check_system_router(config.sam_port).await {
            log::info!("Found existing i2p router on port {}", config.sam_port);
            return Ok(I2pRouterMode::System {
                sam_port: config.sam_port,
            });
        }

        // Start embedded router
        log::info!("No system i2p router found, starting embedded i2pd");
        let router = EmbeddedI2pRouter::start(config)?;

        // Wait for router to be ready
        router.wait_ready(Duration::from_secs(60))?;

        Ok(I2pRouterMode::Embedded { router })
    }

    /// Check if system i2p router is available
    async fn check_system_router(sam_port: u16) -> bool {
        tokio::net::TcpStream::connect(format!("127.0.0.1:{}", sam_port))
            .await
            .is_ok()
    }

    /// Get SAM port
    pub fn sam_port(&self) -> u16 {
        match self {
            I2pRouterMode::System { sam_port } => *sam_port,
            I2pRouterMode::Embedded { router } => router.sam_port(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_generation() {
        let config = I2pRouterConfig::default();
        let config_str = config.generate_config();

        assert!(config_str.contains("sam]"));
        assert!(config_str.contains("port = 7656"));
        assert!(config_str.contains("transittunnels = 50"));
    }

    #[test]
    fn test_default_config() {
        let config = I2pRouterConfig::default();
        assert_eq!(config.sam_port, 7656);
        assert_eq!(config.transit_tunnels, 50);
        assert_eq!(config.bandwidth_limit_kbps, Some(1024));
    }

    #[test]
    fn test_custom_config() {
        let config = I2pRouterConfig {
            sam_port: 7777,
            bandwidth_limit_kbps: None,
            ..Default::default()
        };

        let config_str = config.generate_config();
        assert!(config_str.contains("port = 7777"));
    }
}
