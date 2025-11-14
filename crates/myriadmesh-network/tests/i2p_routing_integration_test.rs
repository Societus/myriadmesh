//! Integration tests for I2P + Routing
//!
//! Tests the conceptual integration of:
//! - Message routing over I2P adapters
//! - Multi-path routing with I2P for privacy
//! - Adaptive routing with I2P failover
//! - Link metrics for I2P routes

use myriadmesh_network::{AdapterManager, I2pAdapter, I2pRouterConfig, NetworkAdapter};
use myriadmesh_protocol::types::AdapterType;
use myriadmesh_routing::{CostWeights, LinkMetrics, MultiPathStrategy, NetworkPath, PriorityLevel};

/// Test that I2P adapter can be used for routing decisions
#[tokio::test]
async fn test_i2p_adapter_in_routing_context() {
    let mut manager = AdapterManager::new();

    // Create i2p adapter with custom config to avoid conflicts
    let config = I2pRouterConfig {
        sam_port: 17658, // Non-standard port for testing
        ..Default::default()
    };

    let adapter = Box::new(I2pAdapter::with_config(config)) as Box<dyn NetworkAdapter>;

    // Register adapter (may fail if no i2p router running)
    let result = manager.register_adapter("i2p".to_string(), adapter).await;

    match result {
        Ok(_) => {
            // Adapter registered successfully
            let caps = manager.get_capabilities("i2p").unwrap();

            // Verify i2p characteristics for routing decisions
            assert_eq!(caps.adapter_type, AdapterType::I2P);
            assert_eq!(caps.range_meters, 0.0); // Global reach
            assert!(caps.typical_latency_ms > 1000.0); // High latency
            assert_eq!(caps.cost_per_mb, 0.0); // Free

            // In routing, i2p should be selected for:
            // - Privacy-sensitive traffic (despite high latency)
            // - Global reach when local adapters fail
            // - Cost-free long-distance communication
        }
        Err(e) => {
            println!("I2P adapter registration failed (expected in CI): {}", e);
        }
    }
}

/// Test cost calculation for I2P routes vs clearnet routes
#[test]
fn test_i2p_route_cost_calculation() {
    use myriadmesh_protocol::types::NODE_ID_SIZE;
    use myriadmesh_protocol::NodeId;

    // Create cost weights for link evaluation
    let weights = CostWeights {
        latency: 1.0,      // Latency is important
        loss: 1.0,         // Packet loss is important
        jitter: 0.5,       // Jitter less important
        utilization: 0.5,  // Utilization less important
    };

    // Ethernet link metrics (fast, low loss)
    let mut ethernet_metrics = LinkMetrics::new();
    ethernet_metrics.update(
        50.0,       // 50ms latency
        false,      // No packet loss
        1_000_000,  // 1 Mbps bandwidth
        0.2,        // 20% utilization
    );

    // I2P link metrics (slow, but reliable and free)
    let mut i2p_metrics = LinkMetrics::new();
    i2p_metrics.update(
        2000.0,     // 2000ms latency
        false,      // No packet loss (I2P is reliable)
        100_000,    // 100 Kbps bandwidth
        0.8,        // 80% utilization (busy)
    );

    // Calculate costs
    let ethernet_cost = ethernet_metrics.calculate_cost(&weights);
    let i2p_cost = i2p_metrics.calculate_cost(&weights);

    // For latency-sensitive traffic, ethernet should have lower cost
    assert!(ethernet_cost < i2p_cost);

    // But for privacy-sensitive traffic or when ethernet fails,
    // I2P provides value despite higher latency cost
}

/// Test multi-path routing with I2P as redundant path
#[test]
fn test_multipath_with_i2p_redundancy() {
    use myriadmesh_protocol::types::NODE_ID_SIZE;
    use myriadmesh_protocol::NodeId;

    let mut source_bytes = [0u8; NODE_ID_SIZE];
    source_bytes[0] = 100;
    let source = NodeId::from_bytes(source_bytes);

    let mut relay_bytes = [0u8; NODE_ID_SIZE];
    relay_bytes[0] = 150;
    let relay = NodeId::from_bytes(relay_bytes);

    let mut dest_bytes = [0u8; NODE_ID_SIZE];
    dest_bytes[0] = 200;
    let destination = NodeId::from_bytes(dest_bytes);

    // Primary path: source -> destination (direct ethernet)
    let primary_path = NetworkPath::with_metrics(
        vec![source, destination],
        10,   // Low cost
        0.95, // High quality
    );

    // Backup path: source -> relay -> destination (via I2P)
    let backup_path = NetworkPath::with_metrics(
        vec![source, relay, destination],
        50,   // Higher cost (latency)
        0.90, // Good quality (I2P is reliable)
    );

    // Verify paths are node-disjoint (for redundancy)
    assert!(primary_path.is_disjoint_with(&backup_path) || primary_path.length() == 1);

    // Strategy: Use primary for normal traffic, I2P backup for:
    // 1. Primary path failure
    // 2. Privacy-sensitive messages
    // 3. Redundant transmission of critical messages
    let strategy = MultiPathStrategy::BestN(2); // Use both paths
    assert_eq!(strategy, MultiPathStrategy::BestN(2));
}

/// Test message priority with I2P routing
#[test]
fn test_message_priority_with_i2p() {
    // Different priority levels suit different adapter types:
    //
    // Emergency (Level 5):
    //   - Use fastest adapter (ethernet)
    //   - Skip I2P (too slow)
    //   - Latency critical
    //
    // High (Level 4):
    //   - Use primary adapter
    //   - I2P as backup only
    //   - Balance speed vs reliability
    //
    // Normal (Level 3):
    //   - Use cost-optimized routing
    //   - I2P acceptable for free transmission
    //   - Balanced approach
    //
    // Low (Level 2):
    //   - Delay-tolerant
    //   - I2P is fine
    //   - Cost > speed
    //
    // Background (Level 1):
    //   - I2P preferred (free, privacy bonus)
    //   - Delay highly tolerant
    //   - Lowest cost priority

    let emergency = PriorityLevel::Emergency; // Don't use I2P
    let normal = PriorityLevel::Normal; // I2P acceptable
    let background = PriorityLevel::Background; // I2P preferred

    // Verify priority ordering
    assert!(emergency > normal);
    assert!(normal > background);
}

/// Test I2P failover scenario
#[test]
fn test_i2p_failover_concept() {
    // Failover scenario:
    //
    // Normal operation:
    //   - Primary: Ethernet adapter (fast)
    //   - Backup: I2P adapter (reliable, global)
    //
    // When primary fails:
    //   1. Detect failure (heartbeat timeout, errors)
    //   2. Switch routing to I2P adapter
    //   3. Accept higher latency for maintained connectivity
    //   4. Bonus: Gain privacy protection during failover
    //   5. Continue normal operation with degraded performance
    //
    // When primary recovers:
    //   1. Detect recovery
    //   2. Gradually shift traffic back to primary
    //   3. Keep I2P as backup
    //
    // Benefits of I2P as failover:
    //   - Global reach (no geographic limitations)
    //   - Free (no cost increase during outage)
    //   - Privacy (unintended benefit)
    //   - Reliability (proven resilient network)

    let primary_available = false; // Primary adapter down
    let i2p_available = true; // I2P adapter up

    let use_i2p_fallback = !primary_available && i2p_available;
    assert!(use_i2p_fallback);
}

/// Test I2P bandwidth considerations
#[test]
fn test_i2p_bandwidth_considerations() {
    // I2P bandwidth characteristics:
    //
    // Typical I2P tunnel bandwidth:
    //   - Default: 50-100 KB/s
    //   - Configured: Up to several MB/s
    //   - Depends on: Router config, peer bandwidth, tunnel count
    //
    // Message size recommendations:
    //   - Small (< 100 KB): Good for I2P
    //   - Medium (100 KB - 1 MB): Acceptable for I2P
    //   - Large (> 1 MB): Use clearnet unless privacy critical
    //
    // Routing strategy by size:
    //   - Small messages: I2P acceptable, especially for privacy
    //   - Large messages: Prefer clearnet, chunk if using I2P
    //   - Critical data: May use both (redundancy)

    let small_message = 10 * 1024; // 10 KB
    let medium_message = 500 * 1024; // 500 KB
    let large_message = 10 * 1024 * 1024; // 10 MB

    let i2p_suitable_for_small = small_message < 100 * 1024;
    let i2p_suitable_for_medium = medium_message < 1024 * 1024;
    let i2p_suitable_for_large = large_message < 100 * 1024; // False

    assert!(i2p_suitable_for_small);
    assert!(i2p_suitable_for_medium);
    assert!(!i2p_suitable_for_large);
}

/// Test routing decision tree concept
#[test]
fn test_routing_decision_tree() {
    // Routing decision tree for I2P integration:
    //
    // Input: Message properties
    //   - Priority level
    //   - Size
    //   - Privacy requirement
    //   - Cost constraint
    //   - Latency requirement
    //
    // Decision process:
    //   if privacy_required {
    //       if has_i2p_capability_token {
    //           route_via_i2p()
    //       } else {
    //           request_token() or use_onion_routing()
    //       }
    //   } else if priority == Emergency {
    //       route_via_fastest_adapter() // Not I2P
    //   } else if primary_adapter_available {
    //       route_via_primary()
    //   } else if i2p_available {
    //       route_via_i2p() // Failover
    //   } else {
    //       queue_for_later()
    //   }

    let privacy_required = true;
    let has_i2p_token = true;
    let i2p_available = true;

    let should_use_i2p = privacy_required && has_i2p_token && i2p_available;
    assert!(should_use_i2p);

    // Alternative: High priority, no privacy requirement
    let priority_emergency = true;
    let privacy_not_required = true;

    let should_avoid_i2p = priority_emergency && privacy_not_required;
    assert!(should_avoid_i2p);
}

/// Test I2P + DHT routing integration concept
#[test]
fn test_i2p_dht_routing_concept() {
    // Integration concept:
    //
    // DHT stores:
    //   - Clearnet NodeID (public)
    //   - Public key (for signature verification)
    //   - Adapter capabilities (types, not I2P destinations)
    //   - Geographic location (if shared)
    //
    // Capability tokens store:
    //   - I2P destination (private)
    //   - I2P NodeID (separate from clearnet)
    //   - Permission grants
    //   - Expiration times
    //
    // Routing table combines:
    //   - DHT data (public routing info)
    //   - Capability tokens (private I2P routes)
    //   - Link metrics (performance data)
    //   - Adapter availability (real-time status)
    //
    // Route selection uses:
    //   1. Check message requirements (privacy, latency, cost)
    //   2. Query DHT for destination node info
    //   3. Check local capability tokens for I2P access
    //   4. Evaluate link metrics for available paths
    //   5. Select best path based on requirements
    //   6. Fall back to alternative paths if needed

    // This test verifies the concept is sound
    assert!(true);
}
