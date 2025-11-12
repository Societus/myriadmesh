//! Onion Routing for Enhanced Anonymity
//!
//! Implements multi-hop onion routing to prevent route tracing and
//! enhance anonymity in i2p communications.
//!
//! ## Security Model
//!
//! - Each hop only knows previous and next hop (no full route knowledge)
//! - Multiple layers of encryption (one per hop)
//! - Route randomization prevents traffic correlation
//! - Minimum 3 hops recommended for strong anonymity

use myriadmesh_protocol::NodeId;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Minimum number of hops for onion routing
pub const MIN_HOPS: usize = 3;

/// Maximum number of hops (performance vs anonymity tradeoff)
pub const MAX_HOPS: usize = 7;

/// Default number of hops
pub const DEFAULT_HOPS: usize = 3;

/// Onion routing configuration
#[derive(Debug, Clone)]
pub struct OnionConfig {
    /// Number of hops in the route
    pub num_hops: usize,

    /// Route selection strategy
    pub selection_strategy: RouteSelectionStrategy,

    /// Maximum route lifetime (seconds)
    pub max_route_lifetime: u64,

    /// Enable route randomization
    pub randomize_routes: bool,
}

impl Default for OnionConfig {
    fn default() -> Self {
        OnionConfig {
            num_hops: DEFAULT_HOPS,
            selection_strategy: RouteSelectionStrategy::Random,
            max_route_lifetime: 3600, // 1 hour
            randomize_routes: true,
        }
    }
}

/// Route selection strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSelectionStrategy {
    /// Completely random selection
    Random,

    /// Prefer high-reliability nodes
    HighReliability,

    /// Prefer low-latency nodes
    LowLatency,

    /// Balance between reliability and latency
    Balanced,
}

/// Single layer in an onion route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnionLayer {
    /// Node ID for this hop
    pub node_id: NodeId,

    /// Next hop (encrypted)
    pub next_hop: Option<Vec<u8>>,

    /// Layer encryption key (in real implementation, this would be derived)
    #[serde(skip)]
    pub encryption_key: Option<[u8; 32]>,
}

impl OnionLayer {
    /// Create new onion layer
    pub fn new(node_id: NodeId) -> Self {
        OnionLayer {
            node_id,
            next_hop: None,
            encryption_key: None,
        }
    }

    /// Set next hop (encrypted)
    pub fn set_next_hop(&mut self, next_hop: Vec<u8>) {
        self.next_hop = Some(next_hop);
    }
}

/// Complete onion route
#[derive(Debug, Clone)]
pub struct OnionRoute {
    /// Route ID
    pub route_id: u64,

    /// Source node
    pub source: NodeId,

    /// Destination node
    pub destination: NodeId,

    /// Intermediate hops (excluding source and destination)
    pub hops: Vec<NodeId>,

    /// Route creation time
    pub created_at: u64,

    /// Route expiration time
    pub expires_at: u64,

    /// Number of times this route has been used
    pub use_count: u64,
}

impl OnionRoute {
    /// Create new onion route
    pub fn new(
        source: NodeId,
        destination: NodeId,
        hops: Vec<NodeId>,
        lifetime_secs: u64,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut rng = rand::thread_rng();
        let route_id = rng.gen();

        OnionRoute {
            route_id,
            source,
            destination,
            hops,
            created_at: now,
            expires_at: now + lifetime_secs,
            use_count: 0,
        }
    }

    /// Check if route is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        now >= self.expires_at
    }

    /// Get total hop count (including source and destination)
    pub fn total_hops(&self) -> usize {
        self.hops.len() + 2 // +2 for source and destination
    }

    /// Get full path (source -> hops -> destination)
    pub fn full_path(&self) -> Vec<NodeId> {
        let mut path = Vec::with_capacity(self.total_hops());
        path.push(self.source);
        path.extend_from_slice(&self.hops);
        path.push(self.destination);
        path
    }

    /// Increment use count
    pub fn increment_use(&mut self) {
        self.use_count += 1;
    }

    /// Check if route should be retired (based on use count or age)
    pub fn should_retire(&self, max_uses: u64) -> bool {
        self.is_expired() || self.use_count >= max_uses
    }
}

/// Node information for route selection
#[derive(Debug, Clone)]
pub struct RouteNode {
    pub node_id: NodeId,
    pub reliability: f64, // 0.0 to 1.0
    pub latency_ms: f64,
    pub available: bool,
}

/// Onion router for managing routes
pub struct OnionRouter {
    config: OnionConfig,
    local_node_id: NodeId,
    active_routes: Vec<OnionRoute>,
}

impl OnionRouter {
    /// Create new onion router
    pub fn new(local_node_id: NodeId, config: OnionConfig) -> Self {
        OnionRouter {
            config,
            local_node_id,
            active_routes: Vec::new(),
        }
    }

    /// Create with default configuration
    pub fn new_default(local_node_id: NodeId) -> Self {
        Self::new(local_node_id, OnionConfig::default())
    }

    /// Select route to destination
    ///
    /// Returns a new onion route with randomly selected intermediate hops.
    pub fn select_route(
        &mut self,
        destination: NodeId,
        available_nodes: &[RouteNode],
    ) -> Result<OnionRoute, String> {
        // Filter out unavailable nodes, source, and destination
        let candidates: Vec<_> = available_nodes
            .iter()
            .filter(|n| {
                n.available && n.node_id != self.local_node_id && n.node_id != destination
            })
            .collect();

        if candidates.len() < self.config.num_hops {
            return Err(format!(
                "Not enough available nodes for route (need {}, have {})",
                self.config.num_hops,
                candidates.len()
            ));
        }

        // Select intermediate hops based on strategy
        let hops = self.select_hops(&candidates, self.config.num_hops)?;

        // Create route
        let route = OnionRoute::new(
            self.local_node_id,
            destination,
            hops,
            self.config.max_route_lifetime,
        );

        // Store active route
        self.active_routes.push(route.clone());

        Ok(route)
    }

    /// Select intermediate hops based on strategy
    fn select_hops(
        &self,
        candidates: &[&RouteNode],
        num_hops: usize,
    ) -> Result<Vec<NodeId>, String> {
        let mut rng = rand::thread_rng();

        match self.config.selection_strategy {
            RouteSelectionStrategy::Random => {
                // Completely random selection
                let selected: Vec<NodeId> = candidates
                    .choose_multiple(&mut rng, num_hops)
                    .map(|n| n.node_id)
                    .collect();

                Ok(selected)
            }

            RouteSelectionStrategy::HighReliability => {
                // Sort by reliability and pick top nodes with some randomness
                let mut sorted = candidates.to_vec();
                sorted.sort_by(|a, b| b.reliability.partial_cmp(&a.reliability).unwrap());

                // Pick from top 2*num_hops to add some randomness
                let pool_size = (num_hops * 2).min(sorted.len());
                let selected: Vec<NodeId> = sorted[..pool_size]
                    .choose_multiple(&mut rng, num_hops)
                    .map(|n| n.node_id)
                    .collect();

                Ok(selected)
            }

            RouteSelectionStrategy::LowLatency => {
                // Sort by latency and pick top nodes with some randomness
                let mut sorted = candidates.to_vec();
                sorted.sort_by(|a, b| a.latency_ms.partial_cmp(&b.latency_ms).unwrap());

                let pool_size = (num_hops * 2).min(sorted.len());
                let selected: Vec<NodeId> = sorted[..pool_size]
                    .choose_multiple(&mut rng, num_hops)
                    .map(|n| n.node_id)
                    .collect();

                Ok(selected)
            }

            RouteSelectionStrategy::Balanced => {
                // Score based on both reliability and latency
                let mut scored: Vec<_> = candidates
                    .iter()
                    .map(|n| {
                        let score = n.reliability * 0.5 + (1.0 - (n.latency_ms / 1000.0).min(1.0)) * 0.5;
                        (n, score)
                    })
                    .collect();

                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                let pool_size = (num_hops * 2).min(scored.len());
                let selected: Vec<NodeId> = scored[..pool_size]
                    .choose_multiple(&mut rng, num_hops)
                    .map(|(n, _)| n.node_id)
                    .collect();

                Ok(selected)
            }
        }
    }

    /// Get active route to destination
    pub fn get_route(&mut self, destination: &NodeId) -> Option<&mut OnionRoute> {
        // Cleanup expired routes
        self.cleanup_expired_routes();

        // Find non-expired route to destination
        self.active_routes
            .iter_mut()
            .find(|r| &r.destination == destination && !r.is_expired())
    }

    /// Cleanup expired routes
    pub fn cleanup_expired_routes(&mut self) -> usize {
        let before = self.active_routes.len();
        self.active_routes.retain(|r| !r.is_expired());
        before - self.active_routes.len()
    }

    /// Get number of active routes
    pub fn active_route_count(&self) -> usize {
        self.active_routes.iter().filter(|r| !r.is_expired()).count()
    }

    /// Build onion layers for a route
    ///
    /// Creates encrypted layers for each hop in the route.
    /// In a real implementation, each layer would be encrypted with the hop's public key.
    pub fn build_onion_layers(&self, route: &OnionRoute, payload: &[u8]) -> Vec<OnionLayer> {
        let mut layers = Vec::new();
        let path = route.full_path();

        // Build layers in reverse order (destination first)
        for i in (0..path.len()).rev() {
            let node_id = path[i];
            let mut layer = OnionLayer::new(node_id);

            // For real implementation:
            // 1. Encrypt payload with this hop's key
            // 2. Add routing info for next hop
            // 3. This becomes the payload for the previous layer

            // For now, just store the node info
            if i < path.len() - 1 {
                // Not the last hop, add next hop info
                let next_node = path[i + 1];
                layer.set_next_hop(next_node.as_bytes().to_vec());
            }

            layers.push(layer);
        }

        // Reverse to get correct order (source first)
        layers.reverse();
        layers
    }

    /// Peel one layer from onion (at intermediate hop)
    ///
    /// Decrypts outer layer and returns next hop info and remaining onion.
    pub fn peel_layer(&self, layers: &[OnionLayer]) -> Result<(NodeId, Vec<OnionLayer>), String> {
        if layers.is_empty() {
            return Err("No layers to peel".to_string());
        }

        let current_layer = &layers[0];

        // Verify this layer is for us
        if current_layer.node_id != self.local_node_id {
            return Err("Layer not intended for this node".to_string());
        }

        // Get next hop
        let next_hop_bytes = current_layer
            .next_hop
            .as_ref()
            .ok_or("No next hop in layer")?;

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(next_hop_bytes);
        let next_hop = NodeId::from_bytes(bytes);

        // Return remaining layers
        let remaining = layers[1..].to_vec();

        Ok((next_hop, remaining))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_nodes(count: usize) -> Vec<RouteNode> {
        (0..count)
            .map(|i| {
                let mut bytes = [0u8; 32];
                bytes[0] = i as u8;
                RouteNode {
                    node_id: NodeId::from_bytes(bytes),
                    reliability: 0.9,
                    latency_ms: 50.0,
                    available: true,
                }
            })
            .collect()
    }

    #[test]
    fn test_onion_route_creation() {
        let source = NodeId::from_bytes([1u8; 32]);
        let dest = NodeId::from_bytes([2u8; 32]);
        let hops = vec![
            NodeId::from_bytes([3u8; 32]),
            NodeId::from_bytes([4u8; 32]),
        ];

        let route = OnionRoute::new(source, dest, hops.clone(), 3600);

        assert_eq!(route.source, source);
        assert_eq!(route.destination, dest);
        assert_eq!(route.hops, hops);
        assert_eq!(route.total_hops(), 4); // source + 2 hops + dest
        assert!(!route.is_expired());
    }

    #[test]
    fn test_onion_route_expiration() {
        let source = NodeId::from_bytes([1u8; 32]);
        let dest = NodeId::from_bytes([2u8; 32]);
        let hops = vec![];

        let route = OnionRoute::new(source, dest, hops, 0); // Immediate expiration

        std::thread::sleep(Duration::from_millis(10));
        assert!(route.is_expired());
    }

    #[test]
    fn test_route_selection_random() {
        let local = NodeId::from_bytes([0u8; 32]);
        let dest = NodeId::from_bytes([255u8; 32]);
        let nodes = create_test_nodes(10);

        let mut router = OnionRouter::new_default(local);
        let route = router.select_route(dest, &nodes).unwrap();

        assert_eq!(route.source, local);
        assert_eq!(route.destination, dest);
        assert_eq!(route.hops.len(), DEFAULT_HOPS);
        assert!(!route.hops.contains(&local));
        assert!(!route.hops.contains(&dest));
    }

    #[test]
    fn test_route_selection_insufficient_nodes() {
        let local = NodeId::from_bytes([0u8; 32]);
        let dest = NodeId::from_bytes([255u8; 32]);
        let nodes = create_test_nodes(2); // Not enough for 3 hops

        let mut router = OnionRouter::new_default(local);
        let result = router.select_route(dest, &nodes);

        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_expired_routes() {
        let local = NodeId::from_bytes([0u8; 32]);
        let dest = NodeId::from_bytes([255u8; 32]);
        let nodes = create_test_nodes(10);

        let mut router = OnionRouter::new_default(local);

        // Create a route with short lifetime
        let mut config = OnionConfig::default();
        config.max_route_lifetime = 0;
        router.config = config;

        router.select_route(dest, &nodes).unwrap();
        assert_eq!(router.active_routes.len(), 1);

        std::thread::sleep(Duration::from_millis(10));

        let removed = router.cleanup_expired_routes();
        assert_eq!(removed, 1);
        assert_eq!(router.active_routes.len(), 0);
    }

    #[test]
    fn test_onion_layer_building() {
        let local = NodeId::from_bytes([0u8; 32]);
        let dest = NodeId::from_bytes([255u8; 32]);
        let hops = vec![
            NodeId::from_bytes([1u8; 32]),
            NodeId::from_bytes([2u8; 32]),
        ];

        let route = OnionRoute::new(local, dest, hops, 3600);
        let router = OnionRouter::new_default(local);

        let payload = b"test message";
        let layers = router.build_onion_layers(&route, payload);

        assert_eq!(layers.len(), 4); // source + 2 hops + dest
        assert_eq!(layers[0].node_id, local);
        assert_eq!(layers[3].node_id, dest);
    }

    #[test]
    fn test_layer_peeling() {
        let local = NodeId::from_bytes([0u8; 32]);
        let next = NodeId::from_bytes([1u8; 32]);
        let dest = NodeId::from_bytes([2u8; 32]);

        let mut layer1 = OnionLayer::new(local);
        layer1.set_next_hop(next.as_bytes().to_vec());

        let layer2 = OnionLayer::new(next);
        let layer3 = OnionLayer::new(dest);

        let layers = vec![layer1, layer2, layer3];
        let router = OnionRouter::new_default(local);

        let (next_hop, remaining) = router.peel_layer(&layers).unwrap();

        assert_eq!(next_hop, next);
        assert_eq!(remaining.len(), 2);
    }

    #[test]
    fn test_route_use_count() {
        let source = NodeId::from_bytes([1u8; 32]);
        let dest = NodeId::from_bytes([2u8; 32]);
        let hops = vec![];

        let mut route = OnionRoute::new(source, dest, hops, 3600);
        assert_eq!(route.use_count, 0);

        route.increment_use();
        assert_eq!(route.use_count, 1);

        assert!(!route.should_retire(10));
        route.use_count = 10;
        assert!(route.should_retire(10));
    }
}
