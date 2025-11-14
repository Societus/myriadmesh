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
//! - SECURITY C5: Timing obfuscation prevents correlation attacks

use myriadmesh_crypto::encryption::{decrypt, encrypt, EncryptedMessage};
use myriadmesh_crypto::keyexchange::{client_session_keys, KeyExchangeKeypair, X25519PublicKey};
use myriadmesh_protocol::{types::NODE_ID_SIZE, NodeId};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

/// Minimum number of hops for onion routing
pub const MIN_HOPS: usize = 3;

/// Maximum number of hops (performance vs anonymity tradeoff)
pub const MAX_HOPS: usize = 7;

/// Default number of hops
pub const DEFAULT_HOPS: usize = 3;

/// SECURITY C5: Minimum delay (ms) before forwarding at each hop
/// Prevents timing correlation by ensuring non-zero forwarding delay
pub const MIN_FORWARD_DELAY_MS: u64 = 10;

/// SECURITY C5: Maximum random jitter (ms) added to forwarding delay
/// Creates unpredictable timing patterns to prevent correlation attacks
pub const MAX_FORWARD_JITTER_MS: u64 = 200;

/// SECURITY C5: Target processing time (ms) for layer building
/// Normalizes timing regardless of hop count to prevent hop count leakage
pub const TARGET_BUILD_TIME_MS: u64 = 100;

/// SECURITY H10: Maximum route uses before requiring rotation
/// Prevents excessive route reuse that could compromise anonymity
pub const MAX_ROUTE_USES: u64 = 1000;

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

    /// Encrypted payload (contains next hop + inner layers)
    pub encrypted_payload: Vec<u8>,
}

impl OnionLayer {
    /// Create new onion layer
    pub fn new(node_id: NodeId, encrypted_payload: Vec<u8>) -> Self {
        OnionLayer {
            node_id,
            encrypted_payload,
        }
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

    /// Public keys for each hop (for encryption)
    /// Maps NodeId -> X25519 public key
    pub hop_public_keys: HashMap<NodeId, X25519PublicKey>,

    /// Route creation time
    pub created_at: u64,

    /// Route expiration time
    pub expires_at: u64,

    /// Number of times this route has been used
    pub use_count: u64,
}

impl OnionRoute {
    /// Create new onion route
    pub fn new(source: NodeId, destination: NodeId, hops: Vec<NodeId>, lifetime_secs: u64) -> Self {
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
            hop_public_keys: HashMap::new(),
            created_at: now,
            expires_at: now + lifetime_secs,
            use_count: 0,
        }
    }

    /// Set public key for a hop
    pub fn set_hop_public_key(&mut self, node_id: NodeId, public_key: X25519PublicKey) {
        self.hop_public_keys.insert(node_id, public_key);
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
    /// X25519 public key for this node (for onion encryption)
    pub public_key: X25519PublicKey,
}

/// Onion router for managing routes
pub struct OnionRouter {
    config: OnionConfig,
    local_node_id: NodeId,
    /// Local keypair for decrypting layers intended for this node
    local_keypair: KeyExchangeKeypair,
    active_routes: Vec<OnionRoute>,
}

impl OnionRouter {
    /// Create new onion router
    pub fn new(
        local_node_id: NodeId,
        local_keypair: KeyExchangeKeypair,
        config: OnionConfig,
    ) -> Self {
        OnionRouter {
            config,
            local_node_id,
            local_keypair,
            active_routes: Vec::new(),
        }
    }

    /// Create with default configuration
    pub fn new_default(local_node_id: NodeId, local_keypair: KeyExchangeKeypair) -> Self {
        Self::new(local_node_id, local_keypair, OnionConfig::default())
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
            .filter(|n| n.available && n.node_id != self.local_node_id && n.node_id != destination)
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
        let mut route = OnionRoute::new(
            self.local_node_id,
            destination,
            hops.clone(),
            self.config.max_route_lifetime,
        );

        // Store public keys for selected hops
        for node in available_nodes {
            if hops.contains(&node.node_id) || node.node_id == destination {
                route.set_hop_public_key(node.node_id, node.public_key);
            }
        }

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
                        let score =
                            n.reliability * 0.5 + (1.0 - (n.latency_ms / 1000.0).min(1.0)) * 0.5;
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
        self.active_routes
            .iter()
            .filter(|r| !r.is_expired())
            .count()
    }

    /// Build onion layers with timing protection (async)
    ///
    /// SECURITY C5: Normalizes processing time regardless of hop count to prevent
    /// hop count leakage through timing analysis. This is the RECOMMENDED method
    /// for production use.
    ///
    /// SECURITY H10: Enforces route expiration - will fail if route is expired or
    /// has exceeded maximum use count.
    ///
    /// Creates encrypted layers for each hop in the route.
    /// Each layer is encrypted with the hop's public key using X25519 key exchange.
    pub async fn build_onion_layers_with_timing_protection(
        &self,
        route: &OnionRoute,
        payload: &[u8],
    ) -> Result<Vec<OnionLayer>, String> {
        use std::time::Instant;

        // SECURITY H10: Verify route has not expired
        if route.is_expired() {
            return Err("Route has expired".to_string());
        }

        // SECURITY H10: Verify route has not exceeded maximum uses
        if route.should_retire(MAX_ROUTE_USES) {
            return Err(format!(
                "Route should be retired (uses: {}, max: {})",
                route.use_count, MAX_ROUTE_USES
            ));
        }

        let start = Instant::now();

        // Build layers synchronously
        let layers = self.build_onion_layers_sync(route, payload)?;

        let elapsed = start.elapsed();

        // SECURITY C5: Normalize processing time to TARGET_BUILD_TIME_MS
        // This prevents timing analysis from revealing the number of hops
        let target = Duration::from_millis(TARGET_BUILD_TIME_MS);
        if elapsed < target {
            let remaining = target - elapsed;
            // Add some randomness to the padding delay (±20%)
            let mut rng = rand::thread_rng();
            let jitter_factor = rng.gen_range(0.8..=1.2);
            let delay = remaining.mul_f64(jitter_factor);
            sleep(delay).await;
        }

        Ok(layers)
    }

    /// Build onion layers (synchronous, no timing protection)
    ///
    /// WARNING: This method does NOT include timing protection and processing time
    /// is proportional to hop count, potentially leaking route information.
    /// For production use, prefer `build_onion_layers_with_timing_protection()`.
    ///
    /// Creates encrypted layers for each hop in the route.
    /// Each layer is encrypted with the hop's public key using X25519 key exchange.
    pub fn build_onion_layers_sync(
        &self,
        route: &OnionRoute,
        payload: &[u8],
    ) -> Result<Vec<OnionLayer>, String> {
        // SECURITY H10: Verify route has not expired
        if route.is_expired() {
            return Err("Route has expired".to_string());
        }

        // SECURITY H10: Verify route has not exceeded maximum uses
        if route.should_retire(MAX_ROUTE_USES) {
            return Err(format!(
                "Route should be retired (uses: {}, max: {})",
                route.use_count, MAX_ROUTE_USES
            ));
        }

        let path = route.full_path();

        // Start with the final payload
        let mut current_payload = payload.to_vec();
        let mut layers = Vec::new();

        // Build layers in reverse order (destination first, working back to source)
        for i in (0..path.len()).rev() {
            let node_id = path[i];

            // Get public key for this hop
            let hop_public_key = route
                .hop_public_keys
                .get(&node_id)
                .ok_or_else(|| format!("No public key for hop {}", node_id))?;

            // Generate ephemeral keypair for this layer
            let ephemeral_keypair = KeyExchangeKeypair::generate();

            // Derive shared secret using ECDH
            let session_keys = client_session_keys(&ephemeral_keypair, hop_public_key)
                .map_err(|e| format!("Key exchange failed: {}", e))?;

            // Add next hop info if not the last hop
            let layer_data = if i < path.len() - 1 {
                let next_hop = path[i + 1];
                // SECURITY C6: Serialize: next_hop (64 bytes) + payload
                let mut data = Vec::with_capacity(NODE_ID_SIZE + current_payload.len());
                data.extend_from_slice(next_hop.as_bytes());
                data.extend_from_slice(&current_payload);
                data
            } else {
                // Last hop, just the payload
                current_payload.clone()
            };

            // Encrypt the layer data
            let encrypted = encrypt(&session_keys.tx_key, &layer_data)
                .map_err(|e| format!("Encryption failed: {}", e))?;

            // Serialize encrypted message (nonce + ciphertext)
            let mut encrypted_bytes = Vec::new();
            encrypted_bytes.extend_from_slice(encrypted.nonce.as_bytes());
            encrypted_bytes.extend_from_slice(&encrypted.ciphertext);

            // Prepend ephemeral public key so recipient can derive shared secret
            let mut full_layer = Vec::new();
            full_layer.extend_from_slice(ephemeral_keypair.public_bytes());
            full_layer.extend_from_slice(&encrypted_bytes);

            // Create the layer
            let layer = OnionLayer::new(node_id, full_layer.clone());
            layers.push(layer);

            // This encrypted layer becomes the payload for the next iteration
            current_payload = full_layer;
        }

        // Reverse to get correct order (source first)
        layers.reverse();
        Ok(layers)
    }

    /// Peel one layer from onion with timing protection (async)
    ///
    /// SECURITY C5: Adds random delay before forwarding to prevent timing correlation.
    /// This is the RECOMMENDED method for production use to prevent de-anonymization.
    ///
    /// Decrypts outer layer and returns next hop info and remaining onion payload.
    /// Returns (next_hop, decrypted_payload) where decrypted_payload is the inner layers.
    pub async fn peel_layer_with_timing_protection(
        &self,
        layer: &OnionLayer,
    ) -> Result<(Option<NodeId>, Vec<u8>), String> {
        // SECURITY C5: Add random delay BEFORE processing to prevent timing attacks
        // This ensures that even if decryption timing varies, external observers
        // cannot correlate timing patterns to determine hop position or route structure
        let mut rng = rand::thread_rng();
        let delay = rng.gen_range(MIN_FORWARD_DELAY_MS..=MAX_FORWARD_JITTER_MS);
        sleep(Duration::from_millis(delay)).await;

        // Perform the actual layer peeling
        self.peel_layer_sync(layer)
    }

    /// Peel one layer from onion (synchronous, no timing protection)
    ///
    /// WARNING: This method does NOT include timing protection and should only be
    /// used for testing or non-privacy-critical operations. For production use,
    /// prefer `peel_layer_with_timing_protection()`.
    ///
    /// Decrypts outer layer and returns next hop info and remaining onion payload.
    /// Returns (next_hop, decrypted_payload) where decrypted_payload is the inner layers.
    pub fn peel_layer_sync(&self, layer: &OnionLayer) -> Result<(Option<NodeId>, Vec<u8>), String> {
        use myriadmesh_crypto::encryption::Nonce;
        use myriadmesh_crypto::keyexchange::server_session_keys;

        // Verify this layer is for us
        if layer.node_id != self.local_node_id {
            return Err("Layer not intended for this node".to_string());
        }

        let encrypted_payload = &layer.encrypted_payload;

        // Extract ephemeral public key (first 32 bytes)
        if encrypted_payload.len() < 32 {
            return Err("Encrypted payload too short".to_string());
        }

        let mut ephemeral_public_bytes = [0u8; 32];
        ephemeral_public_bytes.copy_from_slice(&encrypted_payload[0..32]);
        let ephemeral_public = X25519PublicKey::from_bytes(ephemeral_public_bytes);

        // Derive shared secret using our secret key
        let session_keys = server_session_keys(&self.local_keypair, &ephemeral_public)
            .map_err(|e| format!("Key exchange failed: {}", e))?;

        // Extract nonce (24 bytes after public key)
        if encrypted_payload.len() < 32 + 24 {
            return Err("Encrypted payload missing nonce".to_string());
        }

        let mut nonce_bytes = [0u8; 24];
        nonce_bytes.copy_from_slice(&encrypted_payload[32..56]);
        let nonce = Nonce::from_bytes(nonce_bytes);

        // Extract ciphertext (remainder)
        let ciphertext = encrypted_payload[56..].to_vec();

        // Decrypt the layer
        let encrypted_msg = EncryptedMessage { nonce, ciphertext };
        let decrypted = decrypt(&session_keys.rx_key, &encrypted_msg)
            .map_err(|e| format!("Decryption failed: {}", e))?;

        // Check if this layer contains next hop info (intermediate hop)
        // or if it's the final destination
        // SECURITY C6: NodeID is now 64 bytes for collision resistance
        if decrypted.len() >= NODE_ID_SIZE {
            // This is an intermediate hop, extract next hop
            let mut next_hop_bytes = [0u8; NODE_ID_SIZE];
            next_hop_bytes.copy_from_slice(&decrypted[0..NODE_ID_SIZE]);
            let next_hop = NodeId::from_bytes(next_hop_bytes);

            // Remaining payload is the inner layers
            let inner_payload = decrypted[NODE_ID_SIZE..].to_vec();

            Ok((Some(next_hop), inner_payload))
        } else {
            // This is the final destination, no next hop
            Ok((None, decrypted))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_nodes(count: usize) -> Vec<RouteNode> {
        (0..count)
            .map(|i| {
                let mut bytes = [0u8; NODE_ID_SIZE];
                bytes[0] = i as u8;
                let keypair = KeyExchangeKeypair::generate();
                RouteNode {
                    node_id: NodeId::from_bytes(bytes),
                    reliability: 0.9,
                    latency_ms: 50.0,
                    available: true,
                    public_key: X25519PublicKey::from(&keypair.public_key),
                }
            })
            .collect()
    }

    #[test]
    fn test_onion_route_creation() {
        let source = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let hops = vec![
            NodeId::from_bytes([3u8; NODE_ID_SIZE]),
            NodeId::from_bytes([4u8; NODE_ID_SIZE]),
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
        let source = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let hops = vec![];

        let route = OnionRoute::new(source, dest, hops, 0); // Immediate expiration

        std::thread::sleep(Duration::from_millis(10));
        assert!(route.is_expired());
    }

    #[test]
    fn test_route_selection_random() {
        myriadmesh_crypto::init().unwrap();
        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([255u8; NODE_ID_SIZE]);
        let nodes = create_test_nodes(10);

        let keypair = KeyExchangeKeypair::generate();
        let mut router = OnionRouter::new_default(local, keypair);
        let route = router.select_route(dest, &nodes).unwrap();

        assert_eq!(route.source, local);
        assert_eq!(route.destination, dest);
        assert_eq!(route.hops.len(), DEFAULT_HOPS);
        assert!(!route.hops.contains(&local));
        assert!(!route.hops.contains(&dest));
    }

    #[test]
    fn test_route_selection_insufficient_nodes() {
        myriadmesh_crypto::init().unwrap();
        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([255u8; NODE_ID_SIZE]);
        let nodes = create_test_nodes(2); // Not enough for 3 hops

        let keypair = KeyExchangeKeypair::generate();
        let mut router = OnionRouter::new_default(local, keypair);
        let result = router.select_route(dest, &nodes);

        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_expired_routes() {
        myriadmesh_crypto::init().unwrap();
        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([255u8; NODE_ID_SIZE]);
        let nodes = create_test_nodes(10);

        let keypair = KeyExchangeKeypair::generate();
        let mut router = OnionRouter::new_default(local, keypair);

        // Create a route with short lifetime
        let config = OnionConfig {
            max_route_lifetime: 0,
            ..Default::default()
        };
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
        myriadmesh_crypto::init().unwrap();
        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([255u8; NODE_ID_SIZE]);
        let hops = vec![
            NodeId::from_bytes([1u8; NODE_ID_SIZE]),
            NodeId::from_bytes([2u8; NODE_ID_SIZE]),
        ];

        let mut route = OnionRoute::new(local, dest, hops, 3600);

        // Add public keys for all nodes in the path
        let local_kp = KeyExchangeKeypair::generate();
        let hop1_kp = KeyExchangeKeypair::generate();
        let hop2_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        route.set_hop_public_key(local, X25519PublicKey::from(&local_kp.public_key));
        route.set_hop_public_key(
            NodeId::from_bytes([1u8; NODE_ID_SIZE]),
            X25519PublicKey::from(&hop1_kp.public_key),
        );
        route.set_hop_public_key(
            NodeId::from_bytes([2u8; NODE_ID_SIZE]),
            X25519PublicKey::from(&hop2_kp.public_key),
        );
        route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));

        let router = OnionRouter::new_default(local, local_kp);

        let payload = b"test message";
        let layers = router.build_onion_layers_sync(&route, payload).unwrap();

        assert_eq!(layers.len(), 4); // source + 2 hops + dest
        assert_eq!(layers[0].node_id, local);
        assert_eq!(layers[3].node_id, dest);
    }

    #[test]
    fn test_layer_peeling() {
        myriadmesh_crypto::init().unwrap();

        // Create nodes with keypairs
        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let next = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);

        let local_kp = KeyExchangeKeypair::generate();
        let next_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        // Create route
        let mut route = OnionRoute::new(local, dest, vec![next], 3600);
        route.set_hop_public_key(local, X25519PublicKey::from(&local_kp.public_key));
        route.set_hop_public_key(next, X25519PublicKey::from(&next_kp.public_key));
        route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));

        // Build onion layers
        let payload = b"test message";
        let router = OnionRouter::new_default(local, local_kp);
        let layers = router.build_onion_layers_sync(&route, payload).unwrap();

        // First hop peels their layer
        let next_router = OnionRouter::new_default(next, next_kp);
        let (next_hop, inner_payload) = next_router.peel_layer_sync(&layers[1]).unwrap();

        assert_eq!(next_hop, Some(dest));
        assert!(!inner_payload.is_empty());
    }

    #[test]
    fn test_route_use_count() {
        let source = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let hops = vec![];

        let mut route = OnionRoute::new(source, dest, hops, 3600);
        assert_eq!(route.use_count, 0);

        route.increment_use();
        assert_eq!(route.use_count, 1);

        assert!(!route.should_retire(10));
        route.use_count = 10;
        assert!(route.should_retire(10));
    }

    #[test]
    fn test_end_to_end_onion_encryption() {
        myriadmesh_crypto::init().unwrap();

        // Create a 3-hop route: source -> hop1 -> hop2 -> dest
        let source = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let hop1 = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let hop2 = NodeId::from_bytes([2u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([3u8; NODE_ID_SIZE]);

        let source_kp = KeyExchangeKeypair::generate();
        let hop1_kp = KeyExchangeKeypair::generate();
        let hop2_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        // Create route
        let mut route = OnionRoute::new(source, dest, vec![hop1, hop2], 3600);
        route.set_hop_public_key(source, X25519PublicKey::from(&source_kp.public_key));
        route.set_hop_public_key(hop1, X25519PublicKey::from(&hop1_kp.public_key));
        route.set_hop_public_key(hop2, X25519PublicKey::from(&hop2_kp.public_key));
        route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));

        // Build onion layers at source
        let original_payload = b"Secret message for destination";
        let source_router = OnionRouter::new_default(source, source_kp);
        let layers = source_router
            .build_onion_layers_sync(&route, original_payload)
            .unwrap();

        assert_eq!(layers.len(), 4); // source + hop1 + hop2 + dest

        // Hop1 peels their layer
        let hop1_router = OnionRouter::new_default(hop1, hop1_kp);
        let (next1, payload1) = hop1_router.peel_layer_sync(&layers[1]).unwrap();
        assert_eq!(next1, Some(hop2));

        // Parse payload1 as next layer
        let layer2 = OnionLayer::new(hop2, payload1);

        // Hop2 peels their layer
        let hop2_router = OnionRouter::new_default(hop2, hop2_kp);
        let (next2, payload2) = hop2_router.peel_layer_sync(&layer2).unwrap();
        assert_eq!(next2, Some(dest));

        // Parse payload2 as final layer
        let layer3 = OnionLayer::new(dest, payload2);

        // Destination decrypts final layer
        let dest_router = OnionRouter::new_default(dest, dest_kp);
        let (next_final, final_payload) = dest_router.peel_layer_sync(&layer3).unwrap();
        assert_eq!(next_final, None); // No next hop at destination
        assert_eq!(final_payload, original_payload);
    }

    #[tokio::test]
    async fn test_peel_layer_with_timing_protection() {
        // SECURITY C5: Test that timing protection adds delays
        myriadmesh_crypto::init().unwrap();

        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let next = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);

        let local_kp = KeyExchangeKeypair::generate();
        let next_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        let mut route = OnionRoute::new(local, dest, vec![next], 3600);
        route.set_hop_public_key(local, X25519PublicKey::from(&local_kp.public_key));
        route.set_hop_public_key(next, X25519PublicKey::from(&next_kp.public_key));
        route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));

        let payload = b"test message";
        let router = OnionRouter::new_default(local, local_kp);
        let layers = router.build_onion_layers_sync(&route, payload).unwrap();

        // Measure time with timing protection
        let next_router = OnionRouter::new_default(next, next_kp);
        let start = std::time::Instant::now();
        let (next_hop, inner_payload) = next_router
            .peel_layer_with_timing_protection(&layers[1])
            .await
            .unwrap();
        let elapsed = start.elapsed();

        // Should have added at least MIN_FORWARD_DELAY_MS
        assert!(
            elapsed >= Duration::from_millis(MIN_FORWARD_DELAY_MS),
            "Expected delay >= {}ms, got {:?}",
            MIN_FORWARD_DELAY_MS,
            elapsed
        );

        // Should not exceed MAX_FORWARD_JITTER_MS + processing time (generous allowance for CI)
        // We allow 500ms headroom for CI environments under heavy load
        assert!(
            elapsed <= Duration::from_millis(MAX_FORWARD_JITTER_MS + 500),
            "Expected delay <= {}ms, got {:?}",
            MAX_FORWARD_JITTER_MS + 500,
            elapsed
        );

        // Verify correctness
        assert_eq!(next_hop, Some(dest));
        assert!(!inner_payload.is_empty());
    }

    #[tokio::test]
    async fn test_build_layers_timing_normalization() {
        // SECURITY C5: Test that build time is normalized regardless of hop count
        myriadmesh_crypto::init().unwrap();

        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([255u8; NODE_ID_SIZE]);

        let local_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        // Test with different hop counts
        let hop_counts = [0, 1, 2, 3, 5];
        let mut build_times = Vec::new();

        for &hop_count in &hop_counts {
            // Create route with specified hop count
            let hops: Vec<NodeId> = (1..=hop_count)
                .map(|i| {
                    let mut bytes = [0u8; NODE_ID_SIZE];
                    bytes[0] = i as u8;
                    NodeId::from_bytes(bytes)
                })
                .collect();

            let mut route = OnionRoute::new(local, dest, hops.clone(), 3600);
            route.set_hop_public_key(local, X25519PublicKey::from(&local_kp.public_key));

            // Add public keys for all hops
            for &hop_id in hops.iter() {
                let kp = KeyExchangeKeypair::generate();
                route.set_hop_public_key(hop_id, X25519PublicKey::from(&kp.public_key));
            }
            route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));

            // Measure build time with timing protection
            let router = OnionRouter::new_default(local, KeyExchangeKeypair::generate());
            let start = std::time::Instant::now();
            let _layers = router
                .build_onion_layers_with_timing_protection(&route, b"test")
                .await
                .unwrap();
            let elapsed = start.elapsed();

            build_times.push(elapsed);
        }

        // SECURITY C5: All build times should be close to TARGET_BUILD_TIME_MS
        // Allow for jitter (±20%) plus processing overhead
        let min_expected = Duration::from_millis((TARGET_BUILD_TIME_MS as f64 * 0.7) as u64);
        let max_expected = Duration::from_millis((TARGET_BUILD_TIME_MS as f64 * 1.3) as u64);

        for (i, &time) in build_times.iter().enumerate() {
            assert!(
                time >= min_expected && time <= max_expected,
                "Build time for {} hops ({:?}) outside normalized range ({:?} - {:?})",
                hop_counts[i],
                time,
                min_expected,
                max_expected
            );
        }

        // Verify timing variance is small (normalized successfully)
        let times_ms: Vec<u128> = build_times.iter().map(|d| d.as_millis()).collect();
        let min_time = times_ms.iter().min().unwrap();
        let max_time = times_ms.iter().max().unwrap();
        let variance = max_time - min_time;

        // Variance should be small due to normalization (allow up to 40% due to jitter)
        assert!(
            variance < (TARGET_BUILD_TIME_MS as u128 * 40 / 100),
            "Timing variance too large: {} ms (should be < {} ms)",
            variance,
            TARGET_BUILD_TIME_MS * 40 / 100
        );
    }

    #[tokio::test]
    async fn test_timing_randomness() {
        // SECURITY C5: Verify that delays are actually random and not predictable
        myriadmesh_crypto::init().unwrap();

        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let next = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);

        let local_kp = KeyExchangeKeypair::generate();
        let next_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        let mut route = OnionRoute::new(local, dest, vec![next], 3600);
        route.set_hop_public_key(local, X25519PublicKey::from(&local_kp.public_key));
        route.set_hop_public_key(next, X25519PublicKey::from(&next_kp.public_key));
        route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));

        let payload = b"test message";
        let router = OnionRouter::new_default(local, local_kp.clone());
        let layers = router.build_onion_layers_sync(&route, payload).unwrap();

        // Measure multiple peel operations
        let mut delays = Vec::new();
        for _ in 0..10 {
            let next_router = OnionRouter::new_default(next, next_kp.clone());
            let start = std::time::Instant::now();
            let _ = next_router
                .peel_layer_with_timing_protection(&layers[1])
                .await
                .unwrap();
            delays.push(start.elapsed().as_millis());
        }

        // Check that delays are not all identical (randomness is working)
        let unique_delays: std::collections::HashSet<_> = delays.into_iter().collect();
        assert!(
            unique_delays.len() > 1,
            "Timing protection should produce varied delays, got {} unique values",
            unique_delays.len()
        );
    }

    #[test]
    fn test_expired_route_rejected() {
        // SECURITY TEST H10: Verify expired routes cannot be used
        myriadmesh_crypto::init().unwrap();

        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let next = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);

        let local_kp = KeyExchangeKeypair::generate();
        let next_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        // Create route with 0 second lifetime (already expired)
        let mut route = OnionRoute::new(local, dest, vec![next], 0);
        route.set_hop_public_key(local, X25519PublicKey::from(&local_kp.public_key));
        route.set_hop_public_key(next, X25519PublicKey::from(&next_kp.public_key));
        route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));

        // Route should be expired
        assert!(route.is_expired());

        let router = OnionRouter::new_default(local, local_kp);
        let payload = b"test message";

        // Attempting to build layers should fail due to expiration
        let result = router.build_onion_layers_sync(&route, payload);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
    }

    #[tokio::test]
    async fn test_expired_route_rejected_async() {
        // SECURITY TEST H10: Verify expired routes cannot be used (async version)
        myriadmesh_crypto::init().unwrap();

        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let next = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);

        let local_kp = KeyExchangeKeypair::generate();
        let next_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        // Create route with 0 second lifetime (already expired)
        let mut route = OnionRoute::new(local, dest, vec![next], 0);
        route.set_hop_public_key(local, X25519PublicKey::from(&local_kp.public_key));
        route.set_hop_public_key(next, X25519PublicKey::from(&next_kp.public_key));
        route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));

        // Route should be expired
        assert!(route.is_expired());

        let router = OnionRouter::new_default(local, local_kp);
        let payload = b"test message";

        // Attempting to build layers should fail due to expiration
        let result = router
            .build_onion_layers_with_timing_protection(&route, payload)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
    }

    #[test]
    fn test_overused_route_rejected() {
        // SECURITY TEST H10: Verify routes exceeding max uses cannot be used
        myriadmesh_crypto::init().unwrap();

        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let next = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);

        let local_kp = KeyExchangeKeypair::generate();
        let next_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        // Create route with long lifetime
        let mut route = OnionRoute::new(local, dest, vec![next], 3600);
        route.set_hop_public_key(local, X25519PublicKey::from(&local_kp.public_key));
        route.set_hop_public_key(next, X25519PublicKey::from(&next_kp.public_key));
        route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));

        // Route should not be expired
        assert!(!route.is_expired());

        // Set use_count to exceed MAX_ROUTE_USES
        route.use_count = MAX_ROUTE_USES + 1;

        // Route should be marked for retirement
        assert!(route.should_retire(MAX_ROUTE_USES));

        let router = OnionRouter::new_default(local, local_kp);
        let payload = b"test message";

        // Attempting to build layers should fail due to excessive use
        let result = router.build_onion_layers_sync(&route, payload);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("retired"));
    }

    #[tokio::test]
    async fn test_overused_route_rejected_async() {
        // SECURITY TEST H10: Verify routes exceeding max uses cannot be used (async)
        myriadmesh_crypto::init().unwrap();

        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let next = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);

        let local_kp = KeyExchangeKeypair::generate();
        let next_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        // Create route with long lifetime
        let mut route = OnionRoute::new(local, dest, vec![next], 3600);
        route.set_hop_public_key(local, X25519PublicKey::from(&local_kp.public_key));
        route.set_hop_public_key(next, X25519PublicKey::from(&next_kp.public_key));
        route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));

        // Set use_count to exceed MAX_ROUTE_USES
        route.use_count = MAX_ROUTE_USES + 1;

        // Route should be marked for retirement
        assert!(route.should_retire(MAX_ROUTE_USES));

        let router = OnionRouter::new_default(local, local_kp);
        let payload = b"test message";

        // Attempting to build layers should fail due to excessive use
        let result = router
            .build_onion_layers_with_timing_protection(&route, payload)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("retired"));
    }

    #[test]
    fn test_route_use_count_limits() {
        // SECURITY TEST H10: Verify use count limits are enforced correctly
        myriadmesh_crypto::init().unwrap();

        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);

        let mut route = OnionRoute::new(local, dest, vec![], 3600);

        // Fresh route should not need retirement
        assert!(!route.should_retire(MAX_ROUTE_USES));

        // Route at exactly MAX_ROUTE_USES should be retired
        route.use_count = MAX_ROUTE_USES;
        assert!(route.should_retire(MAX_ROUTE_USES));

        // Route just under limit should be ok
        route.use_count = MAX_ROUTE_USES - 1;
        assert!(!route.should_retire(MAX_ROUTE_USES));
    }

    #[test]
    fn test_valid_route_accepted() {
        // SECURITY TEST H10: Verify valid routes are still accepted
        myriadmesh_crypto::init().unwrap();

        let local = NodeId::from_bytes([0u8; NODE_ID_SIZE]);
        let next = NodeId::from_bytes([1u8; NODE_ID_SIZE]);
        let dest = NodeId::from_bytes([2u8; NODE_ID_SIZE]);

        let local_kp = KeyExchangeKeypair::generate();
        let next_kp = KeyExchangeKeypair::generate();
        let dest_kp = KeyExchangeKeypair::generate();

        // Create route with reasonable lifetime and low use count
        let mut route = OnionRoute::new(local, dest, vec![next], 3600);
        route.set_hop_public_key(local, X25519PublicKey::from(&local_kp.public_key));
        route.set_hop_public_key(next, X25519PublicKey::from(&next_kp.public_key));
        route.set_hop_public_key(dest, X25519PublicKey::from(&dest_kp.public_key));
        route.use_count = 10; // Well below MAX_ROUTE_USES

        // Route should be valid
        assert!(!route.is_expired());
        assert!(!route.should_retire(MAX_ROUTE_USES));

        let router = OnionRouter::new_default(local, local_kp);
        let payload = b"test message";

        // Should successfully build layers
        let result = router.build_onion_layers_sync(&route, payload);
        assert!(result.is_ok());
    }
}
