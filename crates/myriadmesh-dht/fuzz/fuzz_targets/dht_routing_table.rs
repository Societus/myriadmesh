#![no_main]

use libfuzzer_sys::fuzz_target;
use myriadmesh_dht::routing_table::RoutingTable;
use myriadmesh_protocol::types::NodeId;

fuzz_target!(|data: &[u8]| {
    // SECURITY P1.4.1: Fuzz DHT routing table operations
    // This test ensures DHT operations don't crash on malformed input

    // Create a dummy node ID for the local node
    let local_node_id = [1u8; 64];

    // Create routing table
    let mut routing_table = match RoutingTable::new(local_node_id) {
        Ok(rt) => rt,
        Err(_) => return, // Can't proceed without routing table
    };

    // Try to add arbitrary node IDs to the routing table
    if data.len() >= 64 {
        let mut node_id = [0u8; 64];
        node_id.copy_from_slice(&data[0..64]);

        // Try to add node - should not panic on any input
        let _ = routing_table.add_node(node_id);
    }

    // Test with size variations
    if data.len() >= 128 {
        let mut node_id = [0u8; 64];
        node_id.copy_from_slice(&data[64..128]);
        let _ = routing_table.add_node(node_id);
    }

    // Try lookups on arbitrary data
    if !data.is_empty() && data.len() <= 64 {
        let mut search_id = [0u8; 64];
        search_id[..data.len()].copy_from_slice(data);
        let _ = routing_table.find_closest_nodes(search_id, 20);
    }
});
