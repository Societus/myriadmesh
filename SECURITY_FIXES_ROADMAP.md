# MyriadMesh Security Fixes Roadmap
## Action Items from Red Team Assessment

**Created:** 2025-11-12
**Source:** SECURITY_AUDIT_RED_TEAM.md
**Status:** PENDING IMPLEMENTATION

This document provides a prioritized, actionable roadmap for addressing the 28 vulnerabilities identified in the red team security assessment. Items are organized by priority and include specific implementation guidance.

---

## 🔴 CRITICAL Priority (Must Fix Before Any Production Use)

### C1: Token Signature Verification Bypass
**File:** `crates/myriadmesh-i2p/src/capability_token.rs:114-135`
**Issue:** Verification doesn't check if signature is from claimed issuer
**Fix:**
```rust
pub fn verify(&self, issuer_public_key: &ed25519::PublicKey) -> Result<bool, String> {
    // Add: Verify issuer_node_id matches the public key
    let derived_node_id = NodeIdentity::derive_node_id(issuer_public_key);
    if self.issuer_node_id != NodeId::from_bytes(*derived_node_id.as_bytes()) {
        return Ok(false);
    }

    // Then verify signature (existing code)
    // ...
}
```
**Status:** ❌ Not Fixed
**Blocks:** i2p privacy mode, capability token system

---

### C2: Sybil Attack on DHT
**File:** `crates/myriadmesh-dht/src/routing_table.rs:78-96`
**Issue:** No admission control, unlimited identity generation
**Fix Options:**
1. **Proof of Work:** Require computational work for NodeID generation
2. **Stake-based:** Require economic stake to join DHT
3. **Invite-based:** Bootstrap via trusted nodes
4. **Hybrid:** Combine multiple approaches

**Recommended Implementation:**
```rust
pub struct ProofOfWork {
    nonce: u64,
    difficulty: u32, // Number of leading zero bits required
}

impl NodeIdentity {
    pub fn generate_with_pow(difficulty: u32) -> Result<(Self, ProofOfWork)> {
        loop {
            let identity = Self::generate()?;
            let nonce = find_valid_nonce(&identity.public_key, difficulty)?;

            if verify_pow(&identity.node_id, nonce, difficulty) {
                return Ok((identity, ProofOfWork { nonce, difficulty }));
            }
        }
    }
}
```
**Status:** ❌ Not Fixed
**Blocks:** All DHT operations, routing security, reputation system

---

### C3: Timing Correlation Attack (No Obfuscation Implemented)
**File:** Multiple - design exists, implementation missing
**Issue:** Design docs promise timing obfuscation, code doesn't implement it
**Fix:** Implement timing obfuscation as designed in `phase2-privacy-protections.md`

**Required Implementation:**
```rust
// In message router
pub async fn apply_timing_obfuscation(&self, msg: &MessageFrame) -> Duration {
    if !self.config.privacy.timing_obfuscation {
        return Duration::from_millis(0);
    }

    // Exponential distribution for realistic delay
    let lambda = 1.0 / self.config.privacy.mean_delay_ms as f64;
    let delay_ms = exponential_random(lambda);
    Duration::from_millis(delay_ms.min(self.config.privacy.max_delay_ms))
}

// Apply before sending
tokio::time::sleep(delay).await;
```
**Status:** ❌ Not Fixed
**Blocks:** Anonymity guarantees, i2p privacy

---

### C4: Nonce Reuse Vulnerability
**File:** `crates/myriadmesh-crypto/src/channel.rs:274-280`
**Issue:** Nonces derived from message_id without uniqueness enforcement
**Fix:**
```rust
pub struct EncryptedChannel {
    // ... existing fields
    nonce_tracker: Arc<Mutex<LruCache<[u8; 24], ()>>>, // Track recent nonces
}

pub fn encrypt_message(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
    // Generate cryptographically random nonce
    let mut nonce_bytes = [0u8; 24];
    sodiumoxide::randombytes::randombytes_into(&mut nonce_bytes);
    let nonce = Nonce::from_bytes(nonce_bytes);

    // Check for reuse (shouldn't happen with random, but verify)
    let mut tracker = self.nonce_tracker.lock().await;
    if tracker.contains(&nonce_bytes) {
        return Err(CryptoError::NonceReuse);
    }
    tracker.put(nonce_bytes, ());

    // Encrypt (existing code)
    // ...
}
```
**Status:** ❌ Not Fixed
**Blocks:** Message encryption security

---

### C5: No UDP Authentication
**File:** `crates/myriadmesh-network/src/adapters/ethernet.rs`
**Issue:** Network layer has no packet authentication
**Fix:** Add HMAC or authenticated encryption at network layer

**Implementation:**
```rust
pub struct AuthenticatedFrame {
    frame: Frame,
    hmac: [u8; 32], // BLAKE2b HMAC
}

impl EthernetAdapter {
    fn send_authenticated(&self, frame: Frame, peer: SocketAddr) -> Result<()> {
        // Derive per-peer key from session key
        let auth_key = self.derive_peer_auth_key(&peer)?;

        // Compute HMAC over frame
        let hmac = compute_hmac(&auth_key, &frame.to_bytes()?);

        let auth_frame = AuthenticatedFrame { frame, hmac };
        self.socket.send_to(&auth_frame.to_bytes()?, peer)?;
        Ok(())
    }

    fn recv_authenticated(&self) -> Result<(Frame, SocketAddr)> {
        let (data, peer) = self.socket.recv_from()?;
        let auth_frame = AuthenticatedFrame::from_bytes(&data)?;

        let auth_key = self.derive_peer_auth_key(&peer)?;
        if !verify_hmac(&auth_key, &auth_frame.frame.to_bytes()?, &auth_frame.hmac) {
            return Err(NetworkError::AuthenticationFailed);
        }

        Ok((auth_frame.frame, peer))
    }
}
```
**Status:** ❌ Not Fixed
**Blocks:** Network security, prevents packet injection

---

### C6: Reputation System Not Byzantine-Resistant
**File:** `crates/myriadmesh-dht/src/reputation.rs`
**Issue:** Purely local reputation, Sybils can boost each other
**Fix:** Implement consensus-based reputation with cross-validation

**Implementation Approach:**
```rust
pub struct ConsensusReputation {
    local_score: f64,
    peer_reports: HashMap<NodeId, ReputationReport>,
    consensus_score: f64,
}

pub struct ReputationReport {
    reporter: NodeId,
    subject: NodeId,
    score: f64,
    evidence: Vec<ProofOfWork>, // Relay confirmations, etc.
    timestamp: u64,
    signature: Vec<u8>,
}

impl ConsensusReputation {
    pub fn calculate_consensus(&mut self) {
        // Weight reports by reporter's own reputation
        // Use median to resist outliers
        // Require minimum number of reports
        let weighted_scores: Vec<f64> = self.peer_reports
            .values()
            .map(|r| r.score * self.get_reporter_weight(&r.reporter))
            .collect();

        self.consensus_score = median(&weighted_scores);
    }
}
```
**Status:** ❌ Not Fixed
**Blocks:** Trust model, relay selection, Sybil resistance

---

### C7: NodeID Collision Attack
**File:** `crates/myriadmesh-crypto/src/identity.rs:89-99`
**Issue:** Birthday paradox allows collision with 2^128 operations
**Fix:** Use full 512-bit BLAKE2b output or add additional entropy

**Implementation:**
```rust
// Option 1: Use full hash
pub fn derive_node_id(public_key: &ed25519::PublicKey) -> NodeId {
    // Use 512-bit hash, store first 256 bits as NodeId
    // Keep full hash for collision detection
    let mut hasher = Blake2b512::new();
    hasher.update(public_key.as_ref());
    hasher.update(b"MyriadMesh-NodeID-v1"); // Domain separation
    let hash = hasher.finalize();

    let mut node_id = [0u8; 32];
    node_id.copy_from_slice(&hash[..32]);
    NodeId::from_bytes_with_full_hash(node_id, hash.to_vec())
}

// Option 2: Include proof-of-work in NodeID derivation
// Combines with C2 fix
```
**Status:** ❌ Not Fixed
**Blocks:** Identity security, authentication

---

## 🟠 HIGH Priority (Required for Security)

### H1: No Key Pinning / Certificate Transparency
**File:** `crates/myriadmesh-crypto/src/channel.rs:175-216`
**Issue:** TOFU vulnerable to MitM on first connection
**Fix:** Implement key pinning + optional certificate transparency

```rust
pub struct TrustStore {
    pinned_keys: HashMap<NodeId, (ed25519::PublicKey, PinSource)>,
    transparency_log: Option<CertificateTransparencyLog>,
}

pub enum PinSource {
    FirstUse,
    Manual,
    OutOfBand(String), // QR code, etc.
    Transparency,
}

impl TrustStore {
    pub fn verify_or_pin(&mut self, node_id: NodeId, public_key: ed25519::PublicKey) -> Result<()> {
        if let Some((pinned, source)) = self.pinned_keys.get(&node_id) {
            if pinned != &public_key {
                return Err(CryptoError::KeyMismatch {
                    node_id,
                    pinned_source: source.clone(),
                });
            }
        } else {
            // Check transparency log if available
            if let Some(log) = &self.transparency_log {
                log.verify_inclusion(&node_id, &public_key)?;
            }

            // Pin on first use
            self.pinned_keys.insert(node_id, (public_key, PinSource::FirstUse));
        }
        Ok(())
    }
}
```
**Status:** ❌ Not Fixed

---

### H2: Timestamp Validation Missing
**File:** `crates/myriadmesh-crypto/src/channel.rs:161-171`
**Fix:**
```rust
const MAX_CLOCK_SKEW: u64 = 300; // 5 minutes

pub fn process_key_exchange_request(&mut self, request: &KeyExchangeRequest) -> Result<KeyExchangeResponse> {
    let now = current_timestamp();

    // Validate timestamp
    if request.timestamp > now + MAX_CLOCK_SKEW {
        return Err(CryptoError::TimestampInFuture);
    }
    if request.timestamp < now.saturating_sub(MAX_CLOCK_SKEW) {
        return Err(CryptoError::TimestampTooOld);
    }

    // Existing code...
}
```
**Status:** ❌ Not Fixed

---

### H3: Multicast Discovery Spoofing
**File:** `crates/myriadmesh-network/src/adapters/ethernet.rs:130-149`
**Fix:** Add HMAC to discovery messages, derive key from shared secret or pre-shared key

```rust
pub struct SignedDiscoveryMessage {
    node_id: NodeId,
    address: SocketAddr,
    timestamp: u64,
    signature: Vec<u8>, // Ed25519 signature
}

impl EthernetAdapter {
    fn send_discovery_announcement(&self) -> Result<()> {
        let msg = DiscoveryMessage {
            node_id: self.local_node_id,
            address: self.local_addr?,
            timestamp: now(),
        };

        // Sign with node identity
        let signature = self.identity.sign(&msg.to_bytes()?);

        let signed_msg = SignedDiscoveryMessage { ...msg, signature };
        self.multicast_socket.send_to(&signed_msg.to_bytes()?, MULTICAST_ADDR)?;
        Ok(())
    }
}
```
**Status:** ❌ Not Fixed

---

### H4: DHT Poisoning via False Node Info
**File:** `crates/myriadmesh-dht/src/node_info.rs:103-117`
**Fix:** Add signed node info, validate claims

```rust
pub struct SignedNodeInfo {
    node_info: NodeInfo,
    signature: Vec<u8>,
    timestamp: u64,
}

impl NodeInfo {
    pub fn sign(&self, identity: &NodeIdentity) -> SignedNodeInfo {
        let mut data = self.to_bytes();
        data.extend_from_slice(&self.timestamp.to_le_bytes());

        let signature = identity.sign(&data);
        SignedNodeInfo {
            node_info: self.clone(),
            signature,
            timestamp: now(),
        }
    }

    pub fn validate_claims(&self) -> Result<()> {
        // Sanity check values
        if self.rtt_ms < 0.1 {
            return Err(DhtError::InvalidClaim("RTT too low"));
        }
        if self.capabilities.max_message_size > MAX_REASONABLE_SIZE {
            return Err(DhtError::InvalidClaim("Message size too large"));
        }
        Ok(())
    }
}
```
**Status:** ❌ Not Fixed

---

### H5-H12: [Additional High Priority Items]
See SECURITY_AUDIT_RED_TEAM.md sections:
- H5: Eclipse Attack (routing_table.rs)
- H6: Reputation Score Manipulation (reputation.rs)
- H7: Onion Route Fingerprinting (onion.rs)
- H8: No Cover Traffic (privacy.rs - not implemented)
- H9: DHT Storage Honey Pot (storage.rs)
- H10: No Message Padding Enforced (privacy.rs - not implemented)
- H11: Reputation Bootstrap Attack (reputation.rs)
- H12: No Secure Memory for Keys (identity.rs)

**Status:** All ❌ Not Fixed

---

## 🟡 MEDIUM Priority (Improve Security Posture)

### M1: Dual Identity Correlation via Timing
**File:** `crates/myriadmesh-i2p/src/dual_identity.rs:99-120`
**Fix:** Add random delays to token operations

```rust
pub fn grant_i2p_access(&self, contact_node_id: NodeId, validity_days: u64) -> Result<I2pCapabilityToken> {
    // Add random delay (50-500ms) to prevent timing correlation
    let delay = Duration::from_millis(50 + rand::random::<u64>() % 450);
    tokio::time::sleep(delay).await;

    // Existing code...
}
```
**Status:** ❌ Not Fixed

---

### M2-M9: [Additional Medium Priority Items]
See SECURITY_AUDIT_RED_TEAM.md sections:
- M2: Weak Random Number Generation
- M3: Key Derivation Without Salt
- M4: No Rate Limiting at Network Layer
- M5: No Proof of Work for DHT Entries
- M6: I2P Destination Leakage
- M7: Metadata Leakage in PublicNodeInfo
- M8: Traffic Analysis via Rate Limiter Stats
- M9: Message Deduplication Cache as Oracle

**Status:** All ❌ Not Fixed

---

## 📋 Implementation Phases

### Phase 3 (Before Production):
**Must complete all CRITICAL items:**
- [ ] C1: Token signature verification
- [ ] C2: Sybil resistance (PoW/stake)
- [ ] C3: Timing obfuscation
- [ ] C4: Nonce uniqueness
- [ ] C5: UDP authentication
- [ ] C6: Byzantine-resistant reputation
- [ ] C7: NodeID collision resistance

### Phase 4 (Hardening):
**Complete all HIGH priority items**
- [ ] H1-H12: See above

### Phase 5 (Refinement):
**Complete MEDIUM priority items**
- [ ] M1-M9: See above

### Ongoing:
- [ ] Fuzzing infrastructure
- [ ] Security audit (third-party)
- [ ] Penetration testing
- [ ] Bug bounty program

---

## 🔍 Testing & Validation

### Security Test Suite Required:

1. **Sybil Attack Tests**
   - Verify PoW enforcement
   - Test DHT admission control
   - Validate reputation consensus

2. **Anonymity Tests**
   - Timing correlation resistance
   - Traffic pattern analysis
   - Message size padding

3. **Cryptographic Tests**
   - Nonce uniqueness
   - Key pinning
   - Signature verification

4. **Network Layer Tests**
   - Packet authentication
   - Replay protection
   - Amplification resistance

---

## 📊 Progress Tracking

| Category | Critical | High | Medium | Total |
|----------|----------|------|--------|-------|
| Identity & Auth | 3/3 | 2/2 | 1/1 | 6/6 |
| Cryptography | 2/2 | 1/1 | 1/1 | 4/4 |
| Network Layer | 2/2 | 2/2 | 2/2 | 6/6 |
| DHT & Routing | 1/1 | 3/3 | 2/2 | 6/6 |
| Anonymity | 0/0 | 3/3 | 3/3 | 6/6 |
| **Total** | **7/7** | **12/12** | **9/9** | **28/28** |
| **Fixed** | **0** | **0** | **0** | **0** |

---

## 🎯 Success Criteria

Before declaring "security hardening complete":

- ✅ All CRITICAL issues fixed
- ✅ All HIGH issues fixed
- ✅ 90%+ MEDIUM issues fixed
- ✅ Fuzzing coverage >80%
- ✅ Third-party security audit passed
- ✅ Penetration test with no critical findings
- ✅ All PoC exploits no longer work

---

## 📝 Notes for Future Phases

### Design vs Implementation Gaps

These features are **documented but not implemented**:
1. Timing obfuscation (phase2-privacy-protections.md)
2. Message padding (phase2-privacy-protections.md)
3. Cover traffic (phase2-privacy-protections.md)
4. Adaptive privacy (phase2-privacy-protections.md)

**Action:** Implement these in Phase 3 or update design docs to remove promises.

### Architecture Changes Needed

Some fixes require architectural changes:
- **Reputation system:** May need complete redesign for Byzantine resistance
- **DHT admission:** Requires consensus on PoW difficulty
- **Network layer auth:** Needs key distribution mechanism
- **Certificate transparency:** Requires distributed log infrastructure

**Action:** Prototype solutions, evaluate trade-offs before implementing.

---

**Last Updated:** 2025-11-12
**Next Review:** After Phase 3 implementation begins
**Owner:** Security Team
