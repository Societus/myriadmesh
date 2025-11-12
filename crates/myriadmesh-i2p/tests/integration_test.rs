//! Integration tests for Phase 2 i2p implementation
//!
//! Tests the complete flow of:
//! - Dual identity management
//! - Capability token exchange
//! - Privacy protection layers
//! - Onion routing

use myriadmesh_crypto::identity::NodeIdentity;
use myriadmesh_i2p::{
    DualIdentity, I2pDestination, OnionConfig, OnionRouter, PaddingStrategy, PrivacyConfig,
    PrivacyLayer, RouteSelectionStrategy, TimingStrategy,
};
use myriadmesh_protocol::NodeId;

/// Helper to create test route nodes
fn create_test_route_node(
    id: u8,
    reliability: f64,
    latency: f64,
) -> myriadmesh_i2p::onion::RouteNode {
    let mut bytes = [0u8; 32];
    bytes[0] = id;
    myriadmesh_i2p::onion::RouteNode {
        node_id: NodeId::from_bytes(bytes),
        reliability,
        latency_ms: latency,
        available: true,
    }
}

#[test]
fn test_mode2_no_public_i2p_exposure() {
    // Initialize crypto
    myriadmesh_crypto::init().unwrap();

    // Alice creates dual identity
    let alice_dest = I2pDestination::new("alice.b32.i2p".to_string());
    let alice = DualIdentity::generate(alice_dest).unwrap();

    // Verify separate identities (critical for Mode 2)
    assert!(alice.verify_separate_identities());
    assert_ne!(alice.get_clearnet_node_id(), alice.get_i2p_node_id());

    // Verify i2p destination is not publicly linked to clearnet NodeID
    // In real implementation, PublicNodeInfo would NOT contain i2p destination
    // Only clearnet NodeID would be in DHT
    let clearnet_id = alice.get_clearnet_node_id();
    let i2p_id = alice.get_i2p_node_id();

    // These should be completely different
    assert_ne!(clearnet_id.as_bytes(), i2p_id.as_bytes());
}

#[test]
fn test_end_to_end_capability_token_exchange() {
    myriadmesh_crypto::init().unwrap();

    // Alice and Bob create dual identities
    let alice_dest = I2pDestination::new("alice.b32.i2p".to_string());
    let alice = DualIdentity::generate(alice_dest.clone()).unwrap();

    let bob_dest = I2pDestination::new("bob.b32.i2p".to_string());
    let mut bob = DualIdentity::generate(bob_dest.clone()).unwrap();

    // Alice grants Bob access to her i2p destination
    let token = alice
        .grant_i2p_access(bob.get_clearnet_node_id(), 30)
        .unwrap();

    // Verify token properties
    assert_eq!(token.for_node, bob.get_clearnet_node_id());
    assert_eq!(token.i2p_destination, alice_dest);
    assert_eq!(token.i2p_node_id, alice.get_i2p_node_id());
    assert_eq!(token.issuer_node_id, alice.get_clearnet_node_id());

    // Verify token signature
    let alice_pubkey = alice.get_clearnet_public_key().unwrap();
    assert!(token.verify(alice_pubkey).unwrap());

    // Bob stores the token
    bob.store_capability_token(token).unwrap();
    assert_eq!(bob.token_count(), 1);

    // Bob can now retrieve Alice's i2p info
    let alice_token = bob.get_capability_token(&alice.get_clearnet_node_id());
    assert!(alice_token.is_some());

    let alice_token = alice_token.unwrap();
    assert_eq!(alice_token.i2p_destination, alice_dest);
    assert_eq!(alice_token.i2p_node_id, alice.get_i2p_node_id());

    // Importantly, Bob's clearnet NodeID is NOT linked to Alice's i2p destination
    // in any public way. The token is stored locally only.
}

#[test]
fn test_privacy_layer_message_protection() {
    // Create privacy layer with all protections enabled
    let config = PrivacyConfig {
        padding_strategy: PaddingStrategy::FixedBuckets,
        min_message_size: 512,
        max_padding_size: 1024,
        timing_strategy: TimingStrategy::RandomDelay,
        base_delay_ms: 50,
        max_delay_ms: 200,
        enable_cover_traffic: true,
        cover_traffic_rate: 10,
    };

    let layer = PrivacyLayer::new(config);

    // Test message padding
    let original_message = b"Hello, this is a secret message";
    let padded = layer.pad_message(original_message);

    // Should be padded to bucket size
    assert!(padded.len() >= 512);
    let buckets = [512, 1024, 2048, 4096];
    assert!(buckets.contains(&padded.len()));

    // Original data should be at start
    assert_eq!(&padded[..original_message.len()], original_message);

    // Test timing obfuscation
    let delay = layer.calculate_delay();
    assert!(delay.as_millis() >= 50);
    assert!(delay.as_millis() <= 200);

    // Test cover traffic generation
    let cover_msg = layer.generate_cover_message();
    // With FixedBuckets strategy, should be one of the bucket sizes
    let valid_sizes = [512, 1024, 2048, 4096];
    assert!(valid_sizes.contains(&cover_msg.len()));
}

#[test]
fn test_onion_routing_multi_hop() {
    myriadmesh_crypto::init().unwrap();

    // Create local node
    let local_identity = NodeIdentity::generate().unwrap();
    let local_node_id = NodeId::from_bytes(*local_identity.node_id.as_bytes());

    // Create destination
    let mut dest_bytes = [0u8; 32];
    dest_bytes[0] = 255;
    let destination = NodeId::from_bytes(dest_bytes);

    // Create available relay nodes
    let relay_nodes = vec![
        create_test_route_node(1, 0.95, 50.0),
        create_test_route_node(2, 0.90, 100.0),
        create_test_route_node(3, 0.85, 150.0),
        create_test_route_node(4, 0.80, 200.0),
        create_test_route_node(5, 0.75, 250.0),
        create_test_route_node(6, 0.70, 300.0),
    ];

    // Create onion router
    let config = OnionConfig {
        num_hops: 3,
        selection_strategy: RouteSelectionStrategy::Balanced,
        max_route_lifetime: 3600,
        randomize_routes: true,
    };

    let mut router = OnionRouter::new(local_node_id, config);

    // Select route
    let route = router.select_route(destination, &relay_nodes).unwrap();

    // Verify route properties
    assert_eq!(route.source, local_node_id);
    assert_eq!(route.destination, destination);
    assert_eq!(route.hops.len(), 3);
    assert_eq!(route.total_hops(), 5); // source + 3 hops + dest

    // Verify hops don't include source or destination
    for hop in &route.hops {
        assert_ne!(*hop, local_node_id);
        assert_ne!(*hop, destination);
    }

    // Build onion layers
    let test_payload = b"secret message";
    let layers = router.build_onion_layers(&route, test_payload);

    // Should have layer for each hop
    assert_eq!(layers.len(), route.total_hops());

    // Verify layer structure
    let full_path = route.full_path();
    for (i, layer) in layers.iter().enumerate() {
        assert_eq!(layer.node_id, full_path[i]);
    }
}

#[test]
fn test_complete_i2p_communication_flow() {
    myriadmesh_crypto::init().unwrap();

    // Step 1: Alice and Bob create dual identities
    let alice_dest = I2pDestination::new("alice.b32.i2p".to_string());
    let alice = DualIdentity::generate(alice_dest.clone()).unwrap();

    let bob_dest = I2pDestination::new("bob.b32.i2p".to_string());
    let mut bob = DualIdentity::generate(bob_dest).unwrap();

    // Step 2: Alice grants Bob access via capability token
    let token = alice
        .grant_i2p_access(bob.get_clearnet_node_id(), 30)
        .unwrap();

    // Step 3: Bob stores token (transmitted via encrypted channel in real impl)
    bob.store_capability_token(token).unwrap();

    // Step 4: Bob prepares to send message to Alice over i2p
    // Get Alice's i2p info from stored token
    let alice_token = bob
        .get_capability_token(&alice.get_clearnet_node_id())
        .unwrap();

    assert_eq!(alice_token.i2p_destination, alice_dest);

    // Step 5: Apply privacy protection to message
    let privacy_config = PrivacyConfig {
        padding_strategy: PaddingStrategy::FixedBuckets,
        timing_strategy: TimingStrategy::RandomDelay,
        enable_cover_traffic: false, // Disabled for test
        ..Default::default()
    };

    let privacy_layer = PrivacyLayer::new(privacy_config);
    let message = b"Hello Alice, this is Bob";
    let protected_message = privacy_layer.pad_message(message);

    // Message should be padded
    assert!(protected_message.len() > message.len());

    // Step 6: Setup onion route for transmission
    let bob_node_id = NodeId::from_bytes(*bob.get_clearnet_node_id().as_bytes());
    let alice_i2p_node_id = alice_token.i2p_node_id;

    // Create relay nodes
    let relay_nodes = vec![
        create_test_route_node(10, 0.95, 50.0),
        create_test_route_node(11, 0.90, 60.0),
        create_test_route_node(12, 0.85, 70.0),
        create_test_route_node(13, 0.80, 80.0),
    ];

    let onion_config = OnionConfig {
        num_hops: 3,
        selection_strategy: RouteSelectionStrategy::Balanced,
        ..Default::default()
    };

    let mut onion_router = OnionRouter::new(bob_node_id, onion_config);
    let route = onion_router
        .select_route(alice_i2p_node_id, &relay_nodes)
        .unwrap();

    // Build onion layers
    let layers = onion_router.build_onion_layers(&route, &protected_message);

    // Verify complete protection stack
    assert_eq!(layers.len(), route.total_hops());
    assert!(protected_message.len() >= 512); // Padded

    // At this point, the message is:
    // 1. Padded (privacy layer)
    // 2. Wrapped in onion layers (onion routing)
    // 3. Ready for transmission to Alice's i2p destination
    // 4. Alice's clearnet identity is NOT exposed in public DHT
    // 5. Only Bob knows Alice's i2p destination (via capability token)
}

#[test]
fn test_privacy_guarantees() {
    myriadmesh_crypto::init().unwrap();

    // Create multiple nodes with dual identities
    let nodes: Vec<_> = (0..5)
        .map(|i| {
            let dest = I2pDestination::new(format!("node{}.b32.i2p", i));
            DualIdentity::generate(dest).unwrap()
        })
        .collect();

    // Verify privacy properties for each node
    for node in &nodes {
        // 1. Clearnet and i2p NodeIDs are different
        assert!(node.verify_separate_identities());

        // 2. NodeIDs are not derivable from each other
        let clearnet_id = node.get_clearnet_node_id();
        let i2p_id = node.get_i2p_node_id();
        assert_ne!(clearnet_id, i2p_id);

        // 3. i2p destination is not publicly available
        // (would only be in capability tokens, not in DHT PublicNodeInfo)
    }

    // Verify no two nodes have the same clearnet or i2p NodeID
    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            assert_ne!(
                nodes[i].get_clearnet_node_id(),
                nodes[j].get_clearnet_node_id()
            );
            assert_ne!(nodes[i].get_i2p_node_id(), nodes[j].get_i2p_node_id());
        }
    }
}

#[test]
fn test_route_selection_strategies() {
    myriadmesh_crypto::init().unwrap();

    let local_identity = NodeIdentity::generate().unwrap();
    let local_node_id = NodeId::from_bytes(*local_identity.node_id.as_bytes());

    let mut dest_bytes = [0u8; 32];
    dest_bytes[0] = 255;
    let destination = NodeId::from_bytes(dest_bytes);

    // Create nodes with varying reliability and latency
    let relay_nodes = vec![
        create_test_route_node(1, 0.99, 200.0), // High reliability, high latency
        create_test_route_node(2, 0.50, 10.0),  // Low reliability, low latency
        create_test_route_node(3, 0.75, 100.0), // Medium both
        create_test_route_node(4, 0.90, 50.0),  // Good reliability, good latency
        create_test_route_node(5, 0.85, 75.0),  // Balanced
    ];

    // Test Random strategy
    let config_random = OnionConfig {
        num_hops: 3,
        selection_strategy: RouteSelectionStrategy::Random,
        ..Default::default()
    };

    let mut router_random = OnionRouter::new(local_node_id, config_random);
    let route_random = router_random
        .select_route(destination, &relay_nodes)
        .unwrap();
    assert_eq!(route_random.hops.len(), 3);

    // Test HighReliability strategy
    let config_reliability = OnionConfig {
        num_hops: 3,
        selection_strategy: RouteSelectionStrategy::HighReliability,
        ..Default::default()
    };

    let mut router_reliability = OnionRouter::new(local_node_id, config_reliability);
    let route_reliability = router_reliability
        .select_route(destination, &relay_nodes)
        .unwrap();
    assert_eq!(route_reliability.hops.len(), 3);

    // Test LowLatency strategy
    let config_latency = OnionConfig {
        num_hops: 3,
        selection_strategy: RouteSelectionStrategy::LowLatency,
        ..Default::default()
    };

    let mut router_latency = OnionRouter::new(local_node_id, config_latency);
    let route_latency = router_latency
        .select_route(destination, &relay_nodes)
        .unwrap();
    assert_eq!(route_latency.hops.len(), 3);

    // Test Balanced strategy
    let config_balanced = OnionConfig {
        num_hops: 3,
        selection_strategy: RouteSelectionStrategy::Balanced,
        ..Default::default()
    };

    let mut router_balanced = OnionRouter::new(local_node_id, config_balanced);
    let route_balanced = router_balanced
        .select_route(destination, &relay_nodes)
        .unwrap();
    assert_eq!(route_balanced.hops.len(), 3);
}

#[test]
fn test_token_expiration_and_cleanup() {
    myriadmesh_crypto::init().unwrap();

    let alice_dest = I2pDestination::new("alice.b32.i2p".to_string());
    let alice = DualIdentity::generate(alice_dest).unwrap();

    let bob_dest = I2pDestination::new("bob.b32.i2p".to_string());
    let mut bob = DualIdentity::generate(bob_dest).unwrap();

    // Alice grants Bob access with very short validity
    let token = alice
        .grant_i2p_access(bob.get_clearnet_node_id(), 0) // 0 days = immediate expiration
        .unwrap();

    // Token should be expired (or will be very soon)
    std::thread::sleep(std::time::Duration::from_millis(1100)); // Wait > 1 second

    // Verify token is expired
    assert!(token.is_expired());

    // Bob shouldn't be able to store expired token
    let result = bob.store_capability_token(token);
    assert!(result.is_err());
}
