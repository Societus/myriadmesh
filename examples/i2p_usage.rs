//! Example: Using the I2P network adapter
//!
//! This example demonstrates how to use MyriadMesh with i2p for anonymous communication.
//!
//! # Prerequisites
//!
//! For this example to work, you need EITHER:
//! 1. A running i2p router (i2pd or Java I2P) with SAM enabled on port 7656, OR
//! 2. i2pd installed in your PATH (adapter will start it automatically)
//!
//! # Running
//!
//! ```bash
//! cargo run --example i2p_usage
//! ```

use myriadmesh_network::{AdapterManager, I2pAdapter, NetworkAdapter};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init();

    println!("=== MyriadMesh I2P Network Adapter Example ===\n");

    // Example 1: Basic I2P adapter creation and initialization
    println!("1. Creating I2P adapter with default configuration...");
    let mut adapter = I2pAdapter::new();

    println!("   - Auto-detecting i2p router...");
    println!("   - Will start embedded i2pd if needed...");

    match adapter.initialize().await {
        Ok(_) => {
            println!("   ✓ I2P adapter initialized successfully!\n");

            // Get our i2p destination
            if let Some(dest) = adapter.get_local_address() {
                println!("2. Our I2P destination:");
                println!("   {}\n", dest);
                println!("   This is your anonymous address on the i2p network.");
                println!("   Share this with peers who want to connect to you.\n");
            }

            // Show adapter capabilities
            let caps = adapter.get_capabilities();
            println!("3. I2P Adapter Capabilities:");
            println!("   - Max message size: {} bytes", caps.max_message_size);
            println!("   - Typical latency: {:.1}ms", caps.typical_latency_ms);
            println!("   - Reliability: {:.1}%", caps.reliability * 100.0);
            println!("   - Range: Global (anonymous)");
            println!("   - Cost: Free\n");

            adapter.stop().await?;
        }
        Err(e) => {
            println!("   ✗ Failed to initialize I2P adapter: {}", e);
            println!("\n   This is expected if:");
            println!("   - No i2p router is running on port 7656");
            println!("   - i2pd is not installed in PATH");
            println!("\n   To fix:");
            println!("   - Install i2pd: https://i2pd.readthedocs.io/");
            println!("   - Or start existing i2p router with SAM enabled\n");
        }
    }

    // Example 2: Using I2P with the adapter manager
    println!("4. Multi-adapter setup with I2P:\n");

    let mut manager = AdapterManager::new();

    // Register I2P adapter
    println!("   - Registering I2P adapter...");
    let i2p_adapter = Box::new(I2pAdapter::new()) as Box<dyn NetworkAdapter>;

    match manager.register_adapter("i2p".to_string(), i2p_adapter).await {
        Ok(_) => {
            println!("   ✓ I2P adapter registered");

            println!("\n5. Adapter Manager Status:");
            println!("   - Total adapters: {}", manager.adapter_count());
            println!("   - I2P available: Yes");

            // Show health check
            let health = manager.health_check_all().await;
            println!("\n6. Health Check:");
            for (id, status) in health {
                println!("   - {}: {:?}", id, status);
            }

            // Cleanup
            manager.stop_all().await?;
        }
        Err(e) => {
            println!("   ✗ Could not register I2P adapter: {}", e);
        }
    }

    println!("\n=== Example Complete ===");
    println!("\nNext steps:");
    println!("- Use I2P for Mode 2 (Selective Disclosure) privacy");
    println!("- Combine with onion routing for defense in depth");
    println!("- Exchange encrypted capability tokens over I2P");

    Ok(())
}
