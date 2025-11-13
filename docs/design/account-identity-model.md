# Account and Identity Model

**Status:** Design Phase
**Version:** 1.0
**Last Updated:** 2025-01-13

---

## Overview

MyriadMesh separates **persistent user identities (Accounts)** from **ephemeral node identities (NodeIDs)**. This enables:
- Privacy through unlinkable node sessions
- Persistent message delivery to accounts
- Flexible node ownership (run multiple nodes per account)
- Private home node designation

---

## Core Concepts

### Account (Persistent Identity)

**What it is:**
- A user's permanent identity in the mesh
- Similar to an email address or XMPP JID
- Cryptographically derived from a long-term keypair
- Used as the recipient address for messages

**Key Properties:**
- **Persistent** - Never changes unless user generates new account
- **Publicly known** - Shared with contacts for messaging
- **Portable** - Can be accessed from any node
- **Recoverable** - Can be restored from seed phrase

```rust
pub struct Account {
    /// Account identifier (derived from public key)
    pub address: AccountAddress,

    /// Long-term Ed25519 keypair
    pub keypair: KeyPair,

    /// Human-readable name (optional)
    pub display_name: Option<String>,

    /// Home nodes (where to deliver messages)
    pub home_nodes: Vec<NodeId>,

    /// Private flag (don't advertise home nodes)
    pub private_home: bool,
}

pub struct AccountAddress([u8; 32]);

impl AccountAddress {
    /// Derive from public key (like NodeId)
    pub fn from_public_key(pk: &PublicKey) -> Self {
        AccountAddress(pk.to_bytes())
    }

    /// Human-readable format: account@myriadmesh
    pub fn to_string(&self) -> String {
        format!("{}@myriadmesh", hex::encode(&self.0[0..16]))
    }

    /// From seed phrase (BIP39)
    pub fn from_seed_phrase(phrase: &str) -> Result<(Self, KeyPair)> {
        // TODO: Implement BIP39 derivation
    }
}
```

**Example:**
```
Alice's Account: a3f2d8c4e9b1f6a7@myriadmesh
Bob's Account: 7c9e2a1f3b8d4e5c@myriadmesh
```

---

### NodeID (Ephemeral Identity)

**What it is:**
- A node's identity for routing and DHT operations
- Can be ephemeral (rotated periodically) for privacy
- Multiple nodes can belong to same account
- Used for mesh operations, not user messaging

**Key Properties:**
- **Ephemeral (optional)** - Can rotate every session/hour/day
- **Unlinkable** - Different sessions appear as different nodes
- **Mesh-level** - Used for routing, not application layer
- **Disposable** - No long-term value after session ends

```rust
pub struct NodeId([u8; 32]);

impl NodeId {
    /// Generate ephemeral NodeId (random keypair)
    pub fn generate_ephemeral() -> (Self, KeyPair) {
        let keypair = KeyPair::generate();
        let node_id = NodeId(keypair.public.to_bytes());
        (node_id, keypair)
    }

    /// Derive persistent NodeId from account
    pub fn from_account(account: &Account) -> Self {
        // Deterministically derive from account keypair
        // This creates a persistent NodeId if desired
        NodeId(account.keypair.public.to_bytes())
    }

    /// Check if NodeId is ephemeral
    pub fn is_ephemeral(&self) -> bool {
        // Check if stored in ephemeral keystore vs persistent
        // Or check metadata flag
    }
}
```

---

## Identity Separation

### Why Separate Accounts and Nodes?

| Aspect | Account | NodeID |
|--------|---------|--------|
| **Purpose** | User identity | Mesh routing |
| **Persistence** | Permanent | Ephemeral (optional) |
| **Visibility** | Shared with contacts | Mesh-wide |
| **Linkability** | Intentionally linkable | Intentionally unlinkable |
| **Use Case** | "Send message to Alice" | "Route via node XYZ" |

### Example Scenarios

#### Scenario 1: Privacy-Conscious User

```
Alice has one account but rotates NodeIDs:

Account (persistent): alice@myriadmesh (a3f2d8c4e9b1f6a7...)

NodeIDs (ephemeral):
  - Monday:    node_abc123... (session 1)
  - Tuesday:   node_def456... (session 2)
  - Wednesday: node_ghi789... (session 3)

Mesh sees: Three unrelated nodes
Contacts see: One consistent account (alice@myriadmesh)
```

**Privacy Benefit:** Adversary can't track Alice's physical location across sessions by linking NodeIDs.

---

#### Scenario 2: Multi-Device User

```
Bob has one account but three nodes:

Account (persistent): bob@myriadmesh (7c9e2a1f3b8d4e5c...)

Nodes:
  - Home Server (persistent NodeID): node_home_001...
  - Laptop (ephemeral NodeID):       node_laptop_xyz...
  - Phone (ephemeral NodeID):        node_phone_abc...

Home node stores messages.
Laptop/phone fetch from home node when online.
```

**Benefit:** Seamless multi-device messaging without account duplication.

---

#### Scenario 3: Anonymous Whistleblower

```
Charlie has ephemeral account AND ephemeral NodeID:

Account (ephemeral, disposable): whistleblower@myriadmesh (9a4b7c2d...)
NodeID (ephemeral):              node_anon_123...

After sharing information, Charlie:
  - Deletes ephemeral account
  - Generates new account and NodeID
  - No linkage possible
```

**Benefit:** Whistleblower protection, truly disposable identity.

---

## Home Node Designation

### Problem

Where should messages for an account be delivered?

### Solution: Home Nodes

Users can designate one or more "home nodes" where their messages are stored.

```rust
pub struct HomeNodeConfig {
    /// Which nodes should store messages for this account
    pub home_nodes: Vec<HomeNodeEntry>,

    /// Privacy mode
    pub advertise_publicly: bool,  // Default: false
}

pub struct HomeNodeEntry {
    /// Home node's NodeID
    pub node_id: NodeId,

    /// Priority (1-10, higher = preferred)
    pub priority: u8,

    /// Storage quota (bytes)
    pub quota: Option<u64>,

    /// Auto-fetch on connect?
    pub auto_fetch: bool,
}

impl Account {
    /// Privately flag a node as home node
    pub fn add_home_node(&mut self, node_id: NodeId, priority: u8) {
        let entry = HomeNodeEntry {
            node_id,
            priority,
            quota: None,
            auto_fetch: true,
        };

        self.home_nodes.push(entry);

        // Do NOT broadcast this to mesh (privacy)
        // Only local configuration
    }

    /// Check if a node is designated as home node
    pub fn is_home_node(&self, node_id: &NodeId) -> bool {
        self.home_nodes.iter().any(|entry| &entry.node_id == node_id)
    }
}
```

### Home Node Privacy

**CRITICAL:** Home node designations are **PRIVATE** by default.

```rust
// WRONG: Publicly announce home node
pub fn announce_home_node(account: &Account, node_id: &NodeId) {
    dht.put(account.address, node_id);  // DON'T DO THIS
}

// CORRECT: Private configuration, only node knows
impl Node {
    pub fn configure_as_home_node(&mut self, account: &Account) {
        // Node privately stores that it's a home node for this account
        self.home_accounts.insert(account.address.clone());

        // Other nodes don't know about this relationship
    }
}
```

**Why?** Revealing home node linkage:
- Links account to specific NodeID
- Reveals physical location (if node is stationary)
- Enables targeted surveillance

---

## Message Delivery Model

### DHT-Based Delivery

```
1. Sender wants to send message to alice@myriadmesh

2. Lookup account in DHT:
   dht.get("alice@myriadmesh") → Returns routing info

3. DHT can return:
   a) Home nodes (if alice advertised them publicly)
   b) "Store in DHT" (anonymous delivery)
   c) Relay nodes (intermediate hops)

4. Message is delivered to one of:
   - Alice's designated home node
   - DHT storage (alice retrieves later)
   - Relay node (forwards when alice comes online)
```

### Delayed Delivery (Store-and-Forward)

```rust
pub struct MessageStorage {
    /// Messages awaiting delivery, indexed by account
    pending: HashMap<AccountAddress, Vec<StoredMessage>>,

    /// Storage limits per account
    max_messages_per_account: usize,
    max_total_messages: usize,
}

pub struct StoredMessage {
    /// Message content
    message: EncryptedMessage,

    /// When it arrived
    stored_at: u64,

    /// Expiry (TTL)
    expires_at: u64,

    /// How many hops it's traveled
    hop_count: u8,
}

impl Node {
    /// Store message for later delivery
    pub async fn store_message_for_account(
        &mut self,
        account: &AccountAddress,
        message: EncryptedMessage,
        ttl: Duration,
    ) -> Result<()> {
        // Check if this is a home node for the account
        if self.is_home_node_for(account) {
            // Store permanently (until retrieved)
            self.home_storage.store(account, message).await?;
        } else {
            // Store temporarily (relay)
            let stored = StoredMessage {
                message,
                stored_at: current_timestamp(),
                expires_at: current_timestamp() + ttl.as_secs(),
                hop_count: 0,
            };

            self.relay_storage.store(account, stored).await?;
        }

        Ok(())
    }

    /// Retrieve messages for account
    pub async fn retrieve_messages(
        &mut self,
        account: &AccountAddress,
        auth: AccountAuth,  // Proof of account ownership
    ) -> Result<Vec<EncryptedMessage>> {
        // Verify account ownership
        auth.verify(account)?;

        // Retrieve messages
        let messages = self.home_storage.retrieve_all(account).await?;

        // Delete after retrieval
        self.home_storage.delete_all(account).await?;

        // Notify other nodes: stop storing messages for this account
        self.broadcast_message_retrieved(account).await?;

        Ok(messages)
    }
}
```

### Message Retrieved Notification

When an account retrieves its messages, other nodes can stop storing:

```rust
pub struct MessageRetrievedNotification {
    /// Account that retrieved messages
    account: AccountAddress,

    /// Up to which timestamp messages were retrieved
    retrieved_up_to: u64,

    /// Signature (proves account ownership)
    signature: Vec<u8>,
}

impl Node {
    async fn handle_message_retrieved_notification(
        &mut self,
        notification: MessageRetrievedNotification,
    ) -> Result<()> {
        // Verify signature
        notification.verify()?;

        // Delete messages for this account (up to timestamp)
        self.relay_storage.delete_before(
            &notification.account,
            notification.retrieved_up_to,
        ).await?;

        info!("Deleted stored messages for {} (retrieved by account)", notification.account);

        Ok(())
    }
}
```

---

## Account Authentication

### Problem

How does a node prove it owns an account?

### Solution: Challenge-Response

```rust
pub struct AccountAuth {
    /// Account address
    account: AccountAddress,

    /// Challenge (random nonce from verifier)
    challenge: [u8; 32],

    /// Response (signature of challenge)
    signature: Vec<u8>,
}

impl AccountAuth {
    pub fn verify(&self, account: &AccountAddress) -> Result<()> {
        // Derive public key from account address
        let public_key = account.to_public_key();

        // Verify signature of challenge
        public_key.verify(&self.challenge, &Signature::from_bytes(&self.signature)?)
            .map_err(|e| anyhow!("Account authentication failed: {}", e))?;

        // Verify account matches
        if &self.account != account {
            bail!("Account mismatch");
        }

        Ok(())
    }

    pub fn generate_challenge() -> [u8; 32] {
        let mut challenge = [0u8; 32];
        getrandom::getrandom(&mut challenge).unwrap();
        challenge
    }

    pub fn sign_challenge(account_keypair: &KeyPair, challenge: &[u8; 32]) -> Vec<u8> {
        account_keypair.sign(challenge).to_bytes().to_vec()
    }
}
```

### Usage Example

```rust
// Node wants to retrieve messages
async fn retrieve_my_messages(
    node: &Node,
    account: &Account,
) -> Result<Vec<EncryptedMessage>> {
    // Get challenge from home node
    let challenge = node.request_challenge(&account.address).await?;

    // Sign challenge with account keypair
    let signature = AccountAuth::sign_challenge(&account.keypair, &challenge);

    // Create auth proof
    let auth = AccountAuth {
        account: account.address.clone(),
        challenge,
        signature,
    };

    // Retrieve messages
    node.retrieve_messages(&account.address, auth).await
}
```

---

## Account Discovery and Contact Exchange

### Problem

How do users share their accounts with contacts?

### Solutions

#### 1. Account QR Code

```rust
pub struct AccountQRCode {
    /// Account address
    account: AccountAddress,

    /// Optional display name
    display_name: Option<String>,

    /// Optional fingerprint verification
    fingerprint: [u8; 8],
}

impl AccountQRCode {
    pub fn generate(account: &Account) -> String {
        let qr_data = AccountQRCode {
            account: account.address.clone(),
            display_name: account.display_name.clone(),
            fingerprint: account.fingerprint(),
        };

        // Encode as QR code
        qrcode::encode(&qr_data.to_json())
    }

    pub fn scan(qr_data: &str) -> Result<AccountQRCode> {
        // Decode and parse
        serde_json::from_str(qr_data)
    }
}
```

#### 2. Account URI Scheme

```
myriadmesh://add-contact?account=a3f2d8c4e9b1f6a7&name=Alice
```

#### 3. DHT Account Directory (Optional, Not Private)

```rust
// Public directory (opt-in only)
pub struct AccountDirectory {
    /// Publicly listed accounts
    accounts: HashMap<String, AccountAddress>,
}

// Alice can register: "alice" → a3f2d8c4e9b1f6a7@myriadmesh
// Others can lookup: "alice" → find account
```

**Privacy Warning:** Public directory reveals account existence. Opt-in only.

---

## Ephemeral vs Persistent NodeIDs

### Configuration

```toml
[node.identity]
mode = "ephemeral"  # Options: persistent, ephemeral, hybrid

# Ephemeral mode settings
[node.identity.ephemeral]
rotation_interval = "24h"  # Rotate every 24 hours
rotation_trigger = "session"  # Options: session, time, manual

# Persistent mode settings
[node.identity.persistent]
derive_from_account = true  # Use account keypair for NodeID
```

### Rotation Strategy

```rust
pub struct NodeIdRotation {
    current_node_id: NodeId,
    current_keypair: KeyPair,
    last_rotation: Instant,
    rotation_interval: Duration,
}

impl NodeIdRotation {
    pub async fn maybe_rotate(&mut self) -> Option<(NodeId, KeyPair)> {
        if self.last_rotation.elapsed() >= self.rotation_interval {
            // Generate new NodeID
            let (new_id, new_keypair) = NodeId::generate_ephemeral();

            // Migrate state (DHT entries, peer connections)
            self.migrate_state(&self.current_node_id, &new_id).await;

            // Update
            self.current_node_id = new_id.clone();
            self.current_keypair = new_keypair.clone();
            self.last_rotation = Instant::now();

            info!("Rotated NodeID to {}", hex::encode(&new_id.0[0..8]));

            Some((new_id, new_keypair))
        } else {
            None
        }
    }

    async fn migrate_state(&self, old_id: &NodeId, new_id: &NodeId) {
        // 1. Announce to peers: "old_id is now new_id" (if trusted)
        // 2. Republish DHT entries under new_id
        // 3. Close old connections
        // 4. Establish new connections
    }
}
```

---

## Security Considerations

### 1. Account Compromise

If account keypair is compromised:
- Attacker can read messages
- Attacker can impersonate user
- **Mitigation:** Seed phrase backup, forward secrecy (Signal Protocol)

### 2. NodeID Linkage

If ephemeral NodeIDs are linked:
- Privacy loss (tracking across sessions)
- **Mitigation:** Strict rotation, avoid reuse, Tor-like circuit switching

### 3. Home Node Exposure

If home node is revealed:
- Physical location leak
- **Mitigation:** Private home nodes, multiple decoy home nodes, Tor/I2P access

### 4. DHT Surveillance

If adversary monitors DHT:
- Can see who's looking up which accounts
- **Mitigation:** Private Information Retrieval (PIR), onion routing for DHT queries

---

## Implementation Roadmap

### Phase 1: Basic Account Support
- [ ] Account keypair generation
- [ ] Account address derivation
- [ ] Persistent account storage
- [ ] Account authentication (challenge-response)

### Phase 2: Home Node Implementation
- [ ] Home node configuration
- [ ] Private home node designation
- [ ] Message storage for accounts
- [ ] Message retrieval with auth

### Phase 3: DHT Integration
- [ ] Account lookup in DHT
- [ ] Store-and-forward message delivery
- [ ] Message retrieved notifications

### Phase 4: Ephemeral NodeIDs
- [ ] NodeID rotation logic
- [ ] State migration on rotation
- [ ] Peer notification protocol

### Phase 5: Advanced Features
- [ ] Multi-device support
- [ ] Account directory (optional)
- [ ] QR code generation
- [ ] Seed phrase backup (BIP39)

---

## Related Documents

- [Heartbeat Protocol](./heartbeat-protocol.md)
- [Message Acknowledgement Protocol](./message-acknowledgement-protocol.md)
- [Bootstrap Trust and Reputation System](./bootstrap-trust-system.md)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-01-13 | Claude | Initial design |
