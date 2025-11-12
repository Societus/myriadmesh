# i2p Integration Architecture - Anonymity Preserving Design

**Critical Security Requirement**: i2p integration must NOT de-anonymize nodes or create linkage between NodeID and i2p destination.

## The Anonymity Problem

### Naive Approach (INSECURE ❌)
```
DHT NodeRecord:
  node_id: 0x1234...
  adapters: [
    { type: "ethernet", address: "192.168.1.100" },
    { type: "i2p", address: "ukeu3k5o...b32.i2p" }  // ❌ LINKS IDENTITY TO i2p!
  ]
```

**Problem**: Anyone can query DHT and see "NodeID 0x1234 uses i2p destination ukeu3k5o"
**Result**: De-anonymization! Can correlate i2p traffic with clearnet NodeID.

## Secure Architecture: Multiple Operating Modes

### Mode 1: **i2p-Only Identity** (Maximum Anonymity)

Node operates ONLY on i2p, never publishes clearnet identity.

```rust
pub struct I2pOnlyNode {
    // Separate identity just for i2p
    i2p_identity_keypair: Ed25519KeyPair,  // Different from clearnet NodeID!
    i2p_node_id: NodeId,  // Derived from i2p_identity_keypair
    i2p_destination: I2pDestination,

    // Never published to clearnet DHT
    clearnet_node_id: None,
}

impl I2pOnlyNode {
    fn announce_to_i2p_dht(&self) {
        // Only announce on i2p-internal DHT
        // NOT on clearnet DHT
        let record = NodeRecord {
            node_id: self.i2p_node_id,  // Different from clearnet!
            adapters: vec![
                AdapterInfo {
                    adapter_type: AdapterType::I2p,
                    address: self.i2p_destination.to_base32(),
                }
            ],
            // ... other fields
        };

        self.i2p_dht.store(record).await;  // Separate i2p DHT!
    }
}
```

**Properties**:
- ✅ Complete anonymity (no clearnet presence)
- ✅ Cannot be linked to clearnet identity
- ✅ i2p destination only discoverable via i2p network
- ❌ Cannot communicate with clearnet-only nodes

**Use Case**: Maximum anonymity mode (journalists, activists, privacy-critical)

---

### Mode 2: **Selective Disclosure** (Dual Identity, Private Linkage)

Node has BOTH clearnet and i2p identities, but linkage is private.

```rust
pub struct DualIdentityNode {
    // Public clearnet identity
    clearnet_node_id: NodeId,
    clearnet_keypair: Ed25519KeyPair,

    // Private i2p identity (NOT publicly linked)
    i2p_keypair: Ed25519KeyPair,  // Separate keypair!
    i2p_node_id: NodeId,  // Different from clearnet_node_id
    i2p_destination: I2pDestination,

    // Capability tokens for authorized contacts
    authorized_contacts: HashMap<NodeId, I2pCapabilityToken>,
}

/// Token allowing contact to reach this node via i2p
pub struct I2pCapabilityToken {
    /// Who can use this token
    for_node: NodeId,

    /// i2p destination to reach
    i2p_destination: I2pDestination,

    /// Optional: i2p-specific NodeID (different from clearnet)
    i2p_node_id: NodeId,

    /// Validity period
    expires_at: Timestamp,

    /// Signature by clearnet NodeID (proves authorization)
    signature: Signature,  // Signed by clearnet_keypair
}

impl DualIdentityNode {
    /// Generate capability token for trusted contact
    pub fn grant_i2p_access(&self, contact_node_id: NodeId) -> I2pCapabilityToken {
        let token = I2pCapabilityToken {
            for_node: contact_node_id,
            i2p_destination: self.i2p_destination.clone(),
            i2p_node_id: self.i2p_node_id,
            expires_at: now() + Duration::from_days(30),
            signature: Signature::placeholder(),
        };

        // Sign with clearnet key (proves this is authorized)
        let message = serialize(&[
            &token.for_node,
            &token.i2p_destination,
            &token.expires_at,
        ]);

        token.signature = self.clearnet_keypair.sign(&message);
        token
    }

    /// Publish to DHT (clearnet identity ONLY)
    pub async fn announce_to_clearnet_dht(&self) {
        let record = NodeRecord {
            node_id: self.clearnet_node_id,
            adapters: vec![
                AdapterInfo { type: AdapterType::Ethernet, address: "..." },
                // ❌ NO i2p destination here!
            ],
            capabilities: NodeCapabilities {
                i2p_capable: true,  // ✅ Advertise capability
                i2p_destination: None,  // ❌ But NOT the destination!
            },
        };

        self.clearnet_dht.store(record).await;
    }

    /// Exchange i2p capability out-of-band
    pub fn exchange_i2p_capability_qr(&self, contact_node_id: NodeId) -> QrCode {
        let token = self.grant_i2p_access(contact_node_id);
        QrCode::encode(&token)
    }
}
```

**Key Exchange Flow**:
```
Alice (clearnet NodeID: 0xAAAA, i2p: ukeu3k5o...)
Bob (clearnet NodeID: 0xBBBB)

1. Bob discovers Alice's clearnet NodeID in DHT
2. Bob sees: capabilities.i2p_capable = true
3. Bob contacts Alice via clearnet: "Send me your i2p token"
4. Alice generates I2pCapabilityToken (signed by 0xAAAA)
5. Alice sends token to Bob (encrypted, private channel)
6. Bob stores token locally (NOT in DHT!)
7. Bob can now reach Alice via i2p using token
```

**Properties**:
- ✅ Clearnet and i2p identities NOT linked publicly
- ✅ Only authorized contacts know i2p destination
- ✅ Can still be reached via clearnet
- ✅ Token signature proves authenticity
- ❌ Initial key exchange requires clearnet or out-of-band channel

**Use Case**: Selective privacy (privacy for specific contacts, public availability for others)

---

### Mode 3: **Anonymous Rendezvous** (Public i2p Availability, Hidden Identity)

Node publishes encrypted pointer to i2p destination, decryptable only by intended recipients.

```rust
pub struct AnonymousRendezvousNode {
    clearnet_node_id: NodeId,
    clearnet_keypair: Ed25519KeyPair,

    i2p_destination: I2pDestination,
    i2p_keypair: Ed25519KeyPair,
}

impl AnonymousRendezvousNode {
    /// Publish encrypted i2p pointer to DHT
    pub async fn publish_i2p_rendezvous(&self) {
        // Derive rendezvous key from clearnet NodeID
        let rendezvous_key = derive_rendezvous_key(self.clearnet_node_id);

        // Encrypt i2p destination with rendezvous key
        let encrypted_destination = encrypt_with_password(
            &self.i2p_destination.to_bytes(),
            &rendezvous_key,
        );

        // Store in DHT at special key
        let dht_key = blake2b(&[b"i2p-rendezvous:", &self.clearnet_node_id]);

        self.dht.store(dht_key, encrypted_destination, ttl: 24_HOURS).await;
    }

    /// Contact via i2p rendezvous
    pub async fn contact_via_i2p(target_node_id: NodeId) -> Result<I2pDestination> {
        // Derive rendezvous key (same derivation as target)
        let rendezvous_key = derive_rendezvous_key(target_node_id);

        // Lookup encrypted destination in DHT
        let dht_key = blake2b(&[b"i2p-rendezvous:", &target_node_id]);
        let encrypted = self.dht.find_value(dht_key).await?;

        // Decrypt with rendezvous key
        let destination_bytes = decrypt_with_password(&encrypted, &rendezvous_key)?;
        let destination = I2pDestination::from_bytes(destination_bytes)?;

        Ok(destination)
    }
}

fn derive_rendezvous_key(node_id: NodeId) -> [u8; 32] {
    // Deterministic key derivation
    // Anyone who knows the NodeID can derive this key
    blake2b(&[b"myriadmesh-i2p-rendezvous-v1:", &node_id])
}
```

**Properties**:
- ✅ i2p destination discoverable by anyone who knows NodeID
- ✅ Not stored in cleartext in DHT
- ✅ Provides plausible deniability (encrypted, could be decoy)
- ❌ Weak security: Key is deterministic from NodeID
- ⚠️ Only obfuscation, not true anonymity

**Use Case**: Semi-public i2p availability (easier discovery, some privacy)

---

### Mode 4: **i2p Transport with Clearnet Routing** (Metadata Privacy Only)

i2p used only as transport layer, MyriadMesh headers still visible.

```rust
pub struct I2pTransportNode {
    node_id: NodeId,  // Same NodeID for clearnet and i2p
    keypair: Ed25519KeyPair,
    i2p_destination: I2pDestination,
}

impl I2pTransportNode {
    /// Publish to DHT (i2p destination visible)
    pub async fn announce_to_dht(&self) {
        let record = NodeRecord {
            node_id: self.node_id,  // Same NodeID
            adapters: vec![
                AdapterInfo {
                    type: AdapterType::I2p,
                    address: self.i2p_destination.to_base32(),
                    // ⚠️ i2p destination is PUBLIC
                },
            ],
        };

        self.dht.store(record).await;
    }

    /// Send message via i2p (MyriadMesh headers visible)
    pub async fn send_via_i2p(&self, msg: MessageFrame) -> Result<()> {
        // Message includes cleartext NodeID headers
        // i2p provides:
        // - IP address anonymity
        // - Traffic mixing
        // - Onion routing

        // But does NOT hide:
        // - Source/Dest NodeIDs
        // - Message metadata

        self.i2p_client.send(self.i2p_destination, &msg.to_bytes()).await
    }
}
```

**Properties**:
- ✅ IP address hidden (transport layer privacy)
- ✅ Traffic analysis resistance from i2p
- ❌ NodeID linkage is public
- ❌ Metadata visible to relays

**Use Case**: IP anonymity without full identity anonymity

---

## i2p Tunnel Verification

Critical: How does recipient verify message actually came through i2p?

```rust
pub enum MessagePath {
    Clearnet {
        adapter: AdapterType,
        source_address: Address,
    },
    I2p {
        tunnel_id: I2pTunnelId,
        destination: I2pDestination,
        verified: bool,  // Cryptographically verified
    },
}

impl MessageRouter {
    async fn receive_message_with_path_verification(
        &self,
        msg: MessageFrame,
        adapter: &dyn NetworkAdapter,
    ) -> Result<(MessageFrame, MessagePath)> {
        let path = match adapter.adapter_type() {
            AdapterType::I2p => {
                // Verify message came through i2p tunnel
                let i2p_adapter = adapter.as_i2p().unwrap();

                // Check SAM session metadata
                let tunnel_id = i2p_adapter.get_active_tunnel_id();
                let destination = i2p_adapter.get_local_destination();

                // Verify message was received on this tunnel
                let verified = self.verify_i2p_receipt(
                    &msg,
                    tunnel_id,
                    destination,
                ).await?;

                MessagePath::I2p {
                    tunnel_id,
                    destination,
                    verified,
                }
            }
            other => {
                MessagePath::Clearnet {
                    adapter: other,
                    source_address: adapter.get_source_address()?,
                }
            }
        };

        Ok((msg, path))
    }

    /// Cryptographically verify message came through i2p
    async fn verify_i2p_receipt(
        &self,
        msg: &MessageFrame,
        tunnel_id: I2pTunnelId,
        destination: I2pDestination,
    ) -> Result<bool> {
        // Check tunnel state
        let tunnel_active = self.i2p_client.is_tunnel_active(tunnel_id).await?;
        if !tunnel_active {
            return Ok(false);
        }

        // Verify destination matches
        let expected_dest = self.i2p_client.get_tunnel_destination(tunnel_id).await?;
        if expected_dest != destination {
            return Ok(false);
        }

        // Check message timing (i2p has higher latency)
        let receive_time = now();
        let send_time = msg.timestamp;
        let latency = receive_time - send_time;

        // i2p messages should have latency > 500ms typically
        if latency < Duration::from_millis(500) {
            // Suspiciously fast for i2p - possible clearnet interception
            return Ok(false);
        }

        // TODO: Additional i2p-specific verification
        // - Check SAM session ID
        // - Verify tunnel build time
        // - Check hop count metadata (if available)

        Ok(true)
    }
}

/// Application-level verification
impl ApplicationLayer {
    async fn receive_message(&self, msg: MessageFrame, path: MessagePath) -> Result<()> {
        // Check if message meets security policy
        match self.security_policy {
            SecurityPolicy::I2pOnly => {
                match path {
                    MessagePath::I2p { verified: true, .. } => {
                        // Good! Message came through i2p
                    }
                    _ => {
                        return Err(Error::PolicyViolation(
                            "Message must arrive via verified i2p tunnel"
                        ));
                    }
                }
            }
            SecurityPolicy::I2pPreferred => {
                if matches!(path, MessagePath::Clearnet { .. }) {
                    // Warn user but accept
                    self.notify_user(SecurityNotification::ClearnetUsed {
                        expected: "i2p",
                        actual: "clearnet",
                    }).await;
                }
            }
            SecurityPolicy::Any => {
                // Accept from any path
            }
        }

        // Deliver message with path metadata
        self.deliver_to_app(msg, path).await
    }
}
```

---

## Preventing Clearnet Interception

Attack: Adversary intercepts i2p-bound message, delivers via clearnet, pretends to be i2p.

**Defense 1: Path-Specific Keys**

```rust
/// Use different keys for i2p vs clearnet
pub struct PathSpecificKeys {
    clearnet_keypair: Ed25519KeyPair,
    i2p_keypair: Ed25519KeyPair,
}

impl MessageRouter {
    fn encrypt_message(&self, msg: Message, path: MessagePath) -> MessageFrame {
        let keypair = match path {
            MessagePath::I2p { .. } => &self.keys.i2p_keypair,
            MessagePath::Clearnet { .. } => &self.keys.clearnet_keypair,
        };

        // Encrypt with path-specific key
        msg.encrypt_with(keypair)
    }
}
```

**Defense 2: i2p-Specific Authentication Tag**

```rust
/// Add i2p-specific authentication data to message
pub struct I2pAuthTag {
    tunnel_id: I2pTunnelId,
    build_time: Timestamp,
    hop_count: u8,
    signature: Signature,  // Signed by i2p tunnel key
}

impl I2pAdapter {
    fn send_with_auth_tag(&self, msg: MessageFrame) -> Result<()> {
        let tunnel = self.get_active_tunnel()?;

        let auth_tag = I2pAuthTag {
            tunnel_id: tunnel.id,
            build_time: tunnel.build_time,
            hop_count: tunnel.hops.len() as u8,
            signature: tunnel.sign_message(&msg)?,
        };

        // Append auth tag to message
        let authenticated_msg = msg.with_i2p_auth(auth_tag);

        self.i2p_client.send(authenticated_msg).await
    }

    fn receive_with_verification(&self, msg: MessageFrame) -> Result<()> {
        // Extract i2p auth tag
        let auth_tag = msg.extract_i2p_auth()?;

        // Verify tunnel signature
        let tunnel = self.get_tunnel(auth_tag.tunnel_id)?;
        if !tunnel.verify_signature(&msg, &auth_tag.signature) {
            return Err(Error::InvalidI2pAuth);
        }

        // Verify tunnel is still active
        if !tunnel.is_active() {
            return Err(Error::TunnelExpired);
        }

        Ok(())
    }
}
```

---

## Configuration

```yaml
i2p:
  mode: "selective_disclosure"  # i2p_only, selective_disclosure, rendezvous, transport

  # Identity configuration
  identity:
    separate_i2p_identity: true  # Use different NodeID for i2p
    i2p_keypair_file: "~/.myriadmesh/i2p_identity.key"

  # Capability tokens
  capability_tokens:
    enabled: true
    expiry_days: 30
    revocation_check_interval: 3600

  # Verification
  verification:
    require_tunnel_verification: true
    min_i2p_latency_ms: 500  # Reject if too fast
    check_sam_session: true

  # Application policies
  policies:
    default: "i2p_preferred"  # i2p_only, i2p_preferred, any
    enforce_path_specific_keys: true
```

---

## Recommendations

### For Maximum Anonymity:
✅ Use **Mode 1: i2p-Only Identity**
- Separate identity
- No clearnet presence
- i2p-only DHT

### For Dual Identity with Privacy:
✅ Use **Mode 2: Selective Disclosure**
- Separate i2p and clearnet identities
- Capability tokens for authorized contacts
- No public linkage in DHT

### For Easier Discovery:
⚠️ Use **Mode 3: Anonymous Rendezvous**
- Encrypted pointers in DHT
- Trade-off: weaker security for convenience

### NOT Recommended for Anonymity:
❌ **Mode 4: i2p Transport Only**
- Use only if you need IP anonymity but not identity anonymity
- Clearnet routing metadata still visible

---

## Implementation Priority

**Phase 2**: Basic i2p transport (Mode 4)
- Simplest implementation
- Provides IP anonymity
- Foundation for other modes

**Phase 3**: Selective disclosure (Mode 2)
- Capability token system
- Path verification
- Application policies

**Phase 4**: Full anonymity modes (Mode 1 & 3)
- Separate identity support
- i2p-only nodes
- Anonymous rendezvous

---

This architecture preserves i2p anonymity while enabling MyriadMesh routing. The key insight is: **never store NodeID → i2p destination mapping in public DHT**.
