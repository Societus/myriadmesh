//! Plugin Architecture for MyriadMesh
//!
//! Provides extensibility through adapter plugins, application plugins,
//! and bridge plugins for community extensions.

use async_trait::async_trait;
use myriadmesh_protocol::{types::AdapterType, Frame};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::adapter::NetworkAdapter;
use crate::error::Result;

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin-specific configuration data
    pub config_data: serde_json::Value,
}

/// Plugin dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    /// Dependency name
    pub name: String,
    /// Minimum required version
    pub min_version: String,
}

/// Base plugin trait
#[async_trait]
pub trait MyriadMeshPlugin: Send + Sync {
    /// Get plugin name
    fn plugin_name(&self) -> &str;

    /// Get plugin version
    fn plugin_version(&self) -> &str;

    /// Get plugin author
    fn author(&self) -> &str;

    /// Get plugin description
    fn description(&self) -> &str;

    /// Initialize the plugin
    async fn initialize(&mut self, config: PluginConfig) -> Result<()>;

    /// Shutdown the plugin
    async fn shutdown(&mut self) -> Result<()>;

    /// Cast to Any for downcasting
    fn as_any(&self) -> &dyn Any;
}

/// Adapter plugin - adds network transport capability
#[async_trait]
pub trait AdapterPlugin: MyriadMeshPlugin + NetworkAdapter {
    /// Get hardware requirements for this adapter
    fn hardware_requirements(&self) -> Vec<String>;

    /// Get plugin dependencies
    fn dependencies(&self) -> Vec<PluginDependency>;

    /// Get adapter type
    fn adapter_type(&self) -> AdapterType;
}

/// HTTP method for REST endpoints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

/// REST endpoint provided by a plugin
pub struct RestEndpoint {
    /// Endpoint path
    pub path: String,
    /// HTTP method
    pub method: HttpMethod,
    /// Handler function
    pub handler: Arc<dyn Fn() + Send + Sync>,
}

/// Message handler for custom message types
pub struct MessageHandler {
    /// Message type ID
    pub message_type: u8,
    /// Handler function
    pub handler: Arc<dyn Fn(&Frame) -> Result<()> + Send + Sync>,
}

/// UI component type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentType {
    Dashboard,
    Settings,
    Status,
    Custom(String),
}

/// UI component provided by a plugin
pub struct UiComponent {
    /// Component ID
    pub component_id: String,
    /// Component title
    pub title: String,
    /// Component type
    pub component_type: ComponentType,
}

/// Application plugin - high-level functionality
#[async_trait]
pub trait ApplicationPlugin: MyriadMeshPlugin {
    /// Register message handler for custom message types
    fn register_message_handler(&self) -> Option<MessageHandler> {
        None
    }

    /// Provide REST API endpoints
    fn provide_rest_endpoints(&self) -> Vec<RestEndpoint> {
        Vec::new()
    }

    /// Provide UI components
    fn provide_ui_components(&self) -> Vec<UiComponent> {
        Vec::new()
    }
}

/// Bridge plugin - connects to external networks
#[async_trait]
pub trait BridgePlugin: MyriadMeshPlugin {
    /// Get bridge name
    fn bridge_name(&self) -> &str;

    /// Get list of supported external networks
    fn supported_networks(&self) -> Vec<String>;

    /// Translate inbound message from external network to MyriadMesh frame
    async fn translate_inbound(&self, from_network: &str, data: &[u8]) -> Result<Frame>;

    /// Translate outbound frame from MyriadMesh to external network format
    async fn translate_outbound(&self, frame: &Frame, to_network: &str) -> Result<Vec<u8>>;
}

/// Plugin registry for managing all plugins
pub struct PluginRegistry {
    /// Registered adapter plugins
    adapters: Arc<RwLock<HashMap<String, Arc<dyn AdapterPlugin>>>>,
    /// Registered application plugins
    applications: Arc<RwLock<HashMap<String, Arc<dyn ApplicationPlugin>>>>,
    /// Registered bridge plugins
    bridges: Arc<RwLock<HashMap<String, Arc<dyn BridgePlugin>>>>,
    /// Plugin directory path
    plugin_dir: PathBuf,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new(plugin_dir: PathBuf) -> Self {
        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            applications: Arc::new(RwLock::new(HashMap::new())),
            bridges: Arc::new(RwLock::new(HashMap::new())),
            plugin_dir,
        }
    }

    /// Register a core adapter plugin (built-in)
    pub async fn register_core_adapter(&self, adapter: Arc<dyn AdapterPlugin>) -> Result<()> {
        let name = adapter.plugin_name().to_string();
        log::info!("Registering core adapter plugin: {}", name);
        self.adapters.write().await.insert(name, adapter);
        Ok(())
    }

    /// Register an application plugin
    pub async fn register_application(&self, application: Arc<dyn ApplicationPlugin>) -> Result<()> {
        let name = application.plugin_name().to_string();
        log::info!("Registering application plugin: {}", name);
        self.applications.write().await.insert(name, application);
        Ok(())
    }

    /// Register a bridge plugin
    pub async fn register_bridge(&self, bridge: Arc<dyn BridgePlugin>) -> Result<()> {
        let name = bridge.plugin_name().to_string();
        log::info!("Registering bridge plugin: {}", name);
        self.bridges.write().await.insert(name, bridge);
        Ok(())
    }

    /// List all registered adapter plugins
    pub async fn list_adapters(&self) -> Vec<String> {
        self.adapters.read().await.keys().cloned().collect()
    }

    /// List all registered application plugins
    pub async fn list_applications(&self) -> Vec<String> {
        self.applications.read().await.keys().cloned().collect()
    }

    /// List all registered bridge plugins
    pub async fn list_bridges(&self) -> Vec<String> {
        self.bridges.read().await.keys().cloned().collect()
    }

    /// Get an adapter plugin by name
    pub async fn get_adapter(&self, name: &str) -> Option<Arc<dyn AdapterPlugin>> {
        self.adapters.read().await.get(name).cloned()
    }

    /// Get an application plugin by name
    pub async fn get_application(&self, name: &str) -> Option<Arc<dyn ApplicationPlugin>> {
        self.applications.read().await.get(name).cloned()
    }

    /// Get a bridge plugin by name
    pub async fn get_bridge(&self, name: &str) -> Option<Arc<dyn BridgePlugin>> {
        self.bridges.read().await.get(name).cloned()
    }

    /// Unregister an adapter plugin
    pub async fn unregister_adapter(&self, name: &str) -> Option<Arc<dyn AdapterPlugin>> {
        self.adapters.write().await.remove(name)
    }

    /// Unregister an application plugin
    pub async fn unregister_application(&self, name: &str) -> Option<Arc<dyn ApplicationPlugin>> {
        self.applications.write().await.remove(name)
    }

    /// Unregister a bridge plugin
    pub async fn unregister_bridge(&self, name: &str) -> Option<Arc<dyn BridgePlugin>> {
        self.bridges.write().await.remove(name)
    }

    /// Get plugin directory path
    pub fn plugin_dir(&self) -> &PathBuf {
        &self.plugin_dir
    }

    /// Load a plugin from a shared library (future implementation)
    ///
    /// This would use libloading or similar to dynamically load .so/.dylib/.dll files
    pub async fn load_plugin_from_file(&self, _plugin_path: &str) -> Result<()> {
        // Future implementation: dynamic loading
        // For now, plugins must be registered manually at compile time
        log::warn!("Dynamic plugin loading not yet implemented");
        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new(PathBuf::from("./plugins"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_registry_creation() {
        let registry = PluginRegistry::default();
        assert_eq!(registry.list_adapters().await.len(), 0);
        assert_eq!(registry.list_applications().await.len(), 0);
        assert_eq!(registry.list_bridges().await.len(), 0);
    }

    #[tokio::test]
    async fn test_plugin_registry_list_empty() {
        let registry = PluginRegistry::default();
        let adapters = registry.list_adapters().await;
        assert!(adapters.is_empty());
    }
}
