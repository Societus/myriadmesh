# MyriadMesh Security and Cryptography

## Security Overview

MyriadMesh implements defense-in-depth security with multiple layers:

1. **Node Identity**: Cryptographic node identities based on public key pairs
2. **Key Exchange**: Secure key agreement using modern protocols
3. **Message Encryption**: End-to-end encryption for all data messages
4. **Message Authentication**: Digital signatures prevent tampering
5. **Forward Secrecy**: Regular key rotation limits compromise impact
6. **Replay Protection**: Prevents reuse of captured messages
7. **Anonymity Layer**: Optional routing through i2p network

## Cryptographic Primitives

MyriadMesh uses well-established, audited cryptographic libraries:

### Public Key Cryptography

**Ed25519 (Signatures)**
- Key size: 256 bits (32 bytes)
- Signature size: 512 bits (64 bytes)
- Fast signature generation and verification
- Deterministic signatures (RFC 8032)
- Library: libsodium

**X25519 (Key Exchange)**
- Key size: 256 bits (32 bytes)
- Diffie-Hellman key agreement
- Ephemeral keys for forward secrecy
- Library: libsodium

### Symmetric Cryptography

**XSalsa20-Poly1305 (AEAD)**
- Authenticated encryption
- Key size: 256 bits
- Nonce size: 192 bits (24 bytes)
- Authentication tag: 128 bits
- Library: libsodium

### Hash Functions

**BLAKE2b**
- Output size: 512 bits (64 bytes) for Node IDs, 256 bits for other uses (configurable)
- Faster than SHA-3, SHA-2
- Used for:
  - Node ID derivation (BLAKE2b-512 for 64-byte IDs)
  - Message ID generation (BLAKE2b-512, first 16 bytes used)
  - Content addressing
- Library: libsodium

**SHA-256**
- Output size: 256 bits
- Used for ledger block hashing
- Standard compatibility
- Library: OpenSSL or libsodium

## Node Identity

### Key Generation

Each node generates an identity key pair on first run:

```python
# Generate Ed25519 key pair
signing_key = SigningKey.generate()
verify_key = signing_key.verify_key

# Derive node ID from public key (BLAKE2b-512 for 64 bytes)
node_id = blake2b(verify_key.encode(), digest_size=64)

# Save keys
save_key(signing_key, "~/.myriadnode/node.key")
save_key(verify_key, "~/.myriadnode/node.pub")
```

### Node ID Format

```
Node ID: 64 bytes (512 bits)
Derived: BLAKE2b-512(Ed25519_PublicKey)
Representation: Hexadecimal or Base58

Example:
7a3f8c2d9e1b4f6a5c8e9d2a7b3f1c4e9d8a7b6c5e4f3a2b1d9c8e7f6a5b4c3d
2f1e4d3c2b1a9f8e7d6c5b4a3f2e1d9c8b7a6f5e4d3c2b1a0f9e8d7c6b5a4f3e
```

### Key Storage

Private keys are stored encrypted at rest:

```python
# Encrypt private key with passphrase
def save_encrypted_key(key, path, passphrase):
    # Derive encryption key from passphrase
    salt = random_bytes(32)
    encryption_key = argon2id(
        passphrase,
        salt,
        opslimit=MODERATE,
        memlimit=MODERATE
    )

    # Encrypt private key
    nonce = random_bytes(24)
    ciphertext = crypto_secretbox(key, nonce, encryption_key)

    # Save with salt and nonce
    save_file(path, salt + nonce + ciphertext)
```

## Key Exchange Protocol

New nodes establish shared secrets using X25519 Elliptic Curve Diffie-Hellman (ECDH):

### Initial Key Exchange

```
Node A                                    Node B
------                                    ------

Generate ephemeral keypair:
a_priv, a_pub = X25519.generate()

                 KEY_EXCHANGE (INITIATE)
                 - ephemeral_pub: a_pub
                 - signature: sign(a_pub)
                 ----------------------->

                                          Verify signature with A's identity key
                                          Generate ephemeral keypair:
                                          b_priv, b_pub = X25519.generate()

                                          Compute shared secret:
                                          shared = X25519(b_priv, a_pub)

                 KEY_EXCHANGE (RESPOND)
                 - ephemeral_pub: b_pub
                 - signature: sign(b_pub)
                 <-----------------------

Compute shared secret:
shared = X25519(a_priv, b_pub)

Derive session keys:
tx_key, rx_key = HKDF-SHA256(shared,
                              info="MyriadMesh-v1",
                              salt=node_id_A || node_id_B)

                 KEY_EXCHANGE (CONFIRM)
                 - confirm: HMAC(tx_key, "confirm")
                 ----------------------->

                                          Verify confirmation

Session established!
```

### Key Derivation

```python
def derive_session_keys(shared_secret, node_id_a, node_id_b):
    # Determine key order (lower node ID uses tx_key first)
    if node_id_a < node_id_b:
        salt = node_id_a + node_id_b
        label_tx = b"MyriadMesh-v1-A-to-B"
        label_rx = b"MyriadMesh-v1-B-to-A"
    else:
        salt = node_id_b + node_id_a
        label_tx = b"MyriadMesh-v1-B-to-A"
        label_rx = b"MyriadMesh-v1-A-to-B"

    # HKDF for key derivation
    tx_key = HKDF(shared_secret, salt=salt, info=label_tx, length=32)
    rx_key = HKDF(shared_secret, salt=salt, info=label_rx, length=32)

    return tx_key, rx_key
```

### Key Rotation

Keys are rotated regularly to provide forward secrecy:

```python
# Initiate key rotation after 24 hours or 1GB of data
# Note: 24-hour rotation provides enhanced forward secrecy
# compared to the originally planned 90-day rotation
if time_since_exchange > 24_hours or bytes_sent > 1_GB:
    send_key_exchange_rotate()
```

**Rotation Protocol:**
1. Initiator sends KEY_EXCHANGE(ROTATE) with new ephemeral key
2. Responder acknowledges with new ephemeral key
3. New session keys derived
4. Old keys retained for 7 days (for in-flight messages)
5. Messages sent with key version number

**Design Decision:** The implementation uses 24-hour key rotation instead of 90 days
to provide significantly enhanced forward secrecy. This aggressive rotation schedule
ensures that even if a session key is compromised, only 24 hours of communication
can be decrypted. The trade-off is slightly increased overhead for key exchanges,
which is acceptable given modern processing capabilities.

## Message Encryption

### Encryption Process

```python
def encrypt_message(plaintext, shared_key, message_id):
    # Use first 24 bytes of message_id as nonce
    nonce = message_id[0:24]

    # XSalsa20-Poly1305 authenticated encryption
    ciphertext = crypto_secretbox(plaintext, nonce, shared_key)

    return ciphertext  # Includes 16-byte auth tag
```

### Decryption Process

```python
def decrypt_message(ciphertext, shared_key, message_id):
    # Extract nonce from message_id
    nonce = message_id[0:24]

    # Decrypt and verify authentication tag
    try:
        plaintext = crypto_secretbox_open(ciphertext, nonce, shared_key)
        return plaintext
    except CryptoError:
        # Authentication failed - message tampered
        raise MessageAuthenticationError()
```

### Nonce Uniqueness

- Each message has unique message_id (includes timestamp + random)
- First 24 bytes used as nonce
- Nonce reuse protection: track recently used nonces
- Nonce rotation with key rotation

## Message Authentication

### Signature Generation

All messages are signed by the sender:

```python
def sign_message(frame, signing_key):
    # Construct signature payload
    payload = (
        frame.header +  # All header fields
        frame.payload   # Encrypted payload
    )

    # Ed25519 signature
    signature = signing_key.sign(payload)

    return signature
```

### Signature Verification

```python
def verify_signature(frame, signature, sender_public_key):
    # Reconstruct signed payload
    payload = frame.header + frame.payload

    # Verify signature
    try:
        sender_public_key.verify(signature, payload)
        return True
    except BadSignatureError:
        return False
```

### Trust Model

MyriadMesh supports multiple trust models:

**1. Trust on First Use (TOFU)**
- Accept any node's first key exchange
- Remember and trust that key
- Alert on key change

**2. Explicit Trust**
- User manually approves nodes
- Out-of-band key verification (QR code, etc.)
- Reject unapproved nodes

**3. Web of Trust**
- Nodes signed by trusted nodes are trusted
- Transitive trust with limits
- Reputation-based trust scoring

**4. Certificate Authority (Optional)**
- Central CA issues node certificates
- CA public key pre-configured
- Enterprise/organizational use

## Replay Protection

### Seen Message Cache

```python
# Cache of recently seen message IDs
seen_messages = LRU_Cache(max_size=10000, ttl=3600)

def check_replay(message_id, timestamp):
    # Check if already seen
    if message_id in seen_messages:
        raise ReplayAttackDetected()

    # Check timestamp freshness (±5 minutes)
    now = current_time()
    if abs(now - timestamp) > 5 * 60 * 1000:
        raise MessageTimestampInvalid()

    # Add to seen cache
    seen_messages.put(message_id, timestamp)
```

### Timestamp Validation

- Messages must have timestamp within ±5 minutes
- Requires loose time synchronization (NTP)
- Allows for clock drift and network delay
- Stricter validation for critical operations
- Message ID deduplication cache (LRU, configurable size)

**Note:** With 24-hour key rotation, the replay protection window is further
constrained - replayed messages older than 24 hours will fail decryption due
to key rotation, providing an additional layer of replay protection.

## Forward Secrecy

### Ephemeral Keys

- Every key exchange uses new ephemeral keys
- Ephemeral private keys deleted after use
- Compromise of identity key doesn't compromise past sessions

### Key Rotation Schedule

```
Initial Exchange: Hour 0
First Rotation:   Hour 24 (or 1 GB data)
Second Rotation:  Hour 48 (or 2 GB data)
...

Old keys retained for 7 days for in-flight messages
```

**Rationale for 24-Hour Rotation:**
- Enhanced forward secrecy compared to 90-day rotation
- Limits impact of key compromise to 24 hours of traffic
- Minimal performance overhead with modern cryptographic libraries
- Better security posture for high-security scenarios

### Key Version Tracking

Messages include key version to handle rotation:

```
Message sent with:
- Key Version: uint32 (increments on rotation)
- Allows receiver to select correct key
- Old keys purged after retention period
```

## Anonymity and Privacy

### i2p Integration

When anonymity is required:

```python
def send_anonymous_message(message, destination):
    # Route through i2p network
    i2p_dest = lookup_i2p_destination(destination)

    # Message encrypted end-to-end
    encrypted_msg = encrypt_message(message, session_key)

    # Send via i2p tunnel (additional layered encryption)
    i2p_send(i2p_dest, encrypted_msg)
```

**Privacy Properties:**
- Sender IP hidden from destination
- Destination IP hidden from sender
- Intermediary nodes can't read content
- Traffic analysis resistance

### Metadata Minimization

- No unnecessary identifiers in headers
- Node IDs derived from keys (pseudonymous)
- Optional location data (user controlled)
- No logging of message content

### Traffic Analysis Resistance

**Techniques:**
- Constant-rate padding (optional)
- Decoy traffic generation
- Message batching
- Randomized delays

## Threat Model

### In-Scope Threats

**Passive Adversary**
- Eavesdropping on network traffic
- Traffic analysis
- Metadata collection

**Active Adversary**
- Message injection
- Message modification
- Message replay
- Man-in-the-middle attacks

**Compromised Node**
- Single node compromise
- Insider attacks
- Key theft

### Out-of-Scope

- State-level quantum computing (post-quantum crypto future work)
- Physical access to device
- Exploitation of implementation bugs
- Social engineering

## Security Best Practices

### For Users

1. **Use strong passphrase** for key encryption
2. **Verify node identities** out-of-band when possible
3. **Keep software updated** for security patches
4. **Use i2p** for sensitive communication
5. **Enable explicit trust mode** in high-security environments
6. **Regular key rotation** (keep default settings)
7. **Monitor logs** for suspicious activity

### For Developers

1. **Use libsodium** for all cryptographic operations
2. **Never implement custom crypto**
3. **Constant-time operations** to prevent timing attacks
4. **Secure key storage** with OS keychains where available
5. **Input validation** on all external data
6. **Fuzzing and security testing** before release
7. **Security audits** by third parties
8. **Dependency scanning** for vulnerabilities

## Compliance and Standards

### Standards Compliance

- **RFC 8032**: Edwards-Curve Digital Signature Algorithm (EdDSA)
- **RFC 7748**: Elliptic Curves for Security (X25519)
- **RFC 5869**: HMAC-based Extract-and-Expand Key Derivation Function (HKDF)
- **FIPS 180-4**: Secure Hash Standard (SHA-256)

### Security Levels

MyriadMesh provides approximately:
- **128-bit security** for symmetric operations
- **128-bit security** for asymmetric operations (Ed25519, X25519)

Equivalent to:
- RSA 3072-bit
- AES-128

## Cryptographic Agility

### Algorithm Negotiation

Future versions may support:
- Post-quantum key exchange (e.g., Kyber)
- Alternative signature schemes
- Different AEAD constructions

### Version Negotiation

```python
# In DISCOVERY message
supported_crypto = {
    "signatures": ["ed25519", "ed448"],
    "key_exchange": ["x25519", "kyber768"],
    "encryption": ["xsalsa20poly1305", "chacha20poly1305"]
}

# Select common algorithms
negotiated = intersect(local.supported, remote.supported)
```

## Security Auditing

### Audit Logging

Security-relevant events logged:

```
- Node identity created
- Key exchange initiated/completed
- Key rotation performed
- Signature verification failed
- Decryption failed
- Replay attack detected
- Trust relationship established/revoked
- Suspicious activity detected
```

### Ledger as Audit Trail

Blockchain ledger provides immutable record of:
- Node discovery events
- Key exchanges
- Message delivery confirmations

Cannot be retroactively modified, providing accountability.

## Incident Response

### Compromised Node Key

If a node's private key is compromised:

1. **Generate new identity key**
2. **Broadcast KEY_REVOCATION message** (signed with old and new keys)
3. **Re-establish trust** with all known nodes
4. **Investigate** how compromise occurred
5. **Update** all stored references to old key

### Vulnerability Disclosure

Security vulnerabilities should be reported to:
- Email: security@myriadmesh.org
- PGP key: [to be published]
- Responsible disclosure: 90-day embargo

## Future Enhancements

### Post-Quantum Cryptography

Planning for quantum-resistant algorithms:
- **Kyber**: KEM for key exchange
- **Dilithium**: Signatures
- **SPHINCS+**: Stateless hash-based signatures

Migration strategy:
- Hybrid mode: Classical + PQ
- Gradual rollout
- Backward compatibility

### Hardware Security

- **Secure enclaves** (TPM, SGX, etc.)
- **Hardware key storage**
- **Attestation** for node integrity

### Zero-Knowledge Proofs

Potential applications:
- Prove node reputation without revealing history
- Anonymous credentials
- Privacy-preserving metrics

## Next Steps

- [DHT Security Considerations](../protocol/dht-routing.md#security)
- [Network Adapter Security](../protocol/network-adapters.md)
- [Implementation Guidelines](../implementation/security-checklist.md)
