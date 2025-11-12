//! MyriadMesh i2p Integration
//!
//! Implements Mode 2 (Selective Disclosure) for privacy-preserving i2p support:
//! - Separate clearnet and i2p identities
//! - Capability token system for private i2p discovery
//! - No public linkage between NodeID and i2p destination
//!
//! ## Security Model
//!
//! **Mode 2: Selective Disclosure** (Default)
//! - Clearnet NodeID: Public (advertised in DHT)
//! - i2p NodeID: Private (different keypair, only in tokens)
//! - i2p destination: Private (never in public DHT)
//! - Discovery: Via signed capability tokens (out-of-band exchange)
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use myriadmesh_i2p::{DualIdentity, I2pDestination};
//!
//! // Alice creates dual identity
//! let alice_dest = I2pDestination::new("alice.b32.i2p".to_string());
//! let alice = DualIdentity::generate(alice_dest);
//!
//! // Bob creates dual identity
//! let bob_dest = I2pDestination::new("bob.b32.i2p".to_string());
//! let mut bob = DualIdentity::generate(bob_dest);
//!
//! // Alice grants Bob access to her i2p destination
//! let token = alice.grant_i2p_access(bob.get_clearnet_node_id(), 30).unwrap();
//!
//! // Bob stores the token (transmitted via encrypted channel)
//! bob.store_capability_token(token).unwrap();
//!
//! // Bob can now reach Alice via i2p using the token
//! let alice_token = bob.get_capability_token(&alice.get_clearnet_node_id()).unwrap();
//! println!("Alice's i2p: {}", alice_token.i2p_destination);
//! ```

pub mod capability_token;
pub mod dual_identity;
pub mod privacy;
pub mod onion;

pub use capability_token::{I2pCapabilityToken, I2pDestination, TokenStorage};
pub use dual_identity::DualIdentity;
pub use privacy::{PrivacyConfig, PrivacyLayer, PaddingStrategy, TimingStrategy};
pub use onion::{OnionRouter, OnionRoute, OnionLayer as OnionRouteLayer};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_end_to_end_capability_exchange() {
        myriadmesh_crypto::init().unwrap();

        // Alice and Bob create dual identities
        let alice_dest = I2pDestination::new("alice.b32.i2p".to_string());
        let alice = DualIdentity::generate(alice_dest).unwrap();

        let bob_dest = I2pDestination::new("bob.b32.i2p".to_string());
        let mut bob = DualIdentity::generate(bob_dest).unwrap();

        // Verify separate identities
        assert!(alice.verify_separate_identities());
        assert!(bob.verify_separate_identities());

        // Alice grants Bob access
        let token = alice
            .grant_i2p_access(bob.get_clearnet_node_id(), 30)
            .unwrap();

        // Verify token signature
        let alice_pubkey = alice.get_clearnet_public_key().unwrap();
        assert!(token.verify(alice_pubkey).unwrap());

        // Bob stores the token
        bob.store_capability_token(token).unwrap();

        // Bob retrieves Alice's i2p info
        let alice_token = bob.get_capability_token(&alice.get_clearnet_node_id());
        assert!(alice_token.is_some());

        let alice_token = alice_token.unwrap();
        assert_eq!(alice_token.i2p_destination, *alice.get_i2p_destination());
        assert_eq!(alice_token.i2p_node_id, alice.get_i2p_node_id());
    }
}
