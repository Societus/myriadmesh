# MyriadMesh Architecture Overview

## System Architecture

MyriadMesh consists of three primary layers working in concert to provide adaptive multi-network communication:

```
┌─────────────────────────────────────────────────────────┐
│                  Application Layer                       │
│  ┌──────────┐  ┌──────────┐  ┌─────────────────────┐   │
│  │ Web UI   │  │ Terminal │  │  Android App        │   │
│  │          │  │ UI       │  │                     │   │
│  └────┬─────┘  └────┬─────┘  └──────────┬──────────┘   │
│       │             │                    │              │
│       └─────────────┴────────────────────┘              │
│                     │                                    │
└─────────────────────┼────────────────────────────────────┘
                      │
┌─────────────────────┼────────────────────────────────────┐
│                     │   MyriadNode (Companion App)       │
│  ┌──────────────────▼────────────────────────────────┐  │
│  │              REST API Server                      │  │
│  └──────────────────┬────────────────────────────────┘  │
│                     │                                    │
│  ┌──────────────────┴────────────────────────────────┐  │
│  │            Core Services Layer                    │  │
│  │  ┌───────────┐  ┌───────────┐  ┌──────────────┐  │  │
│  │  │  Message  │  │    DHT    │  │   Ledger     │  │  │
│  │  │  Router   │  │  Manager  │  │   Manager    │  │  │
│  │  └─────┬─────┘  └─────┬─────┘  └──────┬───────┘  │  │
│  │        │              │                │          │  │
│  │  ┌─────▼──────────────▼────────────────▼───────┐  │  │
│  │  │      Network Performance Monitor            │  │  │
│  │  └─────────────────┬─────────────────────────┬─┘  │  │
│  │                    │                         │    │  │
│  │  ┌─────────────────▼───────────┐  ┌──────────▼──┐ │  │
│  │  │  Key Exchange & Crypto      │  │  i2p Bridge │ │  │
│  │  └─────────────────────────────┘  └─────────────┘ │  │
│  └───────────────────────────────────────────────────┘  │
│                                                          │
│  ┌───────────────────────────────────────────────────┐  │
│  │         Network Abstraction Layer                 │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │        Network Adapter Manager              │  │  │
│  │  └────────────────┬────────────────────────────┘  │  │
│  └───────────────────┼───────────────────────────────┘  │
└──────────────────────┼──────────────────────────────────┘
                       │
┌──────────────────────┼───────────────────────────────────┐
│    Network Adapters (Physical/Protocol Layer)            │
│                      │                                    │
│  ┌────┬──────┬───────┼────┬────────┬────────┬─────────┐ │
│  │    │      │       │    │        │        │         │ │
│  ▼    ▼      ▼       ▼    ▼        ▼        ▼         ▼ │
│ ETH  BT   LoRaWAN  Cell  WiFi   Radio    Dial-up   i2p  │
│           /Mesh-                 (HF/CB/            │    │
│           tastic                 APRS)              │    │
└──────────────────────────────────────────────────────────┘
```

## Core Components

### 1. MyriadNode (Companion App)

The central server application that orchestrates all network communication and maintains system state.

#### REST API Server
- Provides HTTP/WebSocket endpoints for UI clients
- Handles authentication and session management
- Exposes real-time status updates via Server-Sent Events (SSE)

#### Message Router
- Receives messages from applications or other nodes
- Determines optimal path based on current network metrics
- Implements store-and-forward for offline destinations
- Manages message priority queues
- Handles automatic retry and failover logic

#### DHT Manager
- Maintains distributed hash table of node information
- Stores node health metrics, locations, and capabilities
- Caches messages for offline nodes
- Participates in DHT gossip protocol
- Handles DHT replication and consistency

#### Ledger Manager
- Maintains blockchain-style record of:
  - Node discovery events
  - Network performance test results
  - Message delivery confirmations
  - Key exchange operations
- Provides immutable audit trail
- Implements consensus mechanism for distributed ledger updates

#### Network Performance Monitor
- Continuously tests available networks
- Measures metrics:
  - Latency (round-trip time)
  - Bandwidth (throughput)
  - Packet loss rate
  - Jitter
  - Availability percentage
  - Cost per byte (for metered connections)
  - Power consumption (for battery-powered nodes)
- Updates weighted tier rankings
- Triggers failover when thresholds exceeded

#### Key Exchange & Cryptography
- Manages node identity keypairs
- Performs initial key exchange with new nodes
- Handles key rotation and revocation
- Encrypts/decrypts messages
- Verifies message signatures
- Implements forward secrecy

#### i2p Bridge
- Integrates with i2p network daemon
- Routes privacy-sensitive traffic through i2p
- Manages i2p tunnel configuration
- Handles i2p destination addressing

### 2. Network Abstraction Layer

Provides a unified interface for all network types, abstracting away protocol-specific details.

#### Network Adapter Manager
- Discovers available network adapters
- Initializes and configures adapters
- Provides common API for sending/receiving data
- Handles adapter lifecycle (start, stop, restart)
- Reports adapter status and capabilities

#### Individual Network Adapters
Each adapter implements the NetworkAdapter interface:
```
interface NetworkAdapter {
    - initialize()
    - send(destination, payload)
    - receive() -> message
    - getStatus() -> AdapterStatus
    - getCapabilities() -> AdapterCapabilities
    - performTest(destination) -> TestResults
    - shutdown()
}
```

Adapters exist for:
- Ethernet/IP networks
- Bluetooth Classic/LE
- Cellular (4G/5G)
- Wi-Fi HaLoW
- LoRaWAN/Meshtastic
- Radio protocols (APRS, CB, shortwave)
- Dial-up/PPPoE
- i2p overlay

### 3. Client Interfaces

#### Web UI
- Single-page application (SPA)
- Real-time dashboard showing:
  - Network status and performance
  - Active connections and routes
  - Message queue status
  - Node map (when location data available)
  - Ledger browser
  - Configuration interface

#### Terminal UI (TUI)
- Curses-based interface for SSH/console access
- Provides all functionality of Web UI
- Optimized for keyboard navigation
- Lower resource usage for headless servers

#### Android App
- Mobile client for portable MyriadNode operation
- Can act as full node or remote management interface
- Integrates with device radios (BT, WiFi, cellular)
- Background service for continuous operation

## Data Flow

### Outbound Message Flow

1. Application sends message to MyriadNode via API
2. Message Router:
   - Identifies destination node
   - Queries DHT for destination node capabilities/status
   - Checks Ledger for historical performance data
   - Consults Network Performance Monitor for current metrics
   - Selects optimal network adapter based on weighted scoring
3. Message is encrypted by Key Exchange & Crypto component
4. Encapsulated message sent via selected Network Adapter
5. Adapter transmits via physical/protocol layer
6. Delivery confirmation (when possible) recorded in Ledger

### Inbound Message Flow

1. Network Adapter receives data from physical layer
2. Adapter validates and forwards to Message Router
3. Key Exchange & Crypto verifies signature and decrypts
4. Message Router determines if:
   - Message is for this node -> deliver to application
   - Message is for another node -> relay using same outbound flow
   - Message is a control message -> handle internally
5. Receipt recorded in Ledger
6. DHT updated with reachability information

### Network Testing Flow

1. Network Performance Monitor schedules test for known node
2. Iterates through available network adapters
3. Each adapter performs test:
   - Send test packet to destination
   - Measure round-trip time
   - Test throughput with payload transfer
   - Record success/failure
4. Results stored in local database
5. Results propagated to DHT
6. Results committed to Ledger
7. Weighted tiers recalculated based on new data

## Scalability Considerations

### Horizontal Scaling
- Multiple MyriadNode instances can run in a cluster
- DHT naturally distributes across nodes
- Ledger uses distributed consensus
- Load balancing via anycast addressing

### Resource Management
- Network adapters run in separate threads/processes
- Message queues use disk-backed storage for large queues
- DHT implements cache eviction policies
- Ledger uses pruning for old entries

### Network Efficiency
- Message aggregation/batching where applicable
- Compression for large payloads
- Differential updates for DHT sync
- Bloom filters for efficient queries

## Deployment Modes

### Full Node
Runs all components, participates in DHT, maintains full ledger

### Light Node
Minimal DHT participation, relies on full nodes for routing

### Gateway Node
Specializes in protocol translation, may only support subset of adapters

### Mobile Node
Optimized for battery life, opportunistic connectivity

## Next Steps

- [Companion App Detailed Design](companion-app.md)
- [Network Adapter Specifications](../protocol/network-adapters.md)
- [Message Flow Diagrams](message-flow.md)
