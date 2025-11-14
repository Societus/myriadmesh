# MyriadMesh Development Roadmap

## Overview

This roadmap outlines the phased development approach for building the MyriadMesh protocol and companion application. The project is divided into six major phases, each building upon the previous to create a fully functional multi-network communication system.

## Project Timeline

**Total Estimated Duration:** 18-24 months

```
Phase 1: Foundation         [0-3 months]   ████████░░░░░░░░░░░░░░
Phase 2: Core Protocol      [3-6 months]   ░░░░░░░░████████░░░░░░
Phase 3: Basic Adapters     [6-10 months]  ░░░░░░░░░░░░░░░░████░░
Phase 4: Advanced Features  [10-14 months] ░░░░░░░░░░░░░░░░░░████
Phase 5: Specialized Adapt  [14-18 months] ░░░░░░░░░░░░░░░░░░░░░░
Phase 6: Production Ready   [18-24 months] ░░░░░░░░░░░░░░░░░░░░░░
```

## Phase 1: Foundation (Months 0-3)

**Goal:** Establish project infrastructure and core cryptographic foundations

### 1.1 Project Setup

- [ ] Initialize Git repository
- [ ] Set up CI/CD pipeline (GitHub Actions / GitLab CI)
- [ ] Configure linting and code formatting
- [ ] Establish code review process
- [ ] Set up issue tracking and project management
- [ ] Create contribution guidelines

### 1.2 Core Cryptography

- [ ] Select and integrate libsodium
- [ ] Implement node identity generation
  - Ed25519 key pair generation
  - Node ID derivation (BLAKE2b)
  - Secure key storage
- [ ] Implement key exchange protocol
  - X25519 ECDH
  - Session key derivation (HKDF)
  - Key rotation mechanism
- [ ] Implement message encryption
  - XSalsa20-Poly1305 AEAD
  - Nonce management
- [ ] Implement message signing
  - Ed25519 signatures
  - Signature verification

### 1.3 Protocol Foundation

- [ ] Define protocol data structures
  - Message frame format
  - Header structures
  - Message types enumeration
- [ ] Implement frame serialization/deserialization
- [ ] Message ID generation
- [ ] Basic validation and error handling

### 1.4 Testing Infrastructure

- [ ] Unit test framework setup
- [ ] Integration test framework
- [ ] Test vectors for cryptographic operations
- [ ] Mock network adapters for testing
- [ ] Continuous integration testing

### Deliverables

- Working cryptographic library
- Protocol specification implemented in code
- Test suite with >80% code coverage
- Documentation for core components

### Technologies

- **Language:** Rust (for core implementation)
- **Crypto:** libsodium / sodiumoxide
- **Testing:** cargo test, criterion (benchmarking)
- **CI/CD:** GitHub Actions

## Phase 2: Core Protocol (Months 3-6)

**Goal:** Implement message routing, DHT, and basic networking

### 2.1 DHT Implementation

- [ ] Kademlia routing table
  - K-bucket data structures
  - Bucket maintenance
  - Node insertion/removal
- [ ] DHT operations
  - FIND_NODE lookup
  - STORE operation
  - FIND_VALUE query
- [ ] DHT storage
  - Key-value storage
  - TTL and expiration
  - Republishing mechanism
- [ ] DHT security
  - Signature verification
  - Basic Sybil resistance

### 2.2 Message Router

- [ ] Priority queue implementation
- [ ] Message routing logic
  - Direct routing
  - Multi-hop routing
  - Path selection algorithm
- [ ] Store-and-forward
  - Message caching
  - Offline node handling
  - Cache expiration
- [ ] Message deduplication
- [ ] TTL handling

### 2.3 Network Abstraction Layer

- [ ] Define NetworkAdapter trait/interface
- [ ] Adapter manager
  - Adapter registration
  - Lifecycle management
  - Status monitoring
- [ ] Address abstraction
  - Adapter-specific addressing
  - Address parsing and formatting
- [ ] Message encapsulation for adapters

### 2.4 First Network Adapter: Ethernet/IP

- [ ] UDP transport implementation
- [ ] Local network discovery (multicast)
- [ ] Connection management
- [ ] Adapter testing framework

### 2.5 Component Version Tracking

- [x] Semantic versioning for adapter libraries
- [x] Component manifest structure
- [x] CVE tracking and severity levels
- [x] Reputation penalty calculation
  - Time-based escalation for outdated components
  - CVE-aware penalties
  - Integration with reputation system
- [ ] Automated CVE scanning integration
- [ ] Version update notifications

### Deliverables

- Functional DHT for node discovery
- Message routing between nodes
- Working Ethernet adapter
- Ability to send messages between nodes on same LAN
- Component version tracking system with reputation impact

### Technologies

- **Networking:** tokio (async runtime), quinn (QUIC, optional)
- **Serialization:** bincode / protobuf
- **DHT:** Custom Kademlia implementation

## Phase 3: Basic Adapters & Companion App (Months 6-10)

**Goal:** Build MyriadNode companion app with web UI and add common network adapters

### 3.1 MyriadNode Application

- [ ] Application architecture
  - Multi-threaded design
  - Inter-thread communication
  - Configuration management
- [ ] REST API server
  - API endpoint implementation
  - WebSocket support
  - Authentication/authorization
- [ ] Persistent storage
  - Message queue database
  - Configuration storage
  - Metrics storage
- [ ] Logging and monitoring
  - Structured logging
  - Metrics collection
  - Health checks

### 3.2 Web User Interface

- [ ] Frontend framework setup (React/Vue/Svelte)
- [ ] Dashboard view
  - Node status
  - Network adapter status
  - Active connections
- [ ] Message management
  - Send message form
  - Message queue display
  - Delivery status
- [ ] Configuration interface
  - Adapter configuration
  - Node settings
  - Trust management
- [ ] Real-time updates (WebSocket)

### 3.3 Additional Network Adapters

**Bluetooth Classic:**
- [x] Bluetooth adapter implementation
- [x] Channel-based RFCOMM transport
- [ ] Service discovery (SDP) - Platform integration
- [x] RFCOMM connection management
- [x] Pairing infrastructure

**Bluetooth Low Energy:**
- [x] BLE adapter implementation
- [x] GATT connection management with MTU handling
- [x] Advertisement scanning infrastructure
- [x] Connection pooling and state management

**Cellular (4G/5G):**
- [x] Cellular adapter implementation
- [x] TCP/IP connection management
- [x] Data usage tracking
- [x] Cost monitoring with quota enforcement
- [ ] ModemManager integration (Linux)

### 3.4 Hot-Reloadable Adapter System

- [x] Adapter registry architecture
- [x] Adapter metadata tracking
  - Version information
  - Load time and reload count
  - Connection state
- [x] Zero-downtime adapter updates
  - Graceful connection draining
  - Atomic adapter swapping
  - Automatic initialization
- [x] Connection tracking for draining
- [ ] Health monitoring post-update
  - Success rate tracking
  - Latency monitoring
  - Error rate detection
- [ ] Automatic rollback on degradation
- [ ] Rollback history management

### 3.5 Performance Monitoring

- [ ] Network adapter testing framework
- [ ] Metrics collection
  - Latency measurements
  - Bandwidth testing
  - Reliability tracking
- [ ] Weighted scoring algorithm
- [ ] Automatic failover logic

### Deliverables

- MyriadNode application with web UI
- Functional Ethernet, Bluetooth Classic, Bluetooth LE, Cellular adapters
- Channel-based transport architecture for all adapters
- Hot-reloadable adapter system for zero-downtime updates
- Ability to route messages across multiple network types
- Performance-based adapter selection
- Component version tracking with reputation penalties

### Technologies

- **API Server:** actix-web / axum (Rust) or Express (Node.js)
- **Frontend:** React + TypeScript
- **Database:** SQLite or RocksDB
- **Bluetooth:** bluez (Linux), CoreBluetooth (macOS/iOS)

## Phase 4: Advanced Features (Months 10-14)

**Goal:** Implement ledger, advanced routing, and TUI/Android interfaces

### 4.1 Blockchain Ledger

- [ ] Block structure definition
- [ ] Block creation and validation
- [ ] Consensus mechanism (Proof of Participation)
- [ ] Ledger storage and indexing
- [ ] Chain synchronization
- [ ] Entry types implementation
  - Discovery entries
  - Test result entries
  - Message confirmation entries
  - Key exchange entries
- [ ] Ledger query API

### 4.2 Advanced Routing

- [ ] Geographic routing
  - Location-based path selection
  - Proximity calculations
- [ ] Multi-path routing
  - Parallel transmission
  - Path diversity
- [ ] Adaptive routing
  - Dynamic path updates
  - Traffic-aware routing
- [ ] Quality of Service (QoS)
  - Priority-based scheduling
  - Bandwidth reservation

### 4.3 Terminal User Interface (TUI)

- [ ] Curses-based interface
- [ ] Dashboard view
- [ ] Message management
- [ ] Configuration editor
- [ ] Log viewer
- [ ] Keyboard navigation

### 4.4 Android Application

- [ ] Android project setup
- [ ] MyriadNode port to Android
- [ ] Native UI implementation
  - Dashboard
  - Settings
  - Message interface
- [ ] Background service
- [ ] Android network adapter integration
  - WiFi
  - Bluetooth
  - Cellular
- [ ] Notifications
- [ ] Battery optimization

### 4.5 i2p Integration

- [ ] i2p client integration
- [ ] SAM bridge interface
- [ ] Tunnel management
- [ ] i2p addressing
- [ ] Privacy-preserving routing

### 4.6 Coordinated Update Scheduling

- [ ] Update schedule protocol
  - Neighbor notification messages
  - Acknowledgment/reschedule responses
  - Signature verification
- [ ] Optimal update window selection
  - Off-peak hour preference
  - Network load analysis
  - Conflict avoidance
  - Fallback adapter verification
- [ ] Update execution coordination
  - Pre-update notifications
  - Fallback path establishment
  - Post-update verification
- [ ] Update scheduling UI
  - Manual scheduling interface
  - Automatic scheduling with user approval
  - Maintenance window configuration

### 4.7 Peer-Assisted Update Distribution

- [ ] Update package structure
  - Payload with hash verification
  - Multi-signature support
  - CVE fix metadata
  - Changelog and compatibility info
- [ ] Multi-signature verification
  - Require 3+ trusted peer signatures
  - Reputation-based trust filtering
  - Signature chain tracking
- [ ] Verification period implementation
  - 6-hour verification window for peer updates
  - Critical CVE priority override
  - Manual approval option
- [ ] Update forwarding protocol
  - Automatic forwarding to trusted neighbors
  - Signature chain extension
  - Propagation tracking
- [ ] Health monitoring post-update
  - Baseline metric capture
  - Continuous health checks
  - Degradation detection (success rate, latency, errors)
- [ ] Automatic rollback system
  - Rollback trigger conditions
  - Previous version restoration
  - Rollback notification to peers
  - Problem version blacklisting

### Deliverables

- Functional blockchain ledger
- Geographic and multi-path routing
- Terminal UI for server management
- Android application
- i2p network integration
- Coordinated update scheduling system
- Peer-assisted secure update distribution
- Automatic health monitoring and rollback

### Technologies

- **TUI:** cursive (Rust) or blessed (Node.js)
- **Android:** Kotlin + Jetpack Compose
- **i2p:** i2pd or Java i2p router

## Phase 5: Specialized Adapters (Months 14-18)

**Goal:** Implement radio and specialized network adapters

### 5.1 LoRaWAN/Meshtastic

- [ ] LoRa hardware interfacing
  - SPI communication
  - LoRa modem configuration
- [ ] LoRaWAN protocol
  - Join procedure
  - Class A/B/C support
- [ ] Meshtastic protocol
  - Packet format
  - Mesh routing
  - Protocol translation
- [ ] Message fragmentation
- [ ] Duty cycle management

### 5.2 Wi-Fi HaLoW (802.11ah)

- [ ] 802.11ah adapter implementation
- [ ] Mesh networking support
- [ ] Power save mode
- [ ] Target Wake Time (TWT)

### 5.3 Amateur Packet Radio (APRS)

- [ ] AX.25 protocol implementation
- [ ] TNC (Terminal Node Controller) interface
- [ ] APRS packet format
- [ ] Digipeater support
- [ ] APRS-IS gateway (optional)
- [ ] License compliance checks

### 5.4 FRS/GMRS Radio

- [ ] Serial interface to radio hardware
- [ ] AFSK modem implementation
- [ ] Digital mode encoding (FreeDV/Codec2)
- [ ] Channel management
- [ ] PTT (Push-to-Talk) control

### 5.5 CB/Shortwave Radio

- [ ] HF radio CAT control
- [ ] Digital mode support (PSK31, RTTY, FT8)
- [ ] Propagation awareness
- [ ] Frequency management
- [ ] Error correction for HF

### 5.6 Dial-up/PPPoE

- [ ] Modem control (AT commands)
- [ ] PPP implementation
- [ ] Dial-on-demand
- [ ] Connection management

### Deliverables

- Functional LoRa/Meshtastic adapter
- Working APRS adapter for ham radio
- FRS/GMRS and CB/Shortwave radio support
- Complete set of network adapters

### Technologies

- **LoRa:** SX1276/SX1262 drivers
- **TNC:** kiss protocol, direwolf
- **Modems:** minimodem, codec2, freedv
- **CAT Control:** hamlib

## Phase 6: Production Readiness (Months 18-24)

**Goal:** Harden system for production use, optimize, and document

### 6.1 Security Audit

- [ ] Professional security audit
- [ ] Cryptographic implementation review
- [ ] Penetration testing
- [ ] Vulnerability assessment
- [ ] Address findings
- [ ] Security documentation

### 6.2 Performance Optimization

- [ ] Profiling and benchmarking
- [ ] Memory optimization
- [ ] CPU optimization
- [ ] Network efficiency
  - Message batching
  - Compression
  - Protocol optimization
- [ ] Database optimization
- [ ] Caching strategies

### 6.3 Reliability & Robustness

- [ ] Error handling review
- [ ] Edge case testing
- [ ] Fault injection testing
- [ ] Long-running stability tests
- [ ] Recovery mechanisms
- [ ] Graceful degradation

### 6.4 Documentation

- [ ] User documentation
  - Installation guides
  - Configuration guide
  - User manual
  - Troubleshooting guide
- [ ] Administrator documentation
  - Deployment guide
  - Security best practices
  - Monitoring and maintenance
- [ ] Developer documentation
  - API reference
  - Architecture deep-dive
  - Contributing guide
  - Adapter development guide
- [ ] Video tutorials

### 6.5 Deployment Tools

- [ ] Installation packages
  - Debian/Ubuntu packages
  - RPM packages
  - Windows installer
  - macOS package
  - Android APK
- [ ] Docker images
- [ ] Kubernetes manifests
- [ ] Ansible playbooks
- [ ] Terraform modules

### 6.6 Community & Ecosystem

- [ ] Bootstrap node network
- [ ] Public test network
- [ ] Website and landing page
- [ ] Community forum
- [ ] Discord/Slack community
- [ ] Social media presence
- [ ] Example applications
- [ ] Plugin/extension API

### Deliverables

- Production-ready, audited software
- Complete documentation
- Easy deployment options
- Active community
- Public infrastructure

### Technologies

- **Packaging:** cargo-deb, NSIS, WiX, Flatpak
- **Containers:** Docker, Podman
- **Orchestration:** Kubernetes, Docker Compose
- **IaC:** Terraform, Ansible

## Ongoing Activities

Throughout all phases:

### Quality Assurance
- Continuous integration testing
- Code review for all changes
- Regular security updates
- Dependency auditing (cargo-audit, cargo-deny)
- CVE scanning and tracking
- Component version monitoring
- Automated security advisory checks

### Documentation
- Keep documentation up to date
- API changelog maintenance
- Release notes

### Community
- Respond to issues and PRs
- Community support
- Regular status updates
- Blogging about development

## Success Metrics

### Phase 1-2
- [ ] Test coverage >80%
- [ ] DHT lookup latency <500ms
- [ ] Crypto operations meet performance targets

### Phase 3-4
- [x] Support 4+ network adapter types (Ethernet, BT Classic, BLE, Cellular)
- [x] Channel-based transport architecture
- [x] Hot-reloadable adapters with zero downtime
- [ ] Message delivery success rate >95%
- [ ] Web UI usable on mobile devices
- [x] Component version tracking operational
- [ ] <95% of nodes with current adapter versions

### Phase 5-6
- [ ] Support 10+ network adapter types
- [ ] Successfully relay messages across 5+ hops
- [ ] Zero critical security vulnerabilities
- [ ] Handle 1000+ messages per second per node
- [ ] <100ms latency for direct connections
- [ ] Active user community with 100+ deployments
- [ ] Coordinated updates with <5 minute network-wide propagation
- [ ] <1% failed updates (with automatic rollback)
- [ ] >90% of nodes automatically update within 7 days of security release

## Risk Mitigation

### Technical Risks

**Risk:** Cryptographic implementation flaws
- **Mitigation:** Use well-tested libraries (libsodium), professional audit

**Risk:** DHT scalability issues
- **Mitigation:** Regular performance testing, load testing

**Risk:** Hardware unavailability for radio adapters
- **Mitigation:** Emulation/simulation for development, community hardware donations

**Risk:** Complex multi-threading bugs
- **Mitigation:** Extensive testing, use safe concurrency primitives (Rust)

**Risk:** Outdated or vulnerable dependencies
- **Mitigation:**
  - Adaptive security system with version tracking
  - Reputation penalties for outdated components
  - Hot-reloadable adapters for zero-downtime updates
  - Peer-assisted update distribution
  - Automated CVE scanning (cargo-audit)
  - Health monitoring and automatic rollback

**Risk:** Supply chain attacks on dependencies
- **Mitigation:**
  - Multi-signature verification for peer-distributed updates
  - 6-hour verification period before auto-installation
  - Reputation-based trust filtering
  - Component version manifest signing
  - Dependency vendoring option

### Schedule Risks

**Risk:** Scope creep
- **Mitigation:** Strict phase boundaries, MVP approach

**Risk:** Dependency on external libraries
- **Mitigation:** Early integration, contribute fixes upstream

### Resource Risks

**Risk:** Limited developer availability
- **Mitigation:** Clear documentation, modular design for parallel work

**Risk:** Hardware costs for testing
- **Mitigation:** Virtual testing where possible, community resource sharing

## Adaptation Strategy

This roadmap is a living document and will be adapted based on:
- Community feedback and priorities
- Technical discoveries during development
- Resource availability
- Emerging technologies and standards

Regular roadmap reviews will occur at the end of each phase.

## Contributing

We welcome contributions at any phase! See specific phase documentation for:
- Current phase goals
- Open issues and tasks
- Contribution guidelines
- Getting started guide

## Contact

- **GitHub:** [Project Repository]
- **Email:** dev@myriadmesh.org
- **Chat:** [Discord/Matrix link]
- **Forum:** [Community forum]

## Appendix: Technology Stack Summary

### Core Implementation
- **Language:** Rust (performance, safety, concurrency)
- **Async Runtime:** tokio
- **Serialization:** bincode / protobuf
- **Database:** SQLite / RocksDB

### Cryptography
- **Library:** libsodium (NaCl)
- **Algorithms:** Ed25519, X25519, XSalsa20-Poly1305, BLAKE2b

### Web Stack
- **Backend:** actix-web / axum (Rust)
- **Frontend:** React + TypeScript
- **WebSocket:** tokio-tungstenite

### Mobile
- **Android:** Kotlin + Jetpack Compose
- **iOS:** Swift + SwiftUI (future)

### Radio
- **LoRa:** SX127x/SX126x drivers
- **APRS:** kiss protocol, direwolf
- **Digital Modes:** codec2, freedv, fldigi

### DevOps
- **CI/CD:** GitHub Actions
- **Containers:** Docker
- **Orchestration:** Kubernetes
- **Monitoring:** Prometheus + Grafana
- **Security Auditing:** cargo-audit, cargo-deny
- **Dependency Management:** cargo-vendor, Dependabot

### Adaptive Security
- **Version Tracking:** Custom implementation
- **CVE Database:** RustSec Advisory Database
- **Update Distribution:** Multi-signature verification
- **Health Monitoring:** Metric-based rollback detection

## Design Documentation References

For detailed technical specifications, see:

- **Adaptive Security & Updates:** `docs/design/adaptive-security-updates.md`
  - Component version tracking with reputation impact
  - Hot-reloadable adapter architecture
  - Coordinated update scheduling protocol
  - Peer-assisted secure update distribution
  - Health monitoring and automatic rollback

- **Protocol Specification:** `docs/protocol/specification.md`
  - Message frame structure
  - Encryption and signing
  - Message types and routing

- **Network Adapters:** `docs/protocol/network-adapters.md`
  - Adapter specifications and capabilities
  - Selection strategies
  - Platform integration guides

- **Security Review:** `SECURITY_README.md`
  - Security audit findings
  - Mitigation strategies
  - Production roadmap

## Next Steps

1. Review this roadmap with stakeholders
2. Set up development environment
3. Begin Phase 1 implementation
4. Establish regular progress check-ins
5. Build community around the project

Let's build the future of resilient communication! 🚀
