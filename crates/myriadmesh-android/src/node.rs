use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Android wrapper for MyriadNode.
/// This struct manages the node lifecycle and provides a safe interface for JNI.
pub struct AndroidNode {
    config_path: String,
    data_dir: String,
    #[allow(dead_code)] // Will be used when MyriadNode is integrated
    runtime: Arc<Runtime>,
    // TODO: Add actual MyriadNode instance when ready
    // node: Option<Arc<Mutex<myriadnode::Node>>>,
    is_running: bool,
}

impl AndroidNode {
    /// Create a new AndroidNode instance.
    pub fn new(config_path: String, data_dir: String) -> Result<Self> {
        log::info!("Creating AndroidNode with config: {}", config_path);

        // Create a Tokio runtime for async operations
        let runtime = Runtime::new().context("Failed to create Tokio runtime")?;

        Ok(Self {
            config_path,
            data_dir,
            runtime: Arc::new(runtime),
            is_running: false,
        })
    }

    /// Start the node.
    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            log::warn!("Node is already running");
            return Ok(());
        }

        log::info!("Starting MyriadNode...");

        // TODO: Initialize and start the actual MyriadNode
        // For now, just mark as running
        self.is_running = true;

        log::info!("MyriadNode started successfully");
        Ok(())
    }

    /// Stop the node.
    pub fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            log::warn!("Node is not running");
            return Ok(());
        }

        log::info!("Stopping MyriadNode...");

        // TODO: Stop the actual MyriadNode
        self.is_running = false;

        log::info!("MyriadNode stopped successfully");
        Ok(())
    }

    /// Send a message through the mesh network.
    pub fn send_message(&self, destination: &str, payload: &[u8], priority: u8) -> Result<()> {
        if !self.is_running {
            anyhow::bail!("Node is not running");
        }

        log::debug!(
            "Sending message to {} with priority {} ({} bytes)",
            destination,
            priority,
            payload.len()
        );

        // TODO: Send message through actual MyriadNode
        // For now, just log it
        log::info!("Message would be sent to: {}", destination);

        Ok(())
    }

    /// Get the node's public ID.
    pub fn get_node_id(&self) -> Result<String> {
        // TODO: Get actual node ID from MyriadNode
        // For now, return a placeholder
        Ok("android-node-placeholder".to_string())
    }

    /// Get the node's status as JSON.
    pub fn get_status(&self) -> Result<String> {
        // TODO: Get actual status from MyriadNode
        // For now, return a simple JSON
        let status = serde_json::json!({
            "running": self.is_running,
            "config_path": self.config_path,
            "data_dir": self.data_dir,
        });

        Ok(status.to_string())
    }
}

impl Drop for AndroidNode {
    fn drop(&mut self) {
        if self.is_running {
            log::info!("AndroidNode being dropped while running, stopping...");
            let _ = self.stop();
        }
    }
}
