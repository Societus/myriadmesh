# i2p Anonymity Architecture Review - Phase 2 Code Analysis

**Date**: 2025-11-12
**Status**: 🔴 **CRITICAL SECURITY ISSUE IDENTIFIED**
**Reviewer**: Claude (Phase 2 Implementation Review)

---

## Executive Summary

⚠️ **CRITICAL FINDING**: The current Phase 2 DHT implementation contains a **de-anonymization vulnerability** that would expose i2p destinations publicly in the DHT, violating the i2p anonymity requirements specified in `docs/design/i2p-anonymity-architecture.md`.

**Vulnerability**: NodeID → i2p destination linkage stored in public DHT
**Impact**: De-anonymization of i2p users, correlation of i2p traffic with clearnet identities
**Required Action**: Implement Mode 2 (Selective Disclosure) architecture before deploying i2p adapter

---

## The Vulnerability

### Insecure Pattern (Current Implementation)

The current code stores `NodeInfo` in the DHT, which contains:

```rust
// crates/myriadmesh-dht/src/node_info.rs:51-63
pub struct AdapterInfo {
    pub adapter_type: AdapterType,
    pub address: String,  // ❌ Contains i2p destination!
    pub active: bool,
}

pub struct NodeInfo {
    pub node_id: ProtocolNodeId,
    pub adapters: Vec<AdapterInfo>,  // ❌ Publicly shared in DHT!
    pub reputation: NodeReputation,
    // ...
}
```

This NodeInfo is **publicly distributed** via DHT operations:

```rust
// crates/myriadmesh-dht/src/operations.rs:42-48
pub struct FindNodeResponse {
    pub query_id: QueryId,
    pub nodes: Vec<NodeInfo>,  // ❌ Exposes adapter info!
}

// crates/myriadmesh-dht/src/operations.rs:110-122
pub enum FindValueResponse {
    Found { /* ... */ },
    NotFound {
        query_id: QueryId,
        nodes: Vec<NodeInfo>,  // ❌ Exposes adapter info!
    },
}
```

### Attack Scenario

1. **Alice** runs a node with both clearnet and i2p adapters
2. Alice's NodeInfo is stored in DHT:
   ```
   {
     node_id: 0xABCD1234...,
     adapters: [
       { type: "Ethernet", address: "192.168.1.100" },
       { type: "I2P", address: "ukeu3k5o...b32.i2p" }  // ❌ EXPOSED!
     ]
   }
   ```
3. **Eve** (adversary) queries the DHT for nodes
4. Eve receives Alice's NodeInfo in `FindNodeResponse`
5. **De-anonymization complete**: Eve now knows:
   - Alice's clearnet NodeID: `0xABCD1234`
   - Alice's i2p destination: `ukeu3k5o...b32.i2p`
   - Eve can correlate Alice's i2p traffic with her clearnet identity

### Why This Is Critical

From `docs/design/i2p-anonymity-architecture.md:17-18`:

> **Problem**: Anyone can query DHT and see "NodeID 0x1234 uses i2p destination ukeu3k5o"
> **Result**: De-anonymization! Can correlate i2p traffic with clearnet NodeID.

i2p provides anonymity by hiding the link between IP addresses and destinations. If we expose NodeID → i2p destination in the DHT, we:
- **Undermine i2p's core anonymity guarantee**
- **Create a public database** mapping identities to i2p destinations
- **Enable traffic correlation** across clearnet and i2p networks
- **Expose privacy-conscious users** (journalists, activists, whistleblowers)

---

## Required Architecture: Mode 2 (Selective Disclosure)

The i2p architecture document specifies **Mode 2 as the DEFAULT** for all users:

```yaml
# docs/design/i2p-anonymity-architecture.md:511-513
i2p:
  mode: "selective_disclosure"  # DEFAULT MODE
  identity:
    separate_i2p_identity: true
```

### Mode 2 Requirements

From `docs/design/i2p-anonymity-architecture.md:67-173`:

#### 1. Separate Identities
- **Clearnet NodeID**: Public, stored in DHT
- **i2p NodeID**: Different keypair, NEVER linked to clearnet NodeID publicly

```rust
pub struct DualIdentityNode {
    // Public clearnet identity
    clearnet_node_id: NodeId,
    clearnet_keypair: Ed25519KeyPair,

    // Private i2p identity (SEPARATE!)
    i2p_keypair: Ed25519KeyPair,
    i2p_node_id: NodeId,  // Different from clearnet_node_id
    i2p_destination: I2pDestination,
}
```

#### 2. DHT Record Structure
Clearnet DHT should show i2p **capability**, NOT destination:

```rust
// ✅ SECURE
pub struct NodeDhtRecord {
    node_id: NodeId,  // Clearnet NodeID only
    adapters: vec![
        AdapterInfo { type: AdapterType::Ethernet, address: "..." },
        // ❌ NO i2p destination here!
    ],
    capabilities: NodeCapabilities {
        i2p_capable: true,  // ✅ Advertise capability
        i2p_destination: None,  // ❌ But NOT the destination!
    },
}
```

#### 3. Capability Token System
i2p destination shared **only with authorized contacts** via private exchange:

```rust
pub struct I2pCapabilityToken {
    /// Who can use this token
    for_node: NodeId,

    /// i2p destination (PRIVATE)
    i2p_destination: I2pDestination,

    /// i2p-specific NodeID (different from clearnet)
    i2p_node_id: NodeId,

    /// Expiration
    expires_at: Timestamp,

    /// Signature by clearnet NodeID (proves authorization)
    signature: Signature,
}
```

**Key Exchange Flow** (docs/design/i2p-anonymity-architecture.md:151-163):
1. Bob discovers Alice's clearnet NodeID in DHT
2. Bob sees: `i2p_capable = true` (but NO destination)
3. Bob contacts Alice via clearnet: "Send me your i2p token"
4. Alice generates signed `I2pCapabilityToken`
5. Alice sends token to Bob (encrypted, private channel)
6. Bob stores token **locally** (NOT in DHT!)
7. Bob can now reach Alice via i2p

---

## Code Locations Requiring Changes

### 1. DHT Node Information (`crates/myriadmesh-dht/src/node_info.rs`)

**Current (INSECURE)**:
```rust
pub struct NodeInfo {
    pub node_id: ProtocolNodeId,
    pub adapters: Vec<AdapterInfo>,  // ❌ Contains i2p addresses
}
```

**Required Changes**:
- Remove `adapters` field from `NodeInfo` (or make it clearnet-only)
- Add `capabilities` field with `i2p_capable: bool`
- Create separate `LocalNodeInfo` (private, contains all adapters)
- Create `PublicNodeInfo` (shareable, no i2p destinations)

### 2. DHT Operations (`crates/myriadmesh-dht/src/operations.rs`)

**Current (INSECURE)**:
```rust
pub struct FindNodeResponse {
    pub nodes: Vec<NodeInfo>,  // ❌ Shares full adapter info
}
```

**Required Changes**:
- Return `PublicNodeInfo` instead of `NodeInfo`
- Filter out i2p adapter addresses before sharing
- Add capability flags instead of adapter details

### 3. Network Types (`crates/myriadmesh-network/src/types.rs`)

**Required Additions**:
- `NodeCapabilities` struct with privacy-safe flags
- Separate `PublicAdapterInfo` and `PrivateAdapterInfo`
- i2p capability indication without destination exposure

### 4. New: i2p Capability Token System

**New Files Required**:
- `crates/myriadmesh-i2p/src/capability_token.rs`
- `crates/myriadmesh-i2p/src/identity.rs`
- `crates/myriadmesh-i2p/src/dual_identity.rs`

**Functionality**:
- Generate i2p capability tokens
- Sign tokens with clearnet keypair
- Verify token signatures
- Token expiration and revocation
- Private token storage (NOT in DHT!)

---

## Proposed Architecture Changes

### Phase 1: Fix DHT Information Leak (CRITICAL)

```rust
// crates/myriadmesh-dht/src/node_info.rs
pub struct NodeCapabilities {
    pub i2p_capable: bool,
    pub relay_capable: bool,
    pub store_and_forward: bool,
    // NO i2p destination!
}

pub struct PublicNodeInfo {
    pub node_id: NodeId,
    pub capabilities: NodeCapabilities,
    pub reputation: NodeReputation,
    pub last_seen: u64,
    // NO adapter addresses!
}

pub struct LocalNodeInfo {
    pub node_id: NodeId,
    pub adapters: Vec<AdapterInfo>,  // Private, never shared in DHT
    pub capabilities: NodeCapabilities,
    pub reputation: NodeReputation,
}
```

### Phase 2: Implement Dual Identity System

```rust
// crates/myriadmesh-i2p/src/identity.rs
pub struct DualIdentity {
    /// Public clearnet identity
    pub clearnet_node_id: NodeId,
    clearnet_keypair: Ed25519KeyPair,

    /// Private i2p identity (NEVER linked publicly)
    i2p_node_id: NodeId,
    i2p_keypair: Ed25519KeyPair,
    i2p_destination: I2pDestination,
}

impl DualIdentity {
    pub fn new() -> Result<Self> {
        // Generate TWO separate keypairs
        let clearnet_keypair = Ed25519KeyPair::generate();
        let i2p_keypair = Ed25519KeyPair::generate();

        Ok(Self {
            clearnet_node_id: NodeId::from_public_key(&clearnet_keypair.public_key()),
            clearnet_keypair,
            i2p_node_id: NodeId::from_public_key(&i2p_keypair.public_key()),
            i2p_keypair,
            i2p_destination: I2pDestination::generate()?,
        })
    }

    pub fn get_clearnet_node_id(&self) -> NodeId {
        self.clearnet_node_id
    }

    pub fn get_i2p_node_id(&self) -> NodeId {
        // Different from clearnet!
        self.i2p_node_id
    }
}
```

### Phase 3: Implement Capability Token System

```rust
// crates/myriadmesh-i2p/src/capability_token.rs
pub struct I2pCapabilityToken {
    pub for_node: NodeId,
    pub i2p_destination: I2pDestination,
    pub i2p_node_id: NodeId,
    pub expires_at: u64,
    pub signature: Signature,
}

impl DualIdentity {
    pub fn grant_i2p_access(&self, contact_node_id: NodeId) -> I2pCapabilityToken {
        let expires_at = now() + Duration::from_days(30).as_secs();

        let message = serialize(&[
            &contact_node_id,
            &self.i2p_destination,
            &expires_at,
        ]);

        let signature = self.clearnet_keypair.sign(&message);

        I2pCapabilityToken {
            for_node: contact_node_id,
            i2p_destination: self.i2p_destination.clone(),
            i2p_node_id: self.i2p_node_id,
            expires_at,
            signature,
        }
    }
}
```

---

## Testing Requirements

### Security Tests

```rust
#[test]
fn test_dht_does_not_expose_i2p_destination() {
    let mut dht = Dht::new();
    let node = create_node_with_i2p();

    // Announce to DHT
    dht.announce_node(&node).await.unwrap();

    // Query DHT
    let response = dht.find_node(&node.clearnet_node_id).await.unwrap();

    // Verify i2p destination is NOT in response
    for public_info in response.nodes {
        assert!(public_info.i2p_destination.is_none());
        // Capability flag should be present
        assert_eq!(public_info.capabilities.i2p_capable, true);
    }
}

#[test]
fn test_capability_token_verification() {
    let alice = DualIdentity::new().unwrap();
    let bob_node_id = NodeId::generate();

    // Alice grants Bob access
    let token = alice.grant_i2p_access(bob_node_id);

    // Verify token is signed by Alice's clearnet key
    assert!(token.verify(&alice.clearnet_node_id).unwrap());

    // Verify token is for Bob
    assert_eq!(token.for_node, bob_node_id);

    // Verify token contains i2p destination
    assert_eq!(token.i2p_destination, alice.i2p_destination);
}

#[test]
fn test_separate_identities() {
    let identity = DualIdentity::new().unwrap();

    // Clearnet and i2p NodeIDs must be DIFFERENT
    assert_ne!(identity.clearnet_node_id, identity.i2p_node_id);

    // Should not be derivable from each other
    // (different keypairs)
}
```

---

## Recommendations

### Immediate Actions (Before i2p Adapter Implementation)

1. ✅ **DO NOT implement i2p adapter yet** - would create vulnerability
2. 🔴 **Fix DHT NodeInfo structure** - remove adapter addresses
3. 🔴 **Implement PublicNodeInfo vs LocalNodeInfo** separation
4. 🔴 **Add capability flags** to DHT records
5. 🔴 **Implement dual identity system** with separate keypairs
6. 🔴 **Implement capability token system** for private i2p discovery
7. ✅ **Write security tests** to verify no i2p destination leakage

### Implementation Order

**Phase 2a (Critical Security Fixes)**:
1. Refactor `NodeInfo` → `PublicNodeInfo` + `LocalNodeInfo`
2. Update DHT operations to use `PublicNodeInfo`
3. Add `NodeCapabilities` with `i2p_capable` flag
4. Verify no adapter addresses in DHT responses

**Phase 2b (i2p Identity System)**:
5. Create `myriadmesh-i2p` crate
6. Implement `DualIdentity` with separate keypairs
7. Implement `I2pCapabilityToken` system
8. Add token storage (local only, NOT in DHT)

**Phase 2c (i2p Adapter)**:
9. Implement i2p network adapter (SAM interface)
10. Integrate with dual identity system
11. Add path verification (ensure messages came via i2p)

### Configuration Default

```yaml
# Default configuration (Mode 2)
i2p:
  mode: "selective_disclosure"  # DEFAULT
  identity:
    separate_i2p_identity: true  # REQUIRED
    i2p_keypair_file: "~/.myriadmesh/i2p_identity.key"

  # Capability tokens
  capability_tokens:
    enabled: true
    expiry_days: 30

  # NEVER allow Mode 4 (public linkage) for regular users
  # Only allow for relay/exit nodes with explicit consent
```

---

## References

- **i2p Anonymity Architecture**: `docs/design/i2p-anonymity-architecture.md`
- **Phase 2 Design**: `docs/design/phase2-detailed-design.md`
- **Privacy Protections**: `docs/design/phase2-privacy-protections.md`

---

## Conclusion

The current DHT implementation **MUST NOT** be used with i2p adapters without significant architectural changes. Implementing i2p with the current code would create a **public database of NodeID → i2p destination mappings**, completely undermining i2p's anonymity guarantees.

**Required before deploying i2p support**:
1. Fix DHT information exposure
2. Implement dual identity system (separate keypairs)
3. Implement capability token system (private discovery)
4. Add comprehensive security testing

**Status**: 🔴 **BLOCKING ISSUE** for i2p adapter implementation

The architecture document provides clear guidance (Mode 2: Selective Disclosure) that must be implemented before i2p integration can proceed safely.
