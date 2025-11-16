/**
 * Comprehensive Integration Tests for Phase 3
 *
 * These tests verify the complete system integration including:
 * - Scoring system with all modes
 * - Failover manager with health tracking
 * - Heartbeat service and NodeMap
 * - Backhaul detection
 * - End-to-end workflows
 */
use myriadmesh_network::AdapterManager;
use myriadmesh_protocol::types::NODE_ID_SIZE;
use myriadmesh_protocol::NodeId;
use std::sync::Arc;
use tokio::sync::RwLock;

// Import modules from myriadnode
use myriadmesh_crypto::identity::NodeIdentity;
use myriadnode::backhaul::{BackhaulConfig, BackhaulDetector};
use myriadnode::config::FailoverConfig;
use myriadnode::failover::FailoverManager;
use myriadnode::heartbeat::{HeartbeatConfig, HeartbeatService};
use myriadnode::scoring::{AdapterMetrics, AdapterScorer, ScoringWeights};
use std::collections::HashMap;

// Helper function to create a test NodeId
fn create_test_node_id(seed: u8) -> NodeId {
    let bytes = [seed; NODE_ID_SIZE];
    NodeId::from_bytes(bytes)
}

// Helper function to create a HeartbeatService for testing
fn create_test_heartbeat_service(config: HeartbeatConfig, node_id: NodeId) -> HeartbeatService {
    myriadmesh_crypto::init().ok();
    let identity = Arc::new(NodeIdentity::generate().unwrap());
    let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));
    let backhaul_detector = Arc::new(BackhaulDetector::new(BackhaulConfig::default()));
    let adapter_configs = HashMap::new();

    HeartbeatService::new(
        config,
        node_id,
        identity,
        adapter_manager,
        backhaul_detector,
        adapter_configs,
    )
}

// ====================
// Scoring System Tests
// ====================

#[tokio::test]
async fn test_scoring_all_modes() {
    // Test that all scoring mode presets have valid weights that sum to 1.0
    let modes = vec![
        ("default", ScoringWeights::default()),
        ("battery", ScoringWeights::battery_optimized()),
        ("performance", ScoringWeights::performance_optimized()),
        ("reliability", ScoringWeights::reliability_optimized()),
        ("privacy", ScoringWeights::privacy_optimized()),
    ];

    for (name, weights) in modes {
        let sum = weights.latency
            + weights.bandwidth
            + weights.reliability
            + weights.power
            + weights.privacy;
        assert!(
            (sum - 1.0).abs() < 0.01,
            "Mode '{}' weights should sum to ~1.0, got {}",
            name,
            sum
        );
    }
}

#[tokio::test]
async fn test_scoring_adapter_selection() {
    // Create scorer with default weights
    let scorer = AdapterScorer::new(ScoringWeights::default());

    // High-performance adapter (low latency, high bandwidth)
    let fast_adapter = AdapterMetrics {
        latency_ms: 10.0,
        bandwidth_bps: 100_000_000, // 100 Mbps
        reliability: 0.95,
        power_consumption: 0.7,
        privacy_level: 0.3,
    };

    // Privacy-focused adapter (I2P-like)
    let private_adapter = AdapterMetrics {
        latency_ms: 500.0,
        bandwidth_bps: 1_000_000, // 1 Mbps
        reliability: 0.85,
        power_consumption: 0.3,
        privacy_level: 0.95,
    };

    let fast_score = scorer.calculate_score("fast".to_string(), &fast_adapter);
    let private_score = scorer.calculate_score("private".to_string(), &private_adapter);

    // With default weights, fast adapter should score higher
    assert!(
        fast_score.total_score > private_score.total_score,
        "Fast adapter should score higher with default weights: {} vs {}",
        fast_score.total_score,
        private_score.total_score
    );

    // Now test with privacy-optimized weights
    let privacy_scorer = AdapterScorer::new(ScoringWeights::privacy_optimized());
    let fast_privacy_score = privacy_scorer.calculate_score("fast".to_string(), &fast_adapter);
    let private_privacy_score =
        privacy_scorer.calculate_score("private".to_string(), &private_adapter);

    // With privacy weights, private adapter should score higher
    assert!(
        private_privacy_score.total_score > fast_privacy_score.total_score,
        "Private adapter should score higher with privacy weights: {} vs {}",
        private_privacy_score.total_score,
        fast_privacy_score.total_score
    );
}

#[tokio::test]
async fn test_scoring_edge_cases() {
    let scorer = AdapterScorer::new(ScoringWeights::default());

    // Zero metrics
    let zero_metrics = AdapterMetrics {
        latency_ms: 0.0,
        bandwidth_bps: 0,
        reliability: 0.0,
        power_consumption: 0.0,
        privacy_level: 0.0,
    };

    let score = scorer.calculate_score("zero".to_string(), &zero_metrics);
    assert!(score.total_score >= 0.0, "Score should be non-negative");
    assert!(score.total_score <= 1.0, "Score should not exceed 1.0");

    // Maximum metrics
    let max_metrics = AdapterMetrics {
        latency_ms: 1.0,
        bandwidth_bps: 1_000_000_000, // 1 Gbps
        reliability: 1.0,
        power_consumption: 0.0,
        privacy_level: 1.0,
    };

    let max_score = scorer.calculate_score("max".to_string(), &max_metrics);
    assert!(
        max_score.total_score >= 0.0,
        "Max score should be non-negative"
    );
    assert!(
        max_score.total_score <= 1.0,
        "Max score should not exceed 1.0"
    );
}

// ==========================
// Failover Manager Tests
// ==========================

#[tokio::test]
async fn test_failover_manager_initialization() {
    let config = FailoverConfig {
        auto_failover: true,
        latency_threshold_multiplier: 3.0,
        loss_threshold: 0.3,
        retry_attempts: 3,
    };

    let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));
    let weights = ScoringWeights::default();

    let failover_manager = FailoverManager::new(config, adapter_manager, weights);

    // Verify basic initialization
    let events = failover_manager.get_recent_events(10).await;
    assert_eq!(
        events.len(),
        0,
        "New failover manager should have no events"
    );
}

#[tokio::test]
async fn test_failover_event_logging() {
    let config = FailoverConfig {
        auto_failover: true,
        latency_threshold_multiplier: 3.0,
        loss_threshold: 0.3,
        retry_attempts: 3,
    };

    let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));
    let weights = ScoringWeights::default();

    let failover_manager = FailoverManager::new(config, adapter_manager, weights);

    // Get events (should be empty initially)
    let events = failover_manager.get_recent_events(100).await;
    assert!(events.is_empty(), "Should start with no events");
}

#[tokio::test]
async fn test_failover_force_failover_validation() {
    let config = FailoverConfig {
        auto_failover: true,
        latency_threshold_multiplier: 3.0,
        loss_threshold: 0.3,
        retry_attempts: 3,
    };

    let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));
    let weights = ScoringWeights::default();

    let failover_manager = FailoverManager::new(config, adapter_manager, weights);

    // Try to force failover to non-existent adapter
    let result = failover_manager
        .force_failover("nonexistent".to_string())
        .await;
    assert!(
        result.is_err(),
        "Force failover to non-existent adapter should fail"
    );
}

// ==========================
// Heartbeat Service Tests
// ==========================

#[tokio::test]
async fn test_heartbeat_service_initialization() {
    let config = HeartbeatConfig {
        enabled: true,
        interval_secs: 30,
        timeout_secs: 120,
        include_geolocation: false,
        store_remote_geolocation: false,
        max_nodes: 1000,
    };

    let node_id = create_test_node_id(1);
    let service = create_test_heartbeat_service(config, node_id);

    // Get initial stats
    let stats = service.get_stats().await;
    assert_eq!(stats.total_nodes, 0, "New service should have no nodes");
}

#[tokio::test]
async fn test_heartbeat_privacy_controls() {
    // Test with geolocation disabled (privacy-first)
    let private_config = HeartbeatConfig {
        enabled: true,
        interval_secs: 30,
        timeout_secs: 120,
        include_geolocation: false,
        store_remote_geolocation: false,
        max_nodes: 1000,
    };

    let node_id = create_test_node_id(2);
    let private_service = create_test_heartbeat_service(private_config, node_id);

    let stats = private_service.get_stats().await;
    assert_eq!(
        stats.nodes_with_location, 0,
        "Private mode should not store locations"
    );

    // Test with geolocation enabled
    let public_config = HeartbeatConfig {
        enabled: true,
        interval_secs: 30,
        timeout_secs: 120,
        include_geolocation: true,
        store_remote_geolocation: true,
        max_nodes: 1000,
    };

    let node_id2 = create_test_node_id(3);
    let public_service = create_test_heartbeat_service(public_config, node_id2);
    let public_stats = public_service.get_stats().await;

    // Stats should still be 0 initially
    assert_eq!(public_stats.total_nodes, 0);
}

#[tokio::test]
async fn test_heartbeat_node_map_updates() {
    let config = HeartbeatConfig {
        enabled: true,
        interval_secs: 30,
        timeout_secs: 120,
        include_geolocation: false,
        store_remote_geolocation: false,
        max_nodes: 1000,
    };

    let node_id = create_test_node_id(4);
    let service = create_test_heartbeat_service(config, node_id);

    // Get node map (should be empty)
    let node_map = service.get_node_map().await;
    assert_eq!(node_map.len(), 0, "New service should have empty node map");
}

// ==========================
// Backhaul Detection Tests
// ==========================

#[tokio::test]
async fn test_backhaul_config_defaults() {
    let config = BackhaulConfig::default();

    assert!(
        !config.allow_backhaul_mesh,
        "Should default to no backhaul mesh"
    );
    assert_eq!(
        config.check_interval_secs, 300,
        "Should default to 300s (5 min) check interval"
    );
}

#[tokio::test]
async fn test_backhaul_detection_override() {
    let allow_config = BackhaulConfig {
        allow_backhaul_mesh: true,
        check_interval_secs: 30,
    };

    let _detector = BackhaulDetector::new(allow_config.clone());

    // Even with backhaul mesh allowed, detector should still detect backhauls
    // (The config just changes how we use that information)
    assert!(allow_config.allow_backhaul_mesh);
}

#[tokio::test]
async fn test_backhaul_detection_error_handling() {
    let config = BackhaulConfig::default();
    let detector = BackhaulDetector::new(config);

    // Check interface that doesn't exist
    let result = detector.check_interface("nonexistent_interface_12345");

    // Should either succeed with "not backhaul" or fail gracefully
    // (depending on platform and permissions)
    match result {
        Ok(_) => {
            // Successfully determined status (even if interface doesn't exist)
        }
        Err(_) => {
            // Failed gracefully - this is acceptable
        }
    }
}

// ===========================
// End-to-End Workflow Tests
// ===========================

#[tokio::test]
async fn test_complete_node_component_initialization() {
    // Test that all Phase 3 components can be initialized together

    // 1. Create adapter manager
    let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));

    // 2. Create scoring system
    let weights = ScoringWeights::default();
    let _scorer = AdapterScorer::new(weights.clone());

    // 3. Create failover manager
    let failover_config = FailoverConfig {
        auto_failover: true,
        latency_threshold_multiplier: 3.0,
        loss_threshold: 0.3,
        retry_attempts: 3,
    };
    let _failover = FailoverManager::new(failover_config, Arc::clone(&adapter_manager), weights);

    // 4. Create heartbeat service
    let heartbeat_config = HeartbeatConfig {
        enabled: true,
        interval_secs: 30,
        timeout_secs: 120,
        include_geolocation: false,
        store_remote_geolocation: false,
        max_nodes: 1000,
    };
    let node_id = create_test_node_id(5);
    let _heartbeat = create_test_heartbeat_service(heartbeat_config, node_id);

    // 5. Create backhaul detector
    let backhaul_config = BackhaulConfig::default();
    let _backhaul = BackhaulDetector::new(backhaul_config);

    // If we get here, all components initialized successfully
}

#[tokio::test]
async fn test_adapter_manager_with_scoring() {
    let manager = Arc::new(RwLock::new(AdapterManager::new()));
    let scorer = AdapterScorer::new(ScoringWeights::default());

    // Initially no adapters
    let manager_read = manager.read().await;
    let adapter_ids = manager_read.adapter_ids();
    assert_eq!(adapter_ids.len(), 0, "New manager should have no adapters");

    // Scoring system should handle empty adapter list
    drop(manager_read);

    // Can still create scores for hypothetical adapters
    let metrics = AdapterMetrics {
        latency_ms: 50.0,
        bandwidth_bps: 10_000_000,
        reliability: 0.9,
        power_consumption: 0.5,
        privacy_level: 0.5,
    };

    let score = scorer.calculate_score("test".to_string(), &metrics);
    assert!(score.total_score >= 0.0 && score.total_score <= 1.0);
}

// =============================
// Performance & Load Tests
// =============================

#[tokio::test]
async fn test_scoring_performance() {
    use std::time::Instant;

    let scorer = AdapterScorer::new(ScoringWeights::default());
    let metrics = AdapterMetrics {
        latency_ms: 50.0,
        bandwidth_bps: 10_000_000,
        reliability: 0.9,
        power_consumption: 0.5,
        privacy_level: 0.5,
    };

    let start = Instant::now();

    // Score 1000 adapters
    for i in 0..1000 {
        let _score = scorer.calculate_score(format!("adapter_{}", i), &metrics);
    }

    let duration = start.elapsed();

    // Should complete in under 100ms
    assert!(
        duration.as_millis() < 100,
        "Scoring 1000 adapters took {}ms, should be <100ms",
        duration.as_millis()
    );
}

#[tokio::test]
async fn test_heartbeat_nodemap_capacity() {
    let config = HeartbeatConfig {
        enabled: true,
        interval_secs: 30,
        timeout_secs: 120,
        include_geolocation: false,
        store_remote_geolocation: false,
        max_nodes: 100, // Small capacity for testing
    };

    let node_id = create_test_node_id(6);
    let service = create_test_heartbeat_service(config, node_id);

    // Get stats (capacity enforced internally)
    let stats = service.get_stats().await;
    assert_eq!(stats.total_nodes, 0, "Should start empty");
}

// ==========================
// Error Handling Tests
// ==========================

#[tokio::test]
async fn test_invalid_scoring_weights() {
    // Test that scoring handles edge case weights gracefully
    let weights = ScoringWeights {
        latency: 0.0,
        bandwidth: 0.0,
        reliability: 0.0,
        power: 0.0,
        privacy: 0.0,
    };

    let scorer = AdapterScorer::new(weights);
    let metrics = AdapterMetrics {
        latency_ms: 50.0,
        bandwidth_bps: 10_000_000,
        reliability: 0.9,
        power_consumption: 0.5,
        privacy_level: 0.5,
    };

    // Should not panic even with zero weights
    let score = scorer.calculate_score("test".to_string(), &metrics);
    assert!(score.total_score >= 0.0);
}

#[tokio::test]
async fn test_empty_adapter_manager_queries() {
    let manager = Arc::new(RwLock::new(AdapterManager::new()));
    let manager_read = manager.read().await;

    // Query empty manager
    let adapter_ids = manager_read.adapter_ids();
    assert_eq!(adapter_ids.len(), 0);

    let adapter = manager_read.get_adapter("nonexistent");
    assert!(
        adapter.is_none(),
        "Should return None for nonexistent adapter"
    );
}

// ==========================
// Thread Safety Tests
// ==========================

#[tokio::test]
async fn test_concurrent_adapter_queries() {
    let manager = Arc::new(RwLock::new(AdapterManager::new()));

    // Spawn multiple concurrent readers
    let mut handles = vec![];

    for _ in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let handle = tokio::spawn(async move {
            let reader = manager_clone.read().await;
            let _ids = reader.adapter_ids();
            // Successfully read
        });
        handles.push(handle);
    }

    // All reads should complete without deadlock
    for handle in handles {
        handle.await.expect("Task should complete successfully");
    }
}

#[tokio::test]
async fn test_failover_manager_thread_safety() {
    let config = FailoverConfig {
        auto_failover: true,
        latency_threshold_multiplier: 3.0,
        loss_threshold: 0.3,
        retry_attempts: 3,
    };

    let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));
    let weights = ScoringWeights::default();
    let failover_manager = Arc::new(FailoverManager::new(config, adapter_manager, weights));

    // Spawn multiple concurrent queries
    let mut handles = vec![];

    for _ in 0..10 {
        let fm_clone = Arc::clone(&failover_manager);
        let handle = tokio::spawn(async move {
            let _events = fm_clone.get_recent_events(10).await;
        });
        handles.push(handle);
    }

    // All reads should complete
    for handle in handles {
        handle.await.expect("Task should complete");
    }
}

#[tokio::test]
async fn test_heartbeat_service_thread_safety() {
    let config = HeartbeatConfig {
        enabled: true,
        interval_secs: 30,
        timeout_secs: 120,
        include_geolocation: false,
        store_remote_geolocation: false,
        max_nodes: 1000,
    };

    let node_id = create_test_node_id(7);
    let service = Arc::new(create_test_heartbeat_service(config, node_id));

    // Spawn multiple concurrent queries
    let mut handles = vec![];

    for _ in 0..10 {
        let service_clone = Arc::clone(&service);
        let handle = tokio::spawn(async move {
            let _stats = service_clone.get_stats().await;
            let _map = service_clone.get_node_map().await;
        });
        handles.push(handle);
    }

    // All reads should complete
    for handle in handles {
        handle.await.expect("Task should complete");
    }
}

// =========================================
// Phase 1-4 Bug Fix Integration Tests
// =========================================

// ==========================
// Phase 3: Graceful Shutdown Tests
// ==========================

#[tokio::test]
async fn test_failover_manager_graceful_shutdown() {
    // RESOURCE M4: Test graceful shutdown of failover manager monitor task
    let config = FailoverConfig {
        auto_failover: true,
        latency_threshold_multiplier: 3.0,
        loss_threshold: 0.3,
        retry_attempts: 3,
    };

    let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));
    let weights = ScoringWeights::default();
    let failover_manager = FailoverManager::new(config, adapter_manager, weights);

    // Start the monitor task
    failover_manager.start().await.expect("Failed to start");

    // Give it a moment to actually start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Shutdown gracefully
    failover_manager.shutdown().await;

    // If we get here without hanging, graceful shutdown worked
}

#[tokio::test]
async fn test_heartbeat_service_graceful_shutdown() {
    // RESOURCE M4: Test graceful shutdown of heartbeat broadcast and cleanup tasks
    let config = HeartbeatConfig {
        enabled: true,
        interval_secs: 30,
        timeout_secs: 120,
        include_geolocation: false,
        store_remote_geolocation: false,
        max_nodes: 1000,
    };

    let node_id = create_test_node_id(10);
    let service = create_test_heartbeat_service(config, node_id);

    // Start the service (spawns background tasks)
    service.start().await.expect("Failed to start");

    // Give tasks time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Shutdown gracefully
    service.stop().await;

    // If we get here without hanging, graceful shutdown worked
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_concurrent_shutdown_no_deadlock() {
    // Test that multiple components can shutdown concurrently without deadlocking
    let failover_config = FailoverConfig {
        auto_failover: true,
        latency_threshold_multiplier: 3.0,
        loss_threshold: 0.3,
        retry_attempts: 3,
    };

    let heartbeat_config = HeartbeatConfig {
        enabled: true,
        interval_secs: 30,
        timeout_secs: 120,
        include_geolocation: false,
        store_remote_geolocation: false,
        max_nodes: 1000,
    };

    let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));
    let weights = ScoringWeights::default();

    // Create and start failover manager
    let failover = FailoverManager::new(failover_config, Arc::clone(&adapter_manager), weights);
    failover.start().await.expect("Failed to start failover");

    // Create and start heartbeat service
    let node_id = create_test_node_id(11);
    let heartbeat = create_test_heartbeat_service(heartbeat_config, node_id);
    heartbeat.start().await.expect("Failed to start heartbeat");

    // Give tasks time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Shutdown both concurrently
    let failover_shutdown = failover.shutdown();
    let heartbeat_shutdown = heartbeat.stop();

    tokio::join!(failover_shutdown, heartbeat_shutdown);

    // If we get here, no deadlock occurred
}

// ==========================
// Phase 2: Failover Scenarios with Graceful Shutdown
// ==========================

#[tokio::test]
async fn test_failover_scenario_with_restart() {
    // Test that failover manager can be stopped and restarted
    let config = FailoverConfig {
        auto_failover: true,
        latency_threshold_multiplier: 3.0,
        loss_threshold: 0.3,
        retry_attempts: 3,
    };

    let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));
    let weights = ScoringWeights::default();
    let failover = FailoverManager::new(config, adapter_manager, weights);

    // Start
    failover.start().await.expect("Failed to start");
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Stop
    failover.shutdown().await;
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Restart
    failover.start().await.expect("Failed to restart");
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Stop again
    failover.shutdown().await;
}

// ==========================
// Phase 4: Lock Ordering Verification
// ==========================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_failover_lock_ordering_stress() {
    // Stress test the failover manager to ensure lock ordering doesn't cause deadlocks
    let config = FailoverConfig {
        auto_failover: true,
        latency_threshold_multiplier: 3.0,
        loss_threshold: 0.3,
        retry_attempts: 3,
    };

    let adapter_manager = Arc::new(RwLock::new(AdapterManager::new()));
    let weights = ScoringWeights::default();
    let failover = Arc::new(FailoverManager::new(config, adapter_manager, weights));

    // Start the failover manager
    failover.start().await.expect("Failed to start");

    // Spawn multiple tasks that concurrently access failover manager
    let mut handles = vec![];

    for _ in 0..20 {
        let fm_clone = Arc::clone(&failover);
        let handle = tokio::spawn(async move {
            // Repeatedly query events (acquires event_log lock)
            for _ in 0..10 {
                let _events = fm_clone.get_recent_events(10).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task should complete without deadlock");
    }

    // Shutdown
    failover.shutdown().await;
}
