# MyriadMesh: Market-Ready TODO List

**Generated:** 2025-11-16
**Version:** 0.1.0
**Current Status:** Pre-Alpha (Foundation Complete, Core Features In Progress)
**Target:** Production-Ready Beta Launch

---

## Executive Summary

This document provides a comprehensive, actionable roadmap to take MyriadMesh from its current pre-alpha state to market-ready production status. The project has **excellent foundational security** (all critical/high security issues resolved) and **well-architected core systems**, but requires **significant feature implementation** to achieve basic functionality.

### Current State
- **Security Posture:** Excellent (7/7 Critical, 12/12 High, 4/9 Medium issues fixed)
- **Test Coverage:** 363+ passing tests (unit tests solid, integration tests needed)
- **Code Quality:** High (clippy/rustfmt passing, comprehensive documentation)
- **Implementation Completeness:** ~35-40% (foundation strong, core features incomplete)
- **Production Readiness:** Not Viable (critical path blocked on routing/DHT/adapter integration)

### Critical Path to Market
1. **Complete Core Message Routing** (2-3 weeks) - BLOCKER
2. **Implement DHT Query Logic** (2 weeks) - BLOCKER
3. **Finish Network Adapter Integration** (3-4 weeks) - BLOCKER
4. **End-to-End Integration Testing** (2 weeks)
5. **Production Hardening & Deployment** (2-3 weeks)
6. **Beta Launch Preparation** (1-2 weeks)

**Total Estimated Timeline:** 12-16 weeks (3-4 months)
**Recommended Team Size:** 2-3 senior engineers
**Risk Level:** Medium (architecture solid, execution risk only)

---

## Table of Contents

1. [TODO by Priority Level](#todo-by-priority-level)
2. [Sprint Planning (Timeline)](#sprint-planning-timeline)
3. [Work Streams](#work-streams)
4. [Complete Task Inventory](#complete-task-inventory)
5. [Risk Assessment & Mitigation](#risk-assessment--mitigation)
6. [Success Metrics](#success-metrics)

---

## TODO by Priority Level

### P0: CRITICAL - Blocks Production Launch (Est: 7-9 weeks)

These items prevent basic network functionality. The application cannot function without them.

#### P0.1: Message Routing Integration (Week 1-3)
**Est: 120 hours | Risk: HIGH if delayed**

- [ ] **Router-Node Integration** (40h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/node.rs:63`
  - Integrate Router with Node lifecycle (start/stop/shutdown)
  - Set up message confirmation callback for ledger integration
  - Wire router to adapter manager for actual transmission
  - Dependencies: None
  - Success: Node can route messages through Router component
  - Risk: Application cannot send/receive messages without this

- [ ] **Implement forward_message() Logic** (60h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-routing/src/router.rs:395-454`
  - Complete TODOs: DHT integration, path selection, adapter selection, transmission
  - Implement weighted tier system for adapter ranking by priority
  - Add retry logic with exponential backoff
  - Dependencies: P0.2 (DHT), P0.3 (Adapters)
  - Success: Messages successfully routed hop-by-hop to destination
  - Risk: Core functionality completely blocked

- [ ] **Background Queue Processor** (20h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-routing/src/router.rs:456-468`
  - Create async task to dequeue messages by priority
  - Implement retry logic using retry_count/next_retry fields
  - Handle transmission failures with fallback adapters
  - Dependencies: P0.1 (forward_message)
  - Success: Queued messages automatically sent in priority order
  - Risk: Messages sit in queue forever without processing

#### P0.2: DHT Query Operations (Week 2-4)
**Est: 80 hours | Risk: HIGH**

- [ ] **Implement iterative_find_node()** (30h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-dht/src/iterative_lookup.rs`
  - Complete IterativeLookup::execute() method
  - Add parallel query logic (alpha parameter)
  - Implement timeout and retry handling
  - Dependencies: None
  - Success: Can locate any node in network via DHT
  - Risk: Cannot discover routes to peers

- [ ] **Implement iterative_find_value()** (30h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-dht/src/iterative_lookup.rs`
  - Add value-specific lookup logic
  - Verify signatures on retrieved values (SECURITY H7)
  - Cache verified values locally
  - Dependencies: P0.2.1 (find_node)
  - Success: Can retrieve stored DHT values
  - Risk: Cannot look up node metadata/routing info

- [ ] **DHT Network RPC Layer** (20h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-dht/src/operations.rs`
  - Implement actual RPC transport for DHT operations
  - Serialize/deserialize FindNodeRequest/Response
  - Handle network timeouts and failures
  - Dependencies: P0.3 (Network adapters)
  - Success: DHT operations work across network
  - Risk: DHT queries never reach remote nodes

#### P0.3: Network Adapter Completion (Week 3-6)
**Est: 120 hours | Risk: MEDIUM**

- [ ] **UDP Adapter - Full Implementation** (40h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/ethernet.rs`
  - Implement actual send()/receive() with UDP sockets
  - Add NAT traversal (STUN/TURN integration)
  - Implement connection timeout and keep-alive
  - Add multicast for local peer discovery
  - Dependencies: None
  - Success: Can send/receive frames via UDP
  - Risk: No working transport layer

- [ ] **I2P Adapter - SAM Integration** (60h)
  - Files:
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/i2p/adapter.rs:453,462`
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/i2p/sam_client.rs:342,349,360`
  - Complete SAM v3 protocol implementation
  - Implement destination creation and management
  - Add stream handling and connection pooling
  - Handle I2P router failures gracefully
  - Un-ignore 5 integration tests (require i2p router)
  - Dependencies: None (can run in parallel)
  - Success: Can route traffic through I2P
  - Risk: No privacy-preserving transport

- [ ] **Adapter Manager - Send Integration** (20h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/manager.rs`
  - Implement send_via_adapter() method
  - Add adapter failover on transmission failure
  - Integrate with scoring system for adapter selection
  - Dependencies: P0.3.1, P0.3.2
  - Success: Router can transmit via any active adapter
  - Risk: Cannot actually send messages

#### P0.4: Peer Discovery & Bootstrap (Week 4-5)
**Est: 40 hours | Risk: MEDIUM**

- [ ] **Bootstrap Protocol** (20h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/node.rs:249`
  - Implement node.start() with bootstrap logic
  - Add hardcoded seed nodes for initial DHT join
  - Implement join protocol (announce to network)
  - Dependencies: P0.2 (DHT)
  - Success: New nodes automatically join network
  - Risk: Nodes cannot join network

- [ ] **Local Peer Discovery** (20h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/ethernet.rs`
  - Implement mDNS/multicast discovery
  - Add LAN peer announcements
  - Integrate with DHT for local peers
  - Dependencies: P0.3.1 (UDP)
  - Success: Nodes discover LAN peers automatically
  - Risk: Poor UX in local networks

#### P0.5: Heartbeat Broadcasting (Week 5-6)
**Est: 24 hours | Risk: LOW**

- [ ] **Implement Heartbeat Transmission** (16h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/heartbeat.rs:440`
  - TODO: Broadcast via all eligible adapters
  - Integrate with adapter manager for multicast
  - Add broadcast rate limiting
  - Dependencies: P0.3 (Adapters)
  - Success: Heartbeats broadcast on all networks
  - Risk: Network presence not maintained

- [ ] **Geolocation Collection** (8h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/heartbeat.rs:430`
  - TODO: Implement geolocation collection
  - Add GPS integration (optional)
  - Respect privacy settings (can disable)
  - Dependencies: None
  - Success: Geographic routing can function
  - Risk: Geographic features unavailable

---

### P1: HIGH - Should Have for Launch (Est: 4-6 weeks)

These features are essential for a complete product but don't block basic functionality.

#### P1.1: API Completeness (Week 6-7)
**Est: 60 hours | Risk: MEDIUM**

- [ ] **Message Send API - Router Integration** (16h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:317`
  - TODO: Actually route the message through the router
  - Replace mock response with real routing
  - Add proper error handling
  - Dependencies: P0.1 (Router)
  - Success: REST API can send real messages
  - Risk: API unusable for sending messages

- [ ] **DHT Status API** (12h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:518`
  - TODO: Get actual DHT node list
  - Add k-bucket statistics
  - Show routing table health
  - Dependencies: P0.2 (DHT)
  - Success: Can monitor DHT health via API

- [ ] **Node Statistics Tracking** (20h)
  - Files:
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:218` (messages_queued)
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:537` (heartbeat counts)
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:575-576` (RTT tracking)
  - Implement message queue tracking
  - Add heartbeat counters
  - Track RTT and failure rates per peer
  - Dependencies: P0.1, P0.5
  - Success: Accurate metrics in API responses

- [ ] **Config Management** (12h)
  - Files:
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:699` (get config)
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:720` (update config)
  - Implement config retrieval
  - Add hot-reload for non-critical settings
  - Validate config updates
  - Dependencies: None
  - Success: Can manage node via API

#### P1.2: Remaining Medium Security Issues (Week 7-8)
**Est: 60 hours | Risk: LOW**

- [ ] **M5: Blacklist Mechanism** (16h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-dht/src/routing_table.rs`
  - Add blacklist storage to RoutingTable
  - Implement add_to_blacklist() / remove_from_blacklist()
  - Check blacklist before adding nodes
  - Add persistence layer
  - Dependencies: None
  - Success: Can ban malicious nodes
  - Risk: Cannot defend against persistent attackers

- [ ] **M7: Comprehensive Input Validation** (24h)
  - Files: All network adapters, DHT operations, API endpoints
  - Node ID validation (64 bytes)
  - Port numbers (1-65535)
  - TTL values (1-32)
  - Payload sizes (already partially done)
  - Add validation utilities module
  - Dependencies: None
  - Success: All inputs validated
  - Risk: Crashes from malformed input

- [ ] **M8: Adapter Authentication** (16h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapter.rs`
  - Implement adapter identity verification
  - Add authentication tokens
  - Verify adapter claims (I2P destinations, etc.)
  - Prevent adapter spoofing
  - Dependencies: None
  - Success: Adapters cryptographically verified
  - Risk: Adapter impersonation attacks

- [ ] **M9: Error Message Sanitization** (4h)
  - Files: All error types across crates
  - Audit error messages for private key leakage
  - Truncate node IDs in logs (first 8 bytes only)
  - Remove internal paths from errors
  - Add production/debug error modes
  - Dependencies: None
  - Success: No sensitive info in errors
  - Risk: Information leakage

#### P1.3: Store-and-Forward (Week 8-9)
**Est: 40 hours | Risk: MEDIUM**

- [ ] **Offline Message Cache Enhancement** (24h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-routing/src/router.rs:400-403`
  - Integrate offline detection with DHT
  - Implement cache size limits per destination
  - Add expiration for old messages
  - Persist cache to disk
  - Dependencies: P0.2 (DHT)
  - Success: Messages delivered when peers come online
  - Risk: Messages lost if recipient offline

- [ ] **Message Retry Logic** (16h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-routing/src/priority_queue.rs`
  - Implement exponential backoff
  - Track retry_count per message
  - Move to offline cache after N failures
  - Add retry budget per destination
  - Dependencies: P0.1 (Router)
  - Success: Transient failures don't lose messages
  - Risk: Messages dropped on temporary failures

#### P1.4: Physical Radio Adapters (Week 9-11)
**Est: 80 hours | Risk: MEDIUM**

- [ ] **APRS Adapter Completion** (30h)
  - Files:
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/aprs.rs:242` (digipeater)
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/aprs.rs:394,406,415` (TCP)
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/aprs.rs:543` (APRS-IS)
  - Complete digipeater parsing
  - Implement TCP connection to TNC
  - Add APRS-IS protocol support
  - Integrate with license manager
  - Dependencies: P1.2.3 (M8)
  - Success: Can operate via amateur packet radio

- [ ] **Cellular Adapter Modem Integration** (30h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/cellular.rs:276`
  - TODO: Initialize cellular modem
  - Add ModemManager integration
  - Implement AT command fallback
  - Track data usage for cost management
  - Dependencies: None
  - Success: Can use cellular networks

- [ ] **LoRa/Meshtastic Full Implementation** (20h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/lora.rs`
  - Complete SPI integration for SX1262/SX1276
  - Implement Meshtastic protocol bridge
  - Add duty cycle enforcement
  - Test with mock and real hardware
  - Dependencies: None
  - Success: Long-range mesh networking works

#### P1.5: Android App Integration (Week 10-11)
**Est: 40 hours | Risk: LOW**

- [ ] **Android Node Bridge** (40h)
  - Files: All TODOs in `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-android/src/node.rs`
    - Line 12: Add actual MyriadNode instance
    - Line 42: Initialize and start node
    - Line 59: Stop node properly
    - Line 79: Send message through node
    - Line 88: Get actual node ID
    - Line 95: Get actual status
  - Create JNI bindings to Rust node
  - Implement Android lifecycle integration
  - Add background service support
  - Handle Android permissions
  - Dependencies: P0.1 (Node operational)
  - Success: Android app can run full node
  - Risk: No mobile presence

---

### P2: MEDIUM - Nice to Have for Launch (Est: 3-4 weeks)

Important features that enhance the product but aren't critical for initial launch.

#### P2.1: Monitoring & Observability (Week 11-12)
**Est: 60 hours | Risk: LOW**

- [ ] **Network Performance Testing** (24h)
  - Files:
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/monitor.rs:237` (throughput test)
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/monitor.rs:289` (packet loss test)
  - Implement actual throughput testing
  - Add packet loss measurement
  - Track historical performance
  - Dependencies: P0.3 (Adapters)
  - Success: Accurate adapter performance metrics

- [ ] **Structured Logging** (16h)
  - Files: All modules
  - Replace println! with structured tracing
  - Add contextual fields to log events
  - Implement log levels by module
  - Add JSON output option
  - Dependencies: None
  - Success: Production-grade logging

- [ ] **Metrics Collection** (20h)
  - Add Prometheus exporter
  - Track message counts, latency, errors
  - Add DHT metrics
  - Track adapter health
  - Dependencies: P0.1, P0.2
  - Success: Can monitor production nodes

#### P2.2: I2P Tunnel Management (Week 12-13)
**Est: 40 hours | Risk: LOW**

- [ ] **Tunnel Information API** (16h)
  - Files:
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:784` (destination)
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:804` (tunnel info)
    - `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:764-765` (tunnel/peer counts)
  - Get actual I2P destination from adapter
  - Retrieve tunnel information
  - Track active tunnels and peers
  - Dependencies: P0.3.2 (I2P adapter)
  - Success: I2P status visible in UI

- [ ] **I2P Tunnel Health Monitoring** (24h)
  - Add tunnel quality metrics
  - Implement tunnel rebuild triggers
  - Track tunnel latency and reliability
  - Dependencies: P2.2.1
  - Success: Maintain healthy I2P connectivity

#### P2.3: Hot Reload & Updates (Week 13)
**Est: 32 hours | Risk: LOW**

- [ ] **Binary Preservation** (16h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/reload.rs:453,467,531`
  - Implement binary preservation on restart
  - Add rollback on failed update
  - Clean up old binaries
  - Dependencies: None
  - Success: Zero-downtime updates

- [ ] **Update Checking** (16h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs:1275`
  - Implement update checking logic
  - Add signature verification
  - Integrate with UpdateCoordinator
  - Dependencies: None
  - Success: Automatic update discovery

#### P2.4: Advanced Routing Features (Week 13-14)
**Est: 60 hours | Risk: MEDIUM**

- [ ] **Geographic Routing** (24h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-routing/src/geographic.rs`
  - Integrate with heartbeat geolocation
  - Implement greedy forwarding
  - Add perimeter routing for holes
  - Dependencies: P0.5.2 (Geolocation)
  - Success: Efficient routing in sparse networks

- [ ] **Multipath Routing** (24h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-routing/src/multipath.rs`
  - Implement parallel path discovery
  - Add load balancing across paths
  - Track path reliability
  - Dependencies: P0.1, P0.2
  - Success: Redundant routing for reliability

- [ ] **QoS Implementation** (12h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-routing/src/qos.rs`
  - Implement priority-based scheduling
  - Add bandwidth reservation
  - Track SLA compliance
  - Dependencies: P0.1
  - Success: Emergency messages prioritized

---

### P3: LOW - Post-Launch Enhancements (Est: 4+ weeks)

Features that can wait until after initial launch.

#### P3.1: Ledger Integration (Post-Launch)
**Est: 80 hours | Risk: LOW**

- [ ] **Router-Ledger Callback** (24h)
  - File: `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/node.rs:63-68`
  - Implement confirmation callback
  - Create MESSAGE ledger entries
  - Track delivery confirmations
  - Dependencies: P0.1 (Router)
  - Success: Immutable delivery audit trail

- [ ] **Ledger Synchronization** (40h)
  - Implement chain sync protocol
  - Add block propagation
  - Resolve forks
  - Dependencies: P0.2 (DHT)
  - Success: Distributed consensus on events

- [ ] **Ledger Query API** (16h)
  - Add block explorer endpoints
  - Query by node/message/time
  - Show reputation history
  - Dependencies: P3.1.2
  - Success: Can audit network history

#### P3.2: Advanced Security (Post-Launch)
**Est: 60 hours | Risk: MEDIUM**

- [ ] **M3: DHT Query Encryption** (40h)
  - File: Design doc needed
  - Design encrypted query protocol
  - Implement onion routing for DHT
  - Add PIR (Private Information Retrieval)
  - Dependencies: P0.2
  - Success: Metadata-resistant DHT
  - Risk: Architectural complexity

- [ ] **Forward Secrecy** (20h)
  - Implement session rekeying
  - Add ephemeral key exchange
  - Rotate keys on schedule
  - Dependencies: None
  - Success: Past messages safe if key compromised

#### P3.3: Additional Physical Layers (Post-Launch)
**Est: 120+ hours | Risk: MEDIUM**

- [ ] **Bluetooth Classic/LE** (40h each)
  - Complete adapter implementations
  - Add device discovery
  - Implement mesh networking (BLE)
  - Test with real hardware

- [ ] **Dial-up/PPPoE** (40h)
  - Complete modem integration
  - Add connection management
  - Implement carrier detection

- [ ] **Wi-Fi HaLow** (40h)
  - Complete 802.11ah integration
  - Add long-range mesh support
  - Test with HaLow hardware

#### P3.4: Platform Support (Post-Launch)
**Est: 80 hours | Risk: MEDIUM**

- [ ] **iOS App** (60h)
  - Port Android bindings to iOS
  - Implement background operation
  - Handle iOS permissions
  - Dependencies: P1.5 (Android complete)

- [ ] **Windows Support** (20h)
  - Test on Windows
  - Fix platform-specific issues
  - Create installers

---

## Sprint Planning (Timeline)

### Sprint 1: Core Routing (Weeks 1-3)
**Goal:** Messages can be routed end-to-end

| Week | Focus | Tasks | Hours | Team |
|------|-------|-------|-------|------|
| 1 | Router Integration | P0.1.1, P0.1.3 | 60h | 2 engineers |
| 2 | DHT Queries | P0.2.1, P0.2.2 | 60h | 2 engineers |
| 3 | Message Forwarding | P0.1.2 + Integration | 60h | 2 engineers |

**Deliverable:** Demo of message routing across 3+ nodes

### Sprint 2: Network Layer (Weeks 4-6)
**Goal:** UDP and I2P adapters operational

| Week | Focus | Tasks | Hours | Team |
|------|-------|-------|-------|------|
| 4 | UDP Adapter | P0.3.1, P0.4.2 | 60h | 2 engineers |
| 5 | I2P Adapter | P0.3.2 | 60h | 2 engineers |
| 6 | Integration | P0.3.3, P0.4.1, P0.5 | 60h | 2 engineers |

**Deliverable:** Two nodes communicating via UDP and I2P

### Sprint 3: API & Features (Weeks 7-9)
**Goal:** Complete API, security hardening

| Week | Focus | Tasks | Hours | Team |
|------|-------|-------|-------|------|
| 7 | API Completion | P1.1 (all) | 60h | 2 engineers |
| 8 | Security | P1.2 (all) | 60h | 2 engineers |
| 9 | Store-and-Forward | P1.3 (all) | 40h | 1-2 engineers |

**Deliverable:** Full-featured API with complete security

### Sprint 4: Hardware Integration (Weeks 10-11)
**Goal:** Physical radio support, mobile app

| Week | Focus | Tasks | Hours | Team |
|------|-------|-------|-------|------|
| 10 | Radio Adapters | P1.4.1, P1.4.2 | 60h | 2 engineers |
| 11 | Android/LoRa | P1.4.3, P1.5 | 60h | 2 engineers |

**Deliverable:** Working on Raspberry Pi with radio hardware

### Sprint 5: Production Hardening (Weeks 12-14)
**Goal:** Production-ready with monitoring

| Week | Focus | Tasks | Hours | Team |
|------|-------|-------|-------|------|
| 12 | Monitoring | P2.1 (all) | 60h | 2 engineers |
| 13 | Advanced Features | P2.2, P2.3, P2.4.1 | 60h | 2 engineers |
| 14 | Polish & Testing | Integration tests, bug fixes | 60h | 2 engineers |

**Deliverable:** Beta-ready release candidate

### Sprint 6: Beta Launch Prep (Weeks 15-16)
**Goal:** Documentation, deployment, launch

| Week | Focus | Tasks | Hours | Team |
|------|-------|-------|-------|------|
| 15 | Docs & Deploy | Deployment guide, user docs | 40h | 2 engineers |
| 16 | Testing & Launch | Load testing, beta deployment | 40h | 2 engineers |

**Deliverable:** Public beta launch

---

## Work Streams

### Stream 1: Core API Implementation

**Owner:** Senior Backend Engineer
**Duration:** 8 weeks
**Dependencies:** None (can start immediately)

#### Phase 1: Message Routing (Weeks 1-3)
- P0.1.1: Router-Node Integration
- P0.1.2: forward_message() Logic
- P0.1.3: Background Queue Processor
- **Milestone:** Messages route through network

#### Phase 2: API Endpoints (Weeks 7-8)
- P1.1.1: Message Send API
- P1.1.2: DHT Status API
- P1.1.3: Statistics Tracking
- P1.1.4: Config Management
- **Milestone:** Complete REST API

#### Phase 3: Advanced Routing (Weeks 13-14)
- P2.4.1: Geographic Routing
- P2.4.2: Multipath Routing
- P2.4.3: QoS Implementation
- **Milestone:** Production-grade routing

### Stream 2: Network & Integration

**Owner:** Senior Systems Engineer
**Duration:** 9 weeks
**Dependencies:** Partial (DHT blocks some tasks)

#### Phase 1: DHT Operations (Weeks 2-4)
- P0.2.1: iterative_find_node()
- P0.2.2: iterative_find_value()
- P0.2.3: DHT RPC Layer
- **Milestone:** Working DHT

#### Phase 2: Network Adapters (Weeks 4-6)
- P0.3.1: UDP Adapter
- P0.3.2: I2P Adapter
- P0.3.3: Adapter Manager
- **Milestone:** Multi-transport messaging

#### Phase 3: Physical Radios (Weeks 9-11)
- P1.4.1: APRS Adapter
- P1.4.2: Cellular Adapter
- P1.4.3: LoRa/Meshtastic
- **Milestone:** Hardware integration

### Stream 3: Testing & Quality Assurance

**Owner:** QA Engineer (can be shared role)
**Duration:** Ongoing (Weeks 1-16)

#### Phase 1: Integration Tests (Weeks 3-5)
- Multi-node routing tests
- DHT query tests
- Adapter failover tests
- **Milestone:** Automated integration suite

#### Phase 2: Load Testing (Weeks 8-10)
- Message throughput benchmarks
- DHT query performance
- Concurrent connection limits
- **Milestone:** Performance baseline

#### Phase 3: Security Testing (Weeks 11-13)
- Fuzzing all parsers
- Penetration testing
- Dependency audit
- **Milestone:** Security sign-off

### Stream 4: Documentation & Deployment

**Owner:** DevOps Engineer (can be shared role)
**Duration:** 4 weeks (Weeks 13-16)

#### Phase 1: Documentation (Weeks 13-14)
- API documentation (rustdoc complete)
- Deployment guide
- Configuration reference
- Troubleshooting guide
- **Milestone:** Complete docs

#### Phase 2: Deployment (Weeks 15-16)
- Docker images
- Kubernetes manifests
- Systemd service files
- Raspberry Pi image
- **Milestone:** Automated deployment

### Stream 5: Platform Integration

**Owner:** Mobile Engineer (part-time)
**Duration:** 2 weeks (Weeks 10-11)

#### Android App (Weeks 10-11)
- P1.5: Android Node Bridge
- UI integration
- Background service
- Permissions handling
- **Milestone:** Android beta app

---

## Complete Task Inventory

### Core Protocol (37 TODOs catalogued)

#### Router Integration (6 TODOs)
1. ✅ **Catalogued** - `/crates/myriadnode/src/node.rs:63` - Ledger callback setup
2. ✅ **Catalogued** - `/crates/myriadmesh-routing/src/router.rs:395` - DHT Integration
3. ✅ **Catalogued** - `/crates/myriadmesh-routing/src/router.rs:405` - Path Selection
4. ✅ **Catalogued** - `/crates/myriadmesh-routing/src/router.rs:427` - Adapter Selection
5. ✅ **Catalogued** - `/crates/myriadmesh-routing/src/router.rs:439` - Actual Transmission
6. ✅ **Catalogued** - `/crates/myriadmesh-routing/src/router.rs:456` - Queue Processor

#### Network Adapters (18 TODOs)
7. ✅ **Catalogued** - `/crates/myriadmesh-network/src/adapters/aprs.rs:242` - Digipeater decode
8. ✅ **Catalogued** - `/crates/myriadmesh-network/src/adapters/aprs.rs:394` - TCP connection
9. ✅ **Catalogued** - `/crates/myriadmesh-network/src/adapters/aprs.rs:406` - TCP send
10. ✅ **Catalogued** - `/crates/myriadmesh-network/src/adapters/aprs.rs:415` - TCP receive
11. ✅ **Catalogued** - `/crates/myriadmesh-network/src/adapters/aprs.rs:543` - APRS-IS parse
12. ✅ **Catalogued** - `/crates/myriadmesh-network/src/adapters/cellular.rs:276` - Modem init
13. ✅ **Catalogued** - `/crates/myriadmesh-network/src/reload.rs:453` - Binary preserve
14. ✅ **Catalogued** - `/crates/myriadmesh-network/src/reload.rs:467` - Binary cleanup
15. ✅ **Catalogued** - `/crates/myriadmesh-network/src/reload.rs:531` - Binary cleanup (duplicate)

#### MyriadNode APIs (13 TODOs)
16. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:218` - Message queue tracking
17. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:317` - Router integration
18. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:518` - DHT node list
19. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:537` - Heartbeat counts
20. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:575` - RTT tracking
21. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:576` - Failure tracking
22. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:699` - Get config
23. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:720` - Update config
24. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:764` - Tunnel count
25. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:765` - Peer count
26. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:784` - I2P destination
27. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:804` - Tunnel info
28. ✅ **Catalogued** - `/crates/myriadnode/src/api.rs:1275` - Update checking

#### Heartbeat Service (2 TODOs)
29. ✅ **Catalogued** - `/crates/myriadnode/src/heartbeat.rs:430` - Geolocation
30. ✅ **Catalogued** - `/crates/myriadnode/src/heartbeat.rs:440` - Broadcast

#### Network Monitor (2 TODOs)
31. ✅ **Catalogued** - `/crates/myriadnode/src/monitor.rs:237` - Throughput test
32. ✅ **Catalogued** - `/crates/myriadnode/src/monitor.rs:289` - Packet loss test

#### Android App (6 TODOs)
33. ✅ **Catalogued** - `/crates/myriadmesh-android/src/node.rs:12` - MyriadNode instance
34. ✅ **Catalogued** - `/crates/myriadmesh-android/src/node.rs:42` - Initialize node
35. ✅ **Catalogued** - `/crates/myriadmesh-android/src/node.rs:59` - Stop node
36. ✅ **Catalogued** - `/crates/myriadmesh-android/src/node.rs:79` - Send message
37. ✅ **Catalogued** - `/crates/myriadmesh-android/src/node.rs:88` - Get node ID
38. ✅ **Catalogued** - `/crates/myriadmesh-android/src/node.rs:95` - Get status

### Ignored Tests (5 tests require I2P router)

1. `/crates/myriadmesh-network/src/i2p/adapter.rs:453` - I2P adapter test
2. `/crates/myriadmesh-network/src/i2p/adapter.rs:462` - I2P adapter test
3. `/crates/myriadmesh-network/src/i2p/sam_client.rs:342` - SAM client test
4. `/crates/myriadmesh-network/src/i2p/sam_client.rs:349` - SAM client test
5. `/crates/myriadmesh-network/src/i2p/sam_client.rs:360` - SAM client test

**Action:** Un-ignore these after P0.3.2 (I2P Adapter) complete. Run against local I2P router in CI.

---

## Risk Assessment & Mitigation

### Critical Path Analysis

**Longest Dependency Chain:** 10 weeks
1. Router Integration (Week 1-3) →
2. DHT Queries (Week 2-4) →
3. UDP Adapter (Week 4-5) →
4. Integration Testing (Week 6) →
5. API Completion (Week 7-8) →
6. Security Hardening (Week 8) →
7. Production Testing (Week 12-14)

**Critical Path Items:**
- P0.1.2: forward_message() - Blocks all message routing
- P0.2.1: iterative_find_node() - Blocks DHT functionality
- P0.3.1: UDP Adapter - Blocks basic networking
- P0.3.3: Adapter Manager - Blocks transmission

### High-Risk Tasks

| Task | Risk | Impact | Mitigation |
|------|------|--------|------------|
| P0.1.2: Message Forwarding | HIGH | CRITICAL | Prototype in Week 1, daily progress reviews |
| P0.2.3: DHT RPC Layer | MEDIUM | HIGH | Reuse existing RPC libraries, don't build from scratch |
| P0.3.2: I2P Integration | MEDIUM | MEDIUM | Use established SAMv3 libraries, fallback to basic mode |
| P1.4: Radio Hardware | HIGH | MEDIUM | Mock interfaces for testing, real hardware optional for beta |
| P2.4: Advanced Routing | MEDIUM | LOW | Can ship with basic routing, add post-launch |

### Risk Mitigation Strategies

#### Technical Risks

**Risk:** Router integration more complex than estimated
**Probability:** 40%
**Impact:** 1-2 week delay
**Mitigation:**
- Create minimal working prototype in Week 1
- Daily standup to identify blockers early
- Have contingency: simplify to basic flooding if needed

**Risk:** DHT queries don't scale under load
**Probability:** 30%
**Impact:** Performance degradation
**Mitigation:**
- Load test early (Week 5)
- Implement query caching
- Add query rate limiting
- Document scale limits for beta

**Risk:** I2P SAM integration unstable
**Probability:** 25%
**Impact:** Privacy features delayed
**Mitigation:**
- I2P is P0 but can ship beta with UDP only
- Extensive testing with various I2P router versions
- Fallback to direct TCP if SAM fails

**Risk:** Radio hardware unavailable for testing
**Probability:** 60%
**Impact:** Cannot verify hardware adapters
**Mitigation:**
- Use mock adapters for development
- Ship with "experimental" label for hardware
- Partner with amateur radio community for testing

#### Schedule Risks

**Risk:** Development takes longer than 16 weeks
**Probability:** 50%
**Impact:** Delayed launch
**Mitigation:**
- Build 2-week buffer into schedule
- Define MVP scope: can ship without P2/P3 features
- Prioritize ruthlessly: cut P2 features if needed

**Risk:** Key engineer leaves mid-project
**Probability:** 15%
**Impact:** 2-4 week delay
**Mitigation:**
- Document as we go (not at end)
- Pair program on critical components
- Code reviews ensure knowledge sharing

#### Quality Risks

**Risk:** Security vulnerability discovered pre-launch
**Probability:** 20%
**Impact:** Launch delay
**Mitigation:**
- Weekly security reviews
- External audit in Week 12
- Bug bounty program at launch

**Risk:** Performance below expectations
**Probability:** 35%
**Impact:** Poor UX
**Mitigation:**
- Continuous benchmarking
- Load testing in Week 8
- Set realistic expectations for beta

---

## Success Metrics

### P0 Completion (Launch Blockers)

**Definition of Done:** All P0 tasks complete, tests passing

| Metric | Target | Measurement |
|--------|--------|-------------|
| P0 tasks complete | 100% | Task checklist |
| Integration tests | >90% pass | CI/CD |
| Message delivery success rate | >95% | End-to-end test |
| DHT lookup success | >99% | Integration test |
| Adapter send/receive | 100% functional | Unit tests |

### P1 Completion (Beta Quality)

**Definition of Done:** Feature-complete for beta launch

| Metric | Target | Measurement |
|--------|--------|-------------|
| P1 tasks complete | >80% | Task checklist |
| API coverage | 100% | All endpoints implemented |
| Security issues | 0 critical/high | Security audit |
| Physical adapter support | ≥2 working | Hardware testing |

### Performance Benchmarks

| Metric | Minimum | Target | Measurement |
|--------|---------|--------|-------------|
| Message latency (UDP) | <500ms | <100ms | End-to-end test |
| Message latency (I2P) | <5s | <2s | End-to-end test |
| DHT query time | <2s | <1s | Benchmark |
| Messages/sec (single node) | 100 | 1000 | Load test |
| Concurrent connections | 100 | 1000 | Load test |
| Memory usage (idle) | <100MB | <50MB | Monitoring |
| CPU usage (idle) | <5% | <2% | Monitoring |

### Quality Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Test coverage (lines) | >70% | cargo tarpaulin |
| Test coverage (critical paths) | >95% | Manual review |
| Clippy warnings | 0 | CI/CD |
| Documentation coverage | >80% | cargo doc |
| Security scan issues | 0 high/critical | cargo audit |

### Launch Readiness Checklist

**Beta Launch Criteria:**

- [ ] All P0 tasks complete (100%)
- [ ] >80% P1 tasks complete
- [ ] Security audit passed (0 critical/high issues)
- [ ] Load testing complete (meets minimum benchmarks)
- [ ] Documentation complete (API docs, deployment guide)
- [ ] Deployment automation working (Docker, systemd)
- [ ] 3+ nodes running in test network for 7+ days
- [ ] End-to-end message delivery >95% success rate
- [ ] Monitoring and alerting operational
- [ ] Incident response plan documented

**Production Launch Criteria (Post-Beta):**

- [ ] All P1 tasks complete (100%)
- [ ] >50% P2 tasks complete
- [ ] Beta testing period complete (30+ days)
- [ ] Major bugs resolved from beta feedback
- [ ] Performance meets target benchmarks
- [ ] Hardware adapters tested by community
- [ ] Legal review complete (FCC compliance for radio)
- [ ] Privacy audit complete (GDPR considerations)

---

## Resource Requirements

### Team Composition

**Minimum Team:**
- 1 Senior Backend Engineer (Rust expert)
- 1 Senior Systems Engineer (networking/hardware)
- 0.5 QA Engineer (part-time, can be developer)
- 0.5 DevOps Engineer (part-time, can be developer)

**Total:** 3 FTE (Full-Time Equivalents)

**Optimal Team:**
- 1 Tech Lead / Architect
- 2 Senior Backend Engineers
- 1 Systems Engineer
- 1 Mobile Engineer (part-time)
- 0.5 QA Engineer
- 0.5 DevOps Engineer

**Total:** 5.5 FTE

### Skill Requirements

**Must Have:**
- Expert Rust (async/await, tokio, low-level networking)
- Distributed systems (DHT, P2P protocols)
- Network programming (UDP, TCP, routing)
- Cryptography basics (understand sodiumoxide APIs)

**Nice to Have:**
- Amateur radio license (for APRS/HF testing)
- I2P/Tor protocol knowledge
- Android development (JNI, FFI)
- Embedded systems (Raspberry Pi, SPI)
- DevOps (Docker, Kubernetes)

### Infrastructure

**Development:**
- 3-5 development machines
- 3+ test nodes (Raspberry Pi 4 recommended)
- 1 I2P router (for testing)
- Radio hardware (APRS TNC, LoRa module) - optional

**Testing:**
- CI/CD server (GitHub Actions sufficient)
- 10+ virtual test nodes
- Load testing infrastructure (can use cloud)

**Production (Beta):**
- 3-5 seed nodes (global distribution)
- Monitoring infrastructure (Prometheus + Grafana)
- Update distribution server
- Documentation hosting (GitHub Pages sufficient)

### Budget Estimate

**Development (16 weeks):**
- Engineering: 3 FTE × 16 weeks × $3000/week = $144,000
- Infrastructure: $2,000 (hardware + cloud)
- Tools/Services: $1,000 (GitHub, monitoring)

**Total:** ~$150,000 for beta launch

**Ongoing (Production):**
- Engineering: 1-2 FTE for maintenance
- Infrastructure: $500-1000/month (seed nodes, monitoring)
- Security audits: $10,000/year

---

## Dependencies & Prerequisites

### External Dependencies

**Required:**
- libsodium (cryptography library) - ✅ Already integrated
- SQLite (local storage) - ✅ Already integrated
- tokio runtime - ✅ Already integrated

**Optional (for specific features):**
- I2P router (for I2P adapter testing)
- ModemManager (for cellular support)
- Android SDK (for mobile app)

### Hardware Dependencies

**Development:**
- Any Linux/macOS machine (no special requirements)

**Full Testing:**
- Raspberry Pi 4 (for embedded testing)
- USB cellular modem (optional)
- LoRa module (optional, can use mock)
- APRS TNC (optional, can use mock)
- Amateur radio license (for RF transmission)

### Knowledge Dependencies

**Critical Knowledge Needed:**
1. Kademlia DHT protocol (for P0.2)
2. SAMv3 protocol (for P0.3.2)
3. APRS protocol (for P1.4.1)
4. Android JNI/FFI (for P1.5)

**Mitigation:** Documentation exists for all, team should review before starting tasks.

---

## Deployment Strategy

### Deployment Targets

**Primary:**
- Linux (Ubuntu 22.04+, Debian 11+)
- Raspberry Pi OS (64-bit)

**Secondary:**
- macOS (development/testing)
- Android 8.0+ (mobile)

**Future:**
- Windows 10+ (post-launch)
- iOS 14+ (post-launch)

### Deployment Artifacts

**Required for Launch:**

1. **Binary Packages**
   - [ ] Debian package (.deb)
   - [ ] RPM package (.rpm)
   - [ ] Statically-linked binary (x86_64, aarch64)
   - [ ] Raspberry Pi image (Raspbian-based)

2. **Container Images**
   - [ ] Docker image (multi-arch)
   - [ ] Docker Compose configuration
   - [ ] Kubernetes manifests (Helm chart)

3. **Service Files**
   - [ ] systemd service unit
   - [ ] logrotate configuration
   - [ ] default configuration file

4. **Mobile Apps**
   - [ ] Android APK (Google Play + direct download)
   - [ ] iOS app (post-launch)

### Deployment Process

**Beta Launch:**
1. Tag release (semantic versioning)
2. Build binaries for all targets
3. Sign release artifacts
4. Deploy seed nodes (5 locations)
5. Publish Docker images
6. Update documentation
7. Announce on GitHub + social media

**Production Updates:**
1. Coordinate update window with network
2. Stage update on seed nodes
3. Gradual rollout (10% → 50% → 100%)
4. Monitor for issues
5. Rollback capability maintained

---

## Testing Strategy

### Unit Tests (Existing)

**Current Coverage:** 363+ tests passing
**Quality:** Excellent (security, crypto, core logic well-tested)

**Gaps to Address:**
- [ ] Router message forwarding logic
- [ ] DHT iterative lookup
- [ ] Adapter send/receive paths

**Target:** >80% line coverage on critical paths

### Integration Tests (Needed)

**Priority 1 (Week 3-5):**
- [ ] Multi-node message routing (3+ nodes)
- [ ] DHT query across network (5+ nodes)
- [ ] Adapter failover (primary fails, switches to secondary)
- [ ] Store-and-forward (send to offline node, deliver when online)

**Priority 2 (Week 8-10):**
- [ ] Heartbeat propagation (verify all nodes see heartbeats)
- [ ] Node join/leave (bootstrap, graceful shutdown)
- [ ] Configuration reload (hot-reload without restart)
- [ ] Update propagation (simulate network update)

**Priority 3 (Week 12-14):**
- [ ] Geographic routing (nodes with locations)
- [ ] Multipath routing (parallel paths)
- [ ] I2P tunnel establishment (with real I2P router)
- [ ] APRS message transmission (with TNC or mock)

### Load Testing (Week 8-10)

**Scenarios:**
1. **Message Throughput**
   - 1000 messages/sec through single node
   - Measure latency distribution
   - Identify bottlenecks

2. **DHT Stress**
   - 10,000 concurrent lookups
   - Network with 1000+ nodes
   - Measure query success rate

3. **Connection Limits**
   - 500 concurrent peer connections
   - Measure memory/CPU usage
   - Find breaking point

4. **Long-Running Stability**
   - 7-day continuous operation
   - Monitor memory leaks
   - Check for deadlocks

### Security Testing (Week 11-13)

**Fuzzing (Week 11):**
- [ ] Protocol frame parsing
- [ ] DHT message parsing
- [ ] API input validation
- [ ] Configuration file parsing

**Penetration Testing (Week 12):**
- [ ] Attempt DHT poisoning attacks
- [ ] Try message replay attacks
- [ ] Test rate limiting bypass
- [ ] Attempt adapter spoofing

**Dependency Audit (Week 13):**
- [ ] cargo audit (automated)
- [ ] cargo deny check
- [ ] Review unmaintained dependencies
- [ ] Check for known CVEs

### Acceptance Testing (Week 14-16)

**User Scenarios:**
1. Fresh install on Raspberry Pi
2. Send message between two nodes
3. Node goes offline, comes back online
4. Add new network adapter
5. Monitor network health via TUI
6. Update node to new version

**Acceptance Criteria:**
- [ ] All scenarios complete without errors
- [ ] Documentation sufficient for non-expert
- [ ] Error messages helpful and actionable
- [ ] Performance acceptable on Raspberry Pi

---

## Documentation Requirements

### Technical Documentation

**API Documentation (Week 7):**
- [ ] Complete rustdoc for all public APIs
- [ ] Example code for common operations
- [ ] REST API reference (OpenAPI spec)
- [ ] WebSocket protocol documentation

**Architecture Documentation (Week 13):**
- [ ] System architecture diagram
- [ ] Protocol specification (update existing)
- [ ] DHT routing specification
- [ ] Adapter integration guide
- [ ] Security model documentation

**Developer Documentation (Week 14):**
- [ ] Contributing guide
- [ ] Code style guide
- [ ] Testing guide
- [ ] Release process

### User Documentation

**Getting Started (Week 15):**
- [ ] Quick start guide
- [ ] Installation instructions (all platforms)
- [ ] Configuration reference
- [ ] CLI reference

**Guides (Week 15):**
- [ ] Raspberry Pi deployment
- [ ] Android app usage
- [ ] Network troubleshooting
- [ ] Performance tuning
- [ ] Security best practices

**Operator Documentation (Week 16):**
- [ ] Deployment guide (Docker, systemd)
- [ ] Monitoring and alerting
- [ ] Backup and recovery
- [ ] Incident response
- [ ] Scaling guidelines

### Community Documentation

**Wiki (Post-Launch):**
- [ ] FAQ
- [ ] Hardware compatibility list
- [ ] Adapter setup guides (APRS, LoRa, etc.)
- [ ] Network coverage maps
- [ ] Use case examples

---

## Open Questions & Decisions Needed

### Technical Decisions

**Q1: DHT Bootstrap Strategy**
- Option A: Hardcoded seed nodes (simple, centralized)
- Option B: DNS-based discovery (more resilient)
- Option C: mDNS for local, DNS for WAN (complex)
- **Recommendation:** Start with A, add B in P2

**Q2: Message Encryption Scope**
- Option A: End-to-end only (faster, less secure in transit)
- Option B: Hop-by-hop + end-to-end (slower, more secure)
- **Recommendation:** B (already designed this way)

**Q3: Ledger Consensus for Beta**
- Option A: Full consensus (complex, slower)
- Option B: Single-authority for beta (simple, centralized)
- Option C: Delay ledger to post-launch (simplest)
- **Recommendation:** C (ledger is P3, not critical)

### Product Decisions

**Q4: Beta Launch Scope**
- Include hardware adapters? (APRS, LoRa, Cellular)
- **Recommendation:** Label as "experimental", mock-only testing OK

**Q5: Licensing**
- Current: GPL-3.0-only
- Concern: May limit commercial use
- **Recommendation:** Review with legal before launch

**Q6: FCC Compliance**
- Amateur radio features require operator license
- How to enforce? (Trust users vs. technical enforcement)
- **Recommendation:** Current license manager is good, add disclaimers

### Operational Decisions

**Q7: Seed Node Hosting**
- How many seed nodes for beta? (Recommend 5+)
- Where to host? (AWS, community hosting, both)
- **Recommendation:** 3 AWS + 2 community for redundancy

**Q8: Update Distribution**
- Automatic updates for beta? (risky)
- Manual updates only? (slower adoption)
- **Recommendation:** Opt-in automatic for beta

---

## Appendix: File Reference

### Critical Files by Work Stream

**Core Routing:**
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-routing/src/router.rs`
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-routing/src/priority_queue.rs`
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/node.rs`

**DHT:**
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-dht/src/iterative_lookup.rs`
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-dht/src/operations.rs`
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-dht/src/routing_table.rs`

**Network Adapters:**
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/ethernet.rs` (UDP)
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/i2p/adapter.rs`
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/aprs.rs`
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/cellular.rs`
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-network/src/adapters/lora.rs`

**API:**
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/api.rs`
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/heartbeat.rs`
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadnode/src/monitor.rs`

**Mobile:**
- `/home/martin/ClaudeCode/myriadmesh/crates/myriadmesh-android/src/node.rs`

### Configuration Files

- `/home/martin/ClaudeCode/myriadmesh/Cargo.toml` - Workspace definition
- `/home/martin/ClaudeCode/myriadmesh/.github/workflows/ci.yml` - CI/CD
- `/home/martin/ClaudeCode/myriadmesh/deny.toml` - Dependency policy
- `/home/martin/ClaudeCode/myriadmesh/audit.toml` - Security audit config

### Documentation Files

- `/home/martin/ClaudeCode/myriadmesh/README.md` - Project overview
- `/home/martin/ClaudeCode/myriadmesh/PRODUCTION_TODO.md` - Previous TODO (superseded)
- `/home/martin/ClaudeCode/myriadmesh/docs/protocol/specification.md` - Protocol spec
- `/home/martin/ClaudeCode/myriadmesh/docs/protocol/dht-routing.md` - DHT spec
- `/home/martin/ClaudeCode/myriadmesh/docs/guides/GETTING_STARTED_RASPBERRYPI.md` - Deployment guide

---

## Change Log

**2025-11-16:** Initial creation
- Comprehensive audit of 37 TODOs in codebase
- Analyzed 5 ignored tests requiring I2P router
- Reviewed 363+ passing tests
- Assessed current implementation status (~35-40% complete)
- Created prioritized roadmap (P0-P3)
- Defined 16-week timeline to beta launch
- Estimated effort: 900+ hours of development work

---

## Notes for Development Team

### Critical Success Factors

1. **Focus on Critical Path:** P0 tasks are non-negotiable. Everything else can be deferred if needed.

2. **Iterate Fast:** Get P0.1 (Router) working quickly, even if incomplete. Blocked on nothing.

3. **Test as You Go:** Don't save integration testing for the end. Test after each sprint.

4. **Document Continuously:** Don't leave documentation for Week 15. Write as you code.

5. **Security First:** All security issues (P1.2) must be fixed before beta launch.

### Common Pitfalls to Avoid

❌ **Don't:** Build elaborate features before basic routing works
✅ **Do:** Get messages flowing end-to-end ASAP, then enhance

❌ **Don't:** Perfect each adapter before moving on
✅ **Do:** Get UDP working, then parallelize other adapters

❌ **Don't:** Skip integration tests because unit tests pass
✅ **Do:** Multi-node testing reveals issues unit tests miss

❌ **Don't:** Assume DHT will scale without testing
✅ **Do:** Load test DHT by Week 5, adjust algorithm if needed

❌ **Don't:** Implement all P2 features before launch
✅ **Do:** Ship beta with P0+P1, add P2 based on feedback

### Development Best Practices

**Daily Routine:**
- Morning standup (15 min) - blockers, progress, plan
- Code in 2-hour focus blocks
- Afternoon code review (30 min)
- Update task checklist daily

**Weekly Routine:**
- Monday: Sprint planning (if new sprint)
- Wednesday: Mid-sprint checkpoint
- Friday: Demo working features, retrospective

**Testing Discipline:**
- Write test before fix (TDD for bugs)
- Run full test suite before commit
- Integration test every Friday
- Load test every 2 weeks starting Week 5

**Code Review Standards:**
- All P0/P1 code requires review
- Security-sensitive code requires 2 reviewers
- Performance-critical code requires benchmarks
- No review longer than 400 lines (split PRs)

---

## Conclusion

MyriadMesh is a well-architected, security-focused project with a solid foundation. The core challenge is **execution** - completing the ~38 TODOs and integrating components into a cohesive system.

With a focused team of 3-5 engineers and disciplined execution over 16 weeks, this project can reach beta-ready status. The critical path is clear: **routing → DHT → adapters → integration → hardening**.

The most important decisions are:
1. **Start immediately** on P0.1 (Router Integration)
2. **Don't get distracted** by P2/P3 features
3. **Test continuously** to catch integration issues early
4. **Ship beta** with limited scope, iterate based on feedback

**Timeline Risk:** MEDIUM (50% chance of 2-week slippage)
**Technical Risk:** LOW (architecture proven, just needs implementation)
**Market Opportunity:** HIGH (unique value proposition in privacy/resilience space)

This roadmap provides the detailed blueprint. Now it's time to execute.

---

**Document Maintainer:** Development Team
**Review Frequency:** Weekly during active development
**Next Review:** 2025-11-23
