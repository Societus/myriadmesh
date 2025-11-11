# MyriadMesh

## Overview

MyriadMesh is a multi-network communication aggregation protocol designed to enable resilient, adaptive communication across diverse network technologies. By intelligently routing traffic through the most performant available network at any given time, MyriadMesh ensures reliable message delivery even in challenging network environments.

## Vision

Create a communication infrastructure that:
- Automatically adapts to available network conditions
- Provides seamless failover between communication methods
- Maintains security and message integrity across all transport layers
- Enables offline-first, delay-tolerant messaging
- Integrates with existing privacy-focused networks (i2p)

## Supported Network Technologies

MyriadMesh aggregates and routes traffic across:

### Wired Networks
- **Dial-up Networking**: Legacy modem support for remote/rural areas
- **PPPoE**: Point-to-Point Protocol over Ethernet
- **Ethernet**: Standard wired LAN connections

### Wireless Personal Area Networks
- **Bluetooth Classic**: Short-range device-to-device communication
- **Bluetooth Low Energy (BLE)**: Energy-efficient short-range mesh networking

### Cellular Networks
- **4G/LTE**: Mobile broadband connectivity
- **5G**: Next-generation mobile networks

### Wireless Local/Wide Area Networks
- **Wi-Fi HaLoW (802.11ah)**: Long-range, low-power Wi-Fi
- **LoRaWAN**: Long-range, low-power wide area networking
  - Including Meshtastic protocol relay support

### Radio Frequencies
- **FRS/GMRS**: Family Radio Service / General Mobile Radio Service
- **CB Radio**: Citizens Band radio
- **Shortwave Radio**: Long-distance HF communication
- **Amateur Packet Radio (APRS)**: Ham radio digital communication

## Core Components

### 1. MyriadMesh Protocol
The core protocol specification that defines:
- Message framing and encapsulation
- Network adapter abstraction layer
- Routing and path selection algorithms
- Performance metrics collection

### 2. Companion App (MyriadNode)
A server-scale application providing:
- **Blockchain-style ledger**: Immutable record of node discovery and health metrics
- **Network testing engine**: Continuous performance evaluation of all available networks
- **Weighted failover system**: Intelligent tier-based routing with automatic fallback
- **Key exchange management**: Secure node-to-node authentication
- **Message integrity verification**: End-to-end encryption and tamper detection
- **i2p integration**: Privacy-preserving overlay network support

### 3. Multi-Interface Support
Interact with MyriadNode through:
- **Web Interface**: Browser-based management and monitoring
- **Terminal UI**: Curses-based CLI for server administration
- **Android App**: Mobile management and node operation

### 4. Distributed Hash Table (DHT)
Maintains decentralized records of:
- Node health and availability
- Optimal communication paths between node pairs
- Geographic location data (optional)
- Message cache for delayed delivery

## Key Features

### Adaptive Routing
- Real-time network performance monitoring
- Dynamic path selection based on:
  - Latency
  - Bandwidth
  - Reliability
  - Cost
  - Power consumption
  - Current availability

### Resilient Messaging
- Automatic failover to next-best network
- Store-and-forward for offline nodes
- Message queuing and retry logic
- Delay-tolerant networking (DTN) support

### Security First
- End-to-end encryption for all messages
- Node authentication via key exchange
- Message integrity verification
- Integration with i2p for anonymity
- Blockchain-style audit trail

### Decentralized Architecture
- No single point of failure
- DHT-based node discovery
- Peer-to-peer message routing
- Primary node designation for user preferences

## Use Cases

1. **Emergency Communication**: Maintain connectivity during infrastructure failures
2. **Rural/Remote Areas**: Aggregate spotty coverage from multiple networks
3. **Privacy-Conscious Users**: Route through i2p for anonymous communication
4. **Nomadic/Mobile Users**: Seamless handoff between network types
5. **Mesh Communities**: Build resilient local communication networks
6. **Amateur Radio Integration**: Bridge digital and RF communications
7. **IoT Sensor Networks**: Adaptive telemetry using optimal transport

## Project Status

**Phase**: Planning & Documentation

This project is currently in the planning stage. We are developing comprehensive specifications for the protocol, architecture, and implementation details.

## Documentation

- [Architecture Overview](docs/architecture/overview.md)
- [Protocol Specification](docs/protocol/specification.md)
- [Network Adapters](docs/protocol/network-adapters.md)
- [Companion App Design](docs/architecture/companion-app.md)
- [Security Model](docs/security/cryptography.md)
- [DHT & Routing](docs/protocol/dht-routing.md)
- [Development Roadmap](docs/roadmap/phases.md)

## Contributing

This project is in early planning stages. Documentation contributions and design feedback are welcome.

## License

[To be determined]

## Contact

[Project maintainer information]
