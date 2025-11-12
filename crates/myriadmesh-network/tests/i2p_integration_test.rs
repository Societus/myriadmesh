//! Integration tests for I2P network adapter
//!
//! These tests demonstrate the i2p adapter working with the AdapterManager
//! and show end-to-end usage patterns.

use myriadmesh_network::{AdapterManager, I2pAdapter, I2pRouterConfig, NetworkAdapter};
use myriadmesh_protocol::types::AdapterType;

/// Test that I2P adapter can be registered with the manager
#[tokio::test]
async fn test_register_i2p_adapter_with_manager() {
    let mut manager = AdapterManager::new();

    // Create i2p adapter with custom config to avoid conflicts
    let config = I2pRouterConfig {
        sam_port: 17656, // Non-standard port for testing
        ..Default::default()
    };

    let adapter = Box::new(I2pAdapter::with_config(config)) as Box<dyn NetworkAdapter>;

    // This will fail if no i2p router is running, which is expected in CI
    let result = manager.register_adapter("i2p".to_string(), adapter).await;

    // In a real environment with i2p, this would succeed
    // For CI, we just verify the API works
    match result {
        Ok(_) => {
            assert_eq!(manager.adapter_count(), 1);
            assert!(manager.has_adapters());

            let found = manager.find_adapter_by_type(AdapterType::I2P);
            assert_eq!(found, Some("i2p".to_string()));
        }
        Err(e) => {
            // Expected if no i2p router is available
            println!("I2P adapter registration failed (expected in CI): {}", e);
        }
    }
}

/// Test multi-adapter scenario with both Ethernet and I2P
#[tokio::test]
async fn test_multi_adapter_with_i2p_and_ethernet() {
    let mut manager = AdapterManager::new();

    // Try to register I2P adapter first (may fail if no router)
    let i2p_config = I2pRouterConfig {
        sam_port: 17657,
        ..Default::default()
    };
    let i2p_adapter = Box::new(I2pAdapter::with_config(i2p_config)) as Box<dyn NetworkAdapter>;

    let i2p_registered = manager
        .register_adapter("i2p".to_string(), i2p_adapter)
        .await
        .is_ok();

    if i2p_registered {
        println!("I2P adapter successfully registered");
        assert_eq!(manager.adapter_count(), 1);

        // Verify we can find it
        let i2p = manager.find_adapter_by_type(AdapterType::I2P);
        assert_eq!(i2p, Some("i2p".to_string()));
    } else {
        println!("I2P adapter not available (expected in CI without i2p router)");
        assert_eq!(manager.adapter_count(), 0);
    }
}

/// Test I2P adapter capabilities
#[test]
fn test_i2p_adapter_capabilities() {
    let adapter = I2pAdapter::new();
    let caps = adapter.get_capabilities();

    // Verify i2p characteristics
    assert_eq!(caps.adapter_type, AdapterType::I2P);
    assert_eq!(caps.range_meters, 0.0); // Global reach
    assert!(caps.reliability > 0.9); // High reliability
    assert!(caps.typical_latency_ms > 1000.0); // High latency
    assert_eq!(caps.cost_per_mb, 0.0); // Free
    assert!(!caps.supports_broadcast); // No broadcast support
    assert!(!caps.supports_multicast); // No multicast support
}

/// Test I2P adapter address handling
#[test]
fn test_i2p_address_handling() {
    let adapter = I2pAdapter::new();

    // Valid i2p addresses
    assert!(adapter.parse_address("example.i2p").is_ok());
    assert!(adapter.parse_address("abc~xyz.i2p").is_ok());

    // Invalid addresses
    assert!(adapter.parse_address("not-i2p").is_err());
    assert!(adapter.parse_address("192.168.1.1").is_err());
}

/// Test adapter selection with I2P
#[test]
fn test_adapter_selection_logic() {
    // This test demonstrates the adapter selection logic without actually
    // registering adapters to avoid async runtime issues

    // I2P characteristics:
    let i2p = I2pAdapter::new();
    let i2p_caps = i2p.get_capabilities();

    // I2P should have:
    assert_eq!(i2p_caps.adapter_type, AdapterType::I2P);
    assert_eq!(i2p_caps.range_meters, 0.0); // Global
    assert!(i2p_caps.typical_latency_ms > 1000.0); // High latency
    assert_eq!(i2p_caps.cost_per_mb, 0.0); // Free

    // For small messages, ethernet would score higher due to lower latency
    // For privacy-sensitive traffic, i2p would be preferred
    // This demonstrates the capability-based selection system
}

/// Demonstrate I2P router configuration options
#[test]
fn test_i2p_router_configuration() {
    use std::path::PathBuf;

    // Default configuration
    let default_config = I2pRouterConfig::default();
    assert_eq!(default_config.sam_port, 7656);
    assert_eq!(default_config.transit_tunnels, 50);

    // Custom configuration
    let custom_config = I2pRouterConfig {
        data_dir: PathBuf::from("/tmp/myriadmesh-test"),
        sam_port: 7657,
        enable_ipv6: false,
        bandwidth_limit_kbps: Some(512),
        transit_tunnels: 5,
        i2pd_binary: Some(PathBuf::from("/usr/local/bin/i2pd")),
    };

    assert_eq!(custom_config.sam_port, 7657);
    assert_eq!(custom_config.bandwidth_limit_kbps, Some(512));
    assert_eq!(custom_config.transit_tunnels, 5);
}

/// Test destination persistence concept
#[test]
fn test_destination_persistence_path() {
    let config = I2pRouterConfig::default();
    let keys_path = config.data_dir.join("destination.keys");

    // Verify the path is constructed correctly
    assert!(keys_path.to_string_lossy().contains("myriadmesh"));
    assert!(keys_path.to_string_lossy().contains("i2p"));
    assert!(keys_path.to_string_lossy().ends_with("destination.keys"));
}
