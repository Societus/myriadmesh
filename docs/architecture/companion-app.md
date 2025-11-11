# MyriadNode Companion App Specification

## Overview

MyriadNode is the companion application that implements the MyriadMesh protocol. It is a server-scale application designed to run continuously, managing network connections, routing messages, and maintaining distributed state.

## System Requirements

### Minimum Requirements
- CPU: 2 cores, 1 GHz
- RAM: 512 MB
- Storage: 1 GB for application + DHT cache
- OS: Linux, Windows, macOS, Android (ARM/x86_64)

### Recommended Requirements
- CPU: 4+ cores, 2+ GHz
- RAM: 2+ GB
- Storage: 10+ GB SSD
- Network: Multiple interface types available

## Application Architecture

### Process Model

MyriadNode runs as a multi-threaded daemon process:

```
┌─────────────────────────────────────────────────┐
│              Main Process                        │
│  ┌────────────────────────────────────────────┐ │
│  │         Configuration Manager               │ │
│  └────────────────────────────────────────────┘ │
│                                                  │
│  ┌────────────────────────────────────────────┐ │
│  │          API Server Thread                  │ │
│  │  - REST endpoints                           │ │
│  │  - WebSocket server                         │ │
│  │  - SSE event stream                         │ │
│  └────────────────────────────────────────────┘ │
│                                                  │
│  ┌────────────────────────────────────────────┐ │
│  │       Message Router Thread                 │ │
│  │  - Priority queue processing                │ │
│  │  - Path selection                           │ │
│  │  - Store-and-forward                        │ │
│  └────────────────────────────────────────────┘ │
│                                                  │
│  ┌────────────────────────────────────────────┐ │
│  │          DHT Manager Thread                 │ │
│  │  - DHT protocol implementation              │ │
│  │  - Periodic replication                     │ │
│  │  - Query routing                            │ │
│  └────────────────────────────────────────────┘ │
│                                                  │
│  ┌────────────────────────────────────────────┐ │
│  │       Ledger Manager Thread                 │ │
│  │  - Block creation                           │ │
│  │  - Consensus participation                  │ │
│  │  - Chain validation                         │ │
│  └────────────────────────────────────────────┘ │
│                                                  │
│  ┌────────────────────────────────────────────┐ │
│  │    Network Monitor Thread Pool              │ │
│  │  - Continuous testing                       │ │
│  │  - Metric collection                        │ │
│  │  - Adapter health checks                    │ │
│  └────────────────────────────────────────────┘ │
│                                                  │
│  ┌────────────────────────────────────────────┐ │
│  │    Crypto Worker Thread Pool                │ │
│  │  - Encryption/decryption                    │ │
│  │  - Signature operations                     │ │
│  │  - Key derivation                           │ │
│  └────────────────────────────────────────────┘ │
│                                                  │
│  ┌────────────────────────────────────────────┐ │
│  │     Network Adapter Processes               │ │
│  │  (isolated processes per adapter)           │ │
│  │  - Adapter 1 (Ethernet)                     │ │
│  │  - Adapter 2 (Bluetooth)                    │ │
│  │  - Adapter 3 (LoRa)                         │ │
│  │  - ...                                      │ │
│  └────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

### Data Storage

#### Configuration Files
```
~/.myriadnode/
├── config.yaml              # Main configuration
├── node.key                 # Node identity private key
├── node.pub                 # Node identity public key
├── trusted_nodes.db         # Known/trusted node database
└── adapters/                # Adapter-specific configs
    ├── ethernet.yaml
    ├── bluetooth.yaml
    ├── lora.yaml
    └── ...
```

#### Runtime Data
```
/var/lib/myriadnode/
├── messages.db              # Message queue database
├── ledger/                  # Blockchain data
│   ├── blocks/              # Individual block files
│   └── index.db             # Block index
├── dht/                     # DHT cache
│   ├── nodes.db             # Known nodes
│   └── cache.db             # Cached messages
└── metrics/                 # Performance metrics
    └── timeseries.db        # Time-series database
```

#### Logs
```
/var/log/myriadnode/
├── node.log                 # Main application log
├── api.log                  # API access log
├── adapters/                # Per-adapter logs
└── audit.log                # Security audit log
```

## Core Services

### 1. API Server

#### REST API Endpoints

**Node Management**
```
GET    /api/v1/node/status          # Node status and capabilities
GET    /api/v1/node/info            # Node identity and version
POST   /api/v1/node/shutdown        # Graceful shutdown
GET    /api/v1/node/config          # Get configuration
PUT    /api/v1/node/config          # Update configuration
```

**Message Operations**
```
POST   /api/v1/messages             # Send new message
GET    /api/v1/messages/:id         # Get message status
GET    /api/v1/messages             # List messages (paginated)
DELETE /api/v1/messages/:id         # Cancel pending message
```

**Network Adapters**
```
GET    /api/v1/adapters             # List all adapters
GET    /api/v1/adapters/:name       # Get adapter details
POST   /api/v1/adapters/:name/start # Start adapter
POST   /api/v1/adapters/:name/stop  # Stop adapter
GET    /api/v1/adapters/:name/test  # Test adapter connectivity
```

**DHT Operations**
```
GET    /api/v1/dht/nodes            # List known nodes
GET    /api/v1/dht/nodes/:id        # Get node details
POST   /api/v1/dht/query            # Perform DHT query
GET    /api/v1/dht/cache            # Get cached messages
```

**Ledger**
```
GET    /api/v1/ledger/blocks        # List blocks
GET    /api/v1/ledger/blocks/:hash  # Get block details
GET    /api/v1/ledger/search        # Search ledger
GET    /api/v1/ledger/stats         # Ledger statistics
```

**Performance Metrics**
```
GET    /api/v1/metrics/adapters     # Adapter performance data
GET    /api/v1/metrics/routes       # Route performance data
GET    /api/v1/metrics/system       # System resource usage
```

**Key Management**
```
GET    /api/v1/keys/public          # Get node public key
POST   /api/v1/keys/exchange        # Initiate key exchange
GET    /api/v1/keys/trusted         # List trusted nodes
POST   /api/v1/keys/trust           # Add trusted node
DELETE /api/v1/keys/trust/:id       # Remove trusted node
```

#### WebSocket Events

Real-time updates pushed to connected clients:

```javascript
// Connection
ws://localhost:8080/api/v1/ws

// Event types
{
  "type": "adapter.status",
  "adapter": "ethernet",
  "status": "connected",
  "timestamp": "2025-11-11T12:34:56Z"
}

{
  "type": "message.received",
  "messageId": "abc123",
  "from": "node456",
  "timestamp": "2025-11-11T12:34:56Z"
}

{
  "type": "message.delivered",
  "messageId": "def789",
  "via": "lora",
  "timestamp": "2025-11-11T12:34:56Z"
}

{
  "type": "node.discovered",
  "nodeId": "node789",
  "adapters": ["bluetooth", "wifi"],
  "timestamp": "2025-11-11T12:34:56Z"
}

{
  "type": "route.changed",
  "destination": "node456",
  "oldAdapter": "cellular",
  "newAdapter": "wifi",
  "reason": "better_performance",
  "timestamp": "2025-11-11T12:34:56Z"
}
```

### 2. Message Router

#### Message Priority Levels
- **EMERGENCY**: Emergency/SOS messages (highest priority)
- **HIGH**: Time-sensitive operational data
- **NORMAL**: Standard messages
- **LOW**: Bulk data, logs, telemetry
- **BACKGROUND**: DHT sync, ledger propagation

#### Routing Algorithm

```
function selectRoute(message, destination):
    # Get available adapters
    adapters = getAvailableAdapters()

    # Filter by destination capabilities
    capable = filter(adapters, a => destination.supports(a))

    # Get performance metrics for each adapter
    metrics = []
    for adapter in capable:
        perf = getPerformanceMetrics(adapter, destination)
        score = calculateWeightedScore(perf, message.priority)
        metrics.append((adapter, score, perf))

    # Sort by score (higher is better)
    metrics.sort(key=lambda x: x[1], reverse=True)

    # Return ranked list for failover
    return [m[0] for m in metrics]

function calculateWeightedScore(perf, priority):
    # Weights vary by priority
    if priority == EMERGENCY:
        # Prioritize reliability and availability
        return (perf.reliability * 0.5 +
                perf.availability * 0.4 +
                (1 - perf.latency_normalized) * 0.1)
    elif priority == HIGH:
        # Balance latency and reliability
        return ((1 - perf.latency_normalized) * 0.4 +
                perf.reliability * 0.4 +
                perf.bandwidth_normalized * 0.2)
    elif priority == NORMAL:
        # Balance all factors
        return ((1 - perf.latency_normalized) * 0.2 +
                perf.reliability * 0.3 +
                perf.bandwidth_normalized * 0.2 +
                (1 - perf.cost_normalized) * 0.2 +
                perf.availability * 0.1)
    else:  # LOW or BACKGROUND
        # Prioritize cost and bandwidth
        return ((1 - perf.cost_normalized) * 0.5 +
                perf.bandwidth_normalized * 0.3 +
                perf.reliability * 0.2)
```

#### Store-and-Forward

Messages to unreachable nodes are:
1. Encrypted and stored in local database
2. Metadata added to DHT cache
3. Periodic retry attempts based on node history
4. Delivered when node becomes reachable
5. Optionally relayed through intermediate nodes

### 3. DHT Manager

#### DHT Schema

```
NodeRecord {
    nodeId: bytes32           # Unique node identifier
    publicKey: bytes          # Ed25519 public key
    adapters: [               # Available network adapters
        {
            type: string      # e.g., "bluetooth", "lora"
            address: string   # Adapter-specific address
            lastSeen: timestamp
        }
    ]
    location: {               # Optional
        lat: float
        lon: float
        accuracy: float
        timestamp: timestamp
    }
    capabilities: {
        relay: boolean        # Can relay messages
        cache: boolean        # Can cache messages
        i2p: boolean          # Has i2p support
    }
    reputation: float         # 0.0 - 1.0
    lastUpdated: timestamp
}

RouteRecord {
    sourceNode: bytes32
    destNode: bytes32
    adapter: string
    metrics: {
        latency: float        # milliseconds
        bandwidth: float      # bytes/sec
        reliability: float    # 0.0 - 1.0
        lastTest: timestamp
        sampleCount: int
    }
}

CachedMessage {
    messageId: bytes32
    destNode: bytes32
    encryptedPayload: bytes
    priority: int
    expiresAt: timestamp
    cacheNode: bytes32        # Node storing this cache
}
```

#### DHT Protocol

Based on Kademlia-style routing:
- 160-bit node ID space
- k-bucket routing table (k=20)
- XOR distance metric
- Iterative lookups
- Replication factor of 3

### 4. Ledger Manager

#### Block Structure

```
Block {
    header: {
        version: int
        height: int
        timestamp: timestamp
        previousHash: bytes32
        merkleRoot: bytes32
        signature: bytes         # Signed by block creator
    }
    entries: [
        {
            type: enum           # DISCOVERY, TEST, MESSAGE, KEY_EXCHANGE
            timestamp: timestamp
            nodeId: bytes32
            data: bytes          # Type-specific data
            signature: bytes     # Signed by reporting node
        }
    ]
}
```

#### Entry Types

**DISCOVERY**: Node joined network
```
{
    type: "DISCOVERY",
    nodeId: "...",
    publicKey: "...",
    adapters: [...],
    discoveredBy: "...",
    timestamp: "..."
}
```

**TEST**: Network performance test result
```
{
    type: "TEST",
    sourceNode: "...",
    destNode: "...",
    adapter: "...",
    latency: 45.2,
    bandwidth: 125000,
    success: true,
    timestamp: "..."
}
```

**MESSAGE**: Message delivery confirmation
```
{
    type: "MESSAGE",
    messageId: "...",
    sourceNode: "...",
    destNode: "...",
    adapter: "...",
    delivered: true,
    timestamp: "..."
}
```

**KEY_EXCHANGE**: Key exchange operation
```
{
    type: "KEY_EXCHANGE",
    node1: "...",
    node2: "...",
    keyHash: "...",         # Hash of exchanged key
    timestamp: "..."
}
```

#### Consensus Mechanism

Lightweight consensus for decentralized ledger:
- Proof of Participation (PoP)
- Nodes earn "reputation" by relaying messages
- Block creation rotates among high-reputation nodes
- Blocks require signatures from 2/3 of known nodes
- Fork resolution: longest chain with highest total reputation

### 5. Network Performance Monitor

#### Test Types

**Ping Test**
- Send minimal packet to destination
- Measure round-trip time
- Frequency: Every 5 minutes

**Throughput Test**
- Transfer 1 MB test payload
- Measure transfer rate
- Frequency: Every 30 minutes

**Reliability Test**
- Send 100 packets
- Measure loss rate
- Frequency: Every hour

**Cost Test**
- Query adapter for cost per byte
- Update from configuration or API
- Frequency: On adapter start and daily

#### Metric Storage

Time-series database stores:
- Raw test results
- 5-minute aggregates (retain 7 days)
- Hourly aggregates (retain 30 days)
- Daily aggregates (retain 1 year)

#### Failover Triggers

Automatic failover when:
- Adapter becomes unavailable
- Latency exceeds threshold (configurable, default 5x baseline)
- Packet loss exceeds threshold (default 25%)
- Three consecutive test failures
- Cost exceeds budget limit

## Configuration

### config.yaml Example

```yaml
node:
  id: "auto-generated-on-first-run"
  name: "my-myriad-node"
  primary: true  # Is this the user's primary node?

api:
  enabled: true
  bind: "0.0.0.0"
  port: 8080
  tls:
    enabled: false
    cert: "/path/to/cert.pem"
    key: "/path/to/key.pem"
  auth:
    type: "token"  # or "none", "basic"
    token: "secret-api-token"

dht:
  enabled: true
  bootstrap_nodes:
    - "bootstrap1.myriadmesh.org:4001"
    - "bootstrap2.myriadmesh.org:4001"
  port: 4001
  cache_messages: true
  cache_ttl: "7d"

ledger:
  enabled: true
  participate_consensus: true
  min_reputation: 0.5  # Min reputation to create blocks
  pruning:
    enabled: true
    keep_blocks: 10000  # Keep most recent N blocks

network:
  adapters:
    ethernet:
      enabled: true
      interface: "eth0"
    bluetooth:
      enabled: true
      discoverable: true
    lora:
      enabled: true
      frequency: 915.0
      power: 20
    cellular:
      enabled: true
      apn: "internet"
      cost_per_mb: 0.10

  monitoring:
    ping_interval: "5m"
    throughput_interval: "30m"
    reliability_interval: "1h"

  failover:
    auto_failover: true
    latency_threshold_multiplier: 5.0
    loss_threshold: 0.25
    retry_attempts: 3

security:
  key_rotation_interval: "90d"
  require_signatures: true
  trusted_nodes_only: false

i2p:
  enabled: true
  sam_host: "127.0.0.1"
  sam_port: 7656
  tunnel_length: 3

routing:
  max_hops: 10
  store_and_forward: true
  message_ttl: "7d"

logging:
  level: "info"  # debug, info, warn, error
  file: "/var/log/myriadnode/node.log"
  max_size: "100MB"
  max_backups: 10
```

## Deployment

### Linux Service (systemd)

```ini
[Unit]
Description=MyriadNode - Multi-Network Communication Node
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=myriadnode
Group=myriadnode
ExecStart=/usr/local/bin/myriadnode --config /etc/myriadnode/config.yaml
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

# Security hardening
PrivateTmp=true
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/myriadnode /var/log/myriadnode

[Install]
WantedBy=multi-user.target
```

### Docker Container

```dockerfile
FROM ubuntu:22.04
RUN apt-get update && apt-get install -y \
    bluetooth bluez \
    network-manager \
    i2pd \
    && rm -rf /var/lib/apt/lists/*

COPY myriadnode /usr/local/bin/
COPY config.yaml /etc/myriadnode/

EXPOSE 8080 4001
VOLUME ["/var/lib/myriadnode", "/var/log/myriadnode"]

CMD ["/usr/local/bin/myriadnode", "--config", "/etc/myriadnode/config.yaml"]
```

### Android App

APK includes:
- Full MyriadNode implementation
- Native UI for configuration
- Background service
- Integration with Android radios (WiFi, BT, cellular)

## Monitoring and Observability

### Metrics Exported

Prometheus-compatible metrics endpoint:
```
GET /api/v1/metrics/prometheus

# HELP myriadnode_messages_sent_total Total messages sent
# TYPE myriadnode_messages_sent_total counter
myriadnode_messages_sent_total{adapter="lora"} 142

# HELP myriadnode_adapter_latency_ms Adapter latency in milliseconds
# TYPE myriadnode_adapter_latency_ms gauge
myriadnode_adapter_latency_ms{adapter="lora",destination="node123"} 245.5

# HELP myriadnode_dht_nodes Number of nodes in DHT
# TYPE myriadnode_dht_nodes gauge
myriadnode_dht_nodes 47
```

### Health Checks

```
GET /api/v1/health

Response:
{
  "status": "healthy",
  "uptime": "3d 4h 23m",
  "adapters": {
    "ethernet": "healthy",
    "lora": "degraded",
    "bluetooth": "healthy"
  },
  "dht": "healthy",
  "ledger": "healthy"
}
```

## Next Steps

- [Security and Cryptography](../security/cryptography.md)
- [Protocol Specification](../protocol/specification.md)
- [Client UI Design](client-interfaces.md)
