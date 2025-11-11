# MyriadMesh Protocol Specification

Version: 0.1.0-draft

## Overview

The MyriadMesh Protocol defines the message format, framing, and communication rules for nodes participating in the MyriadMesh network. The protocol is designed to be:

- **Transport-agnostic**: Works over any reliable or unreliable transport
- **Extensible**: Supports future message types and features
- **Efficient**: Minimal overhead for resource-constrained networks
- **Secure**: Encryption and authentication built-in

## Protocol Layers

```
┌─────────────────────────────────────┐
│      Application Layer              │  User messages, commands
├─────────────────────────────────────┤
│      MyriadMesh Message Layer       │  Message framing, routing
├─────────────────────────────────────┤
│      MyriadMesh Transport Layer     │  Adapter abstraction
├─────────────────────────────────────┤
│      Physical Layer                 │  Ethernet, LoRa, BT, etc.
└─────────────────────────────────────┘
```

## Message Frame Structure

All MyriadMesh messages use the following frame structure:

```
┌──────────────────────────────────────────────────────────┐
│  Magic (4 bytes): 0x4D594D53 ("MYMS")                    │
├──────────────────────────────────────────────────────────┤
│  Version (1 byte): Protocol version (currently 0x01)     │
├──────────────────────────────────────────────────────────┤
│  Flags (1 byte): Message flags                           │
├──────────────────────────────────────────────────────────┤
│  Message Type (1 byte): Type of message                  │
├──────────────────────────────────────────────────────────┤
│  Priority (1 byte): Message priority (0-255)             │
├──────────────────────────────────────────────────────────┤
│  TTL (1 byte): Time-to-live (hop count)                  │
├──────────────────────────────────────────────────────────┤
│  Payload Length (2 bytes): Length of payload             │
├──────────────────────────────────────────────────────────┤
│  Message ID (16 bytes): Unique message identifier        │
├──────────────────────────────────────────────────────────┤
│  Source Node ID (32 bytes): Sender's node ID             │
├──────────────────────────────────────────────────────────┤
│  Dest Node ID (32 bytes): Recipient's node ID            │
├──────────────────────────────────────────────────────────┤
│  Timestamp (8 bytes): Unix timestamp in milliseconds     │
├──────────────────────────────────────────────────────────┤
│  Payload (variable): Encrypted message payload           │
├──────────────────────────────────────────────────────────┤
│  Signature (64 bytes): Ed25519 signature of header+payload│
└──────────────────────────────────────────────────────────┘

Total header size: 162 bytes
```

### Field Descriptions

#### Magic (4 bytes)
- Fixed value: `0x4D594D53` (ASCII "MYMS")
- Used to identify MyriadMesh frames
- Helps detect corruption and invalid frames

#### Version (1 byte)
- Protocol version number
- Current version: `0x01`
- Allows future protocol evolution

#### Flags (1 byte)
Bit flags for message properties:
```
Bit 0: Encrypted (1 = payload is encrypted)
Bit 1: Signed (1 = signature present)
Bit 2: Compressed (1 = payload is compressed)
Bit 3: Relay (1 = message is being relayed)
Bit 4: ACK Required (1 = sender wants delivery confirmation)
Bit 5: ACK Message (1 = this is an acknowledgment)
Bit 6: Broadcast (1 = message to all nodes)
Bit 7: Reserved
```

#### Message Type (1 byte)
```
0x00: Reserved
0x01: DATA - User data message
0x02: CONTROL - Protocol control message
0x03: DHT_QUERY - DHT lookup request
0x04: DHT_RESPONSE - DHT lookup response
0x05: DHT_STORE - Store value in DHT
0x06: DISCOVERY - Node discovery announcement
0x07: HEARTBEAT - Keep-alive message
0x08: KEY_EXCHANGE - Cryptographic key exchange
0x09: LEDGER_BLOCK - Blockchain block
0x0A: LEDGER_QUERY - Query ledger
0x0B: TEST_REQUEST - Performance test request
0x0C: TEST_RESPONSE - Performance test response
0x0D: ROUTE_REQUEST - Request route information
0x0E: ROUTE_RESPONSE - Provide route information
0x0F-0xFF: Reserved for future use
```

#### Priority (1 byte)
```
0-63:   BACKGROUND
64-127: LOW
128-191: NORMAL
192-223: HIGH
224-255: EMERGENCY
```

#### TTL (1 byte)
- Time-to-live: maximum number of hops
- Decremented by each relay node
- Message discarded when TTL reaches 0
- Default: 10 hops

#### Payload Length (2 bytes)
- Length of payload in bytes (0-65535)
- Big-endian byte order

#### Message ID (16 bytes)
- Unique identifier for this message
- Generated using: `BLAKE2b(timestamp + source_id + random_nonce)[0:16]`
- Used for deduplication and tracking

#### Source Node ID (32 bytes)
- Public key hash of sending node
- Derived from: `BLAKE2b(node_public_key)`

#### Destination Node ID (32 bytes)
- Public key hash of recipient node
- Special values:
  - `0xFF...FF` (all F's): Broadcast to all nodes
  - `0x00...00` (all 0's): Reserved

#### Timestamp (8 bytes)
- Unix timestamp in milliseconds
- Big-endian byte order
- Used for message ordering and expiration

#### Payload (variable)
- Encrypted message content
- Format depends on Message Type
- Maximum size: 65535 bytes (practical limit may be lower based on transport)

#### Signature (64 bytes)
- Ed25519 signature of entire message (header + payload)
- Signed with source node's private key
- Allows recipient to verify authenticity

## Message Types

### DATA (0x01)

User application data.

**Payload Structure:**
```
┌──────────────────────────────────────┐
│  Content-Type (1 byte)               │
├──────────────────────────────────────┤
│  Data (variable)                     │
└──────────────────────────────────────┘
```

**Content-Type values:**
```
0x00: Raw binary
0x01: UTF-8 text
0x02: JSON
0x03: Protocol Buffers
0x04: MessagePack
0x05-0xFF: Reserved
```

### CONTROL (0x02)

Protocol control messages.

**Payload Structure:**
```
┌──────────────────────────────────────┐
│  Command (1 byte)                    │
├──────────────────────────────────────┤
│  Parameters (variable)               │
└──────────────────────────────────────┘
```

**Commands:**
```
0x01: PING - Request echo response
0x02: PONG - Echo response
0x03: ROUTE_UPDATE - Notify route change
0x04: NODE_STATUS - Node status update
0x05: ERROR - Error notification
```

### DHT_QUERY (0x03) / DHT_RESPONSE (0x04)

DHT lookup operations.

**QUERY Payload:**
```
┌──────────────────────────────────────┐
│  Query Type (1 byte)                 │
├──────────────────────────────────────┤
│  Key (32 bytes)                      │
└──────────────────────────────────────┘
```

**Query Types:**
```
0x01: FIND_NODE - Find nodes close to key
0x02: FIND_VALUE - Find value for key
0x03: FIND_ROUTE - Find route to node
```

**RESPONSE Payload:**
```
┌──────────────────────────────────────┐
│  Result Count (2 bytes)              │
├──────────────────────────────────────┤
│  Result 1 (variable)                 │
├──────────────────────────────────────┤
│  Result 2 (variable)                 │
├──────────────────────────────────────┤
│  ...                                 │
└──────────────────────────────────────┘
```

### DHT_STORE (0x05)

Store value in DHT.

**Payload:**
```
┌──────────────────────────────────────┐
│  Key (32 bytes)                      │
├──────────────────────────────────────┤
│  Value Length (2 bytes)              │
├──────────────────────────────────────┤
│  Value (variable)                    │
├──────────────────────────────────────┤
│  TTL (4 bytes): seconds until expiry │
└──────────────────────────────────────┘
```

### DISCOVERY (0x06)

Node discovery announcement.

**Payload:**
```
┌──────────────────────────────────────┐
│  Public Key (32 bytes)               │
├──────────────────────────────────────┤
│  Capabilities (4 bytes)              │
├──────────────────────────────────────┤
│  Adapter Count (1 byte)              │
├──────────────────────────────────────┤
│  Adapter 1 Info (variable)           │
├──────────────────────────────────────┤
│  ...                                 │
└──────────────────────────────────────┘
```

**Capabilities (bit flags):**
```
Bit 0: Can relay messages
Bit 1: Can cache messages
Bit 2: Has i2p support
Bit 3: Has location data
Bit 4-31: Reserved
```

**Adapter Info:**
```
┌──────────────────────────────────────┐
│  Adapter Type (1 byte)               │
├──────────────────────────────────────┤
│  Address Length (1 byte)             │
├──────────────────────────────────────┤
│  Address (variable)                  │
└──────────────────────────────────────┘
```

### HEARTBEAT (0x07)

Keep-alive message.

**Payload:**
```
┌──────────────────────────────────────┐
│  Sequence Number (4 bytes)           │
├──────────────────────────────────────┤
│  Uptime (4 bytes): seconds           │
├──────────────────────────────────────┤
│  Load Average (2 bytes): x100        │
└──────────────────────────────────────┘
```

### KEY_EXCHANGE (0x08)

Cryptographic key exchange using X25519.

**Payload:**
```
┌──────────────────────────────────────┐
│  Exchange Type (1 byte)              │
├──────────────────────────────────────┤
│  Ephemeral Public Key (32 bytes)     │
├──────────────────────────────────────┤
│  Nonce (24 bytes)                    │
└──────────────────────────────────────┘
```

**Exchange Types:**
```
0x01: INITIATE - Start key exchange
0x02: RESPOND - Respond to key exchange
0x03: CONFIRM - Confirm shared secret
0x04: ROTATE - Rotate to new key
```

### LEDGER_BLOCK (0x09)

Blockchain block propagation.

**Payload:**
```
┌──────────────────────────────────────┐
│  Block Height (4 bytes)              │
├──────────────────────────────────────┤
│  Previous Hash (32 bytes)            │
├──────────────────────────────────────┤
│  Merkle Root (32 bytes)              │
├──────────────────────────────────────┤
│  Entry Count (2 bytes)               │
├──────────────────────────────────────┤
│  Entries (variable)                  │
└──────────────────────────────────────┘
```

### TEST_REQUEST (0x0B) / TEST_RESPONSE (0x0C)

Network performance testing.

**REQUEST Payload:**
```
┌──────────────────────────────────────┐
│  Test Type (1 byte)                  │
├──────────────────────────────────────┤
│  Test ID (8 bytes)                   │
├──────────────────────────────────────┤
│  Payload Size (2 bytes)              │
├──────────────────────────────────────┤
│  Test Payload (variable)             │
└──────────────────────────────────────┘
```

**Test Types:**
```
0x01: PING - Latency test
0x02: THROUGHPUT - Bandwidth test
0x03: RELIABILITY - Packet loss test
```

**RESPONSE Payload:**
```
┌──────────────────────────────────────┐
│  Test ID (8 bytes)                   │
├──────────────────────────────────────┤
│  Echo Payload (variable)             │
└──────────────────────────────────────┘
```

## Message Flow Examples

### Simple Message Delivery

```
Node A -> Node B (direct connection)

1. Node A constructs DATA message
2. Node A encrypts payload with shared secret
3. Node A signs message with private key
4. Node A sends via best available adapter (e.g., Ethernet)
5. Node B receives message
6. Node B verifies signature
7. Node B decrypts payload
8. Node B delivers to application
9. Node B sends ACK (if requested)
```

### Relayed Message

```
Node A -> Node B (no direct connection, relay via Node C)

1. Node A queries DHT for Node B location
2. DHT returns Node C as relay
3. Node A sends message with:
   - Dest: Node B
   - Next Hop: Node C
   - TTL: 10
   - Relay flag: 1
4. Node C receives message
5. Node C decrements TTL to 9
6. Node C queries DHT for best path to Node B
7. Node C forwards to Node B via LoRa adapter
8. Node B receives message
9. Node B sends ACK to Node A via Node C
```

### Node Discovery

```
New Node X joins network

1. Node X broadcasts DISCOVERY message
2. Nearby nodes receive DISCOVERY
3. Each node:
   - Adds Node X to local DHT
   - Updates routing tables
   - Records in ledger
   - Responds with their own DISCOVERY
4. Node X learns about network topology
5. Node X initiates KEY_EXCHANGE with discovered nodes
6. Encrypted communication established
```

## Adapter Encapsulation

Each network adapter wraps MyriadMesh frames in adapter-specific headers:

### Ethernet/IP
```
[IP Header][UDP Header][MyriadMesh Frame]
```
- Default port: UDP 4001

### Bluetooth
```
[L2CAP Header][MyriadMesh Frame]
```
- Service UUID: `00004d59-0000-1000-8000-00805f9b34fb`

### LoRaWAN
```
[LoRaWAN Header][MyriadMesh Frame (fragmented)]
```
- Messages >255 bytes split across multiple LoRa packets
- Fragment header: `[Frag ID (2B)][Frag Num (1B)][Total Frags (1B)]`

### i2p
```
[i2p Destination][MyriadMesh Frame]
```
- Messages sent to destination's tunnel

## Security Considerations

### Encryption

All DATA messages are encrypted using XSalsa20-Poly1305:
- Shared secret derived from X25519 key exchange
- Per-message nonce: first 24 bytes of Message ID
- Authenticated encryption prevents tampering

### Signatures

All messages (except HEARTBEAT) are signed:
- Ed25519 signature algorithm
- Signature covers entire frame (header + payload)
- Public key validated against Source Node ID

### Replay Protection

- Nodes maintain cache of seen Message IDs (last hour)
- Duplicate messages are silently dropped
- Timestamp must be within ±5 minutes of receiver time

### Key Rotation

- Shared secrets rotated every 90 days (configurable)
- KEY_EXCHANGE message initiates rotation
- Old keys retained for 7 days for in-flight messages

## Performance Optimizations

### Message Batching

Multiple small messages can be batched:
```
[MyriadMesh Frame: Type=DATA_BATCH]
  [Batch Count (2 bytes)]
  [Message 1 Length (2 bytes)][Message 1]
  [Message 2 Length (2 bytes)][Message 2]
  ...
```

### Compression

When Compressed flag set, payload is compressed with:
- Zstandard (zstd) for general data
- Only compress if size reduction >10%

### Partial Signatures

For resource-constrained devices, signature can be optional:
- Signed flag = 0
- Only allowed from trusted nodes
- Reduces overhead by 64 bytes

## Protocol Evolution

### Version Negotiation

Nodes advertise supported versions in DISCOVERY message.
When communicating:
- Use lowest common version
- Fallback to v1 if unknown

### Feature Flags

Future versions may add optional features:
- Advertised in DISCOVERY capabilities
- Negotiated per-connection
- Backward compatible with v1

## Error Handling

### Error Messages

CONTROL messages with ERROR command:
```
┌──────────────────────────────────────┐
│  Command: 0x05 (ERROR)               │
├──────────────────────────────────────┤
│  Error Code (2 bytes)                │
├──────────────────────────────────────┤
│  Original Message ID (16 bytes)      │
├──────────────────────────────────────┤
│  Error Description (variable UTF-8)  │
└──────────────────────────────────────┘
```

**Error Codes:**
```
0x0001: INVALID_SIGNATURE
0x0002: DECRYPTION_FAILED
0x0003: UNKNOWN_DESTINATION
0x0004: TTL_EXCEEDED
0x0005: PAYLOAD_TOO_LARGE
0x0006: UNSUPPORTED_VERSION
0x0007: RATE_LIMIT_EXCEEDED
0x0008: NODE_UNAVAILABLE
```

### Timeouts

- ACK timeout: 30 seconds (ethernet/wifi), 5 minutes (LoRa)
- DHT query timeout: 10 seconds
- Key exchange timeout: 60 seconds

## Implementation Notes

### Endianness

All multi-byte integers are big-endian (network byte order).

### String Encoding

All strings are UTF-8 encoded.

### Time Synchronization

Nodes should use NTP or similar to maintain accurate time.
Messages with timestamps >5 minutes off may be rejected.

### Buffer Sizes

Implementations should support:
- Minimum message size: 162 bytes (header only)
- Recommended max message size: 1024 bytes
- Absolute max message size: 65535 bytes

## Next Steps

- [Network Adapter Specifications](network-adapters.md)
- [Security Details](../security/cryptography.md)
- [DHT Protocol Details](dht-routing.md)
