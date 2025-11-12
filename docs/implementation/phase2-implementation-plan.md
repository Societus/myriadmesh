# Phase 2 Implementation Plan

**Status**: In Progress
**Started**: 2025-11-12
**Target Completion**: 14 weeks

## Overview

Phase 2 builds on the Phase 1 foundation to implement the core protocol: DHT, message routing, network abstraction, and the Ethernet adapter.

## Components

### 1. Protocol Updates (Week 1)

**Status**: Not Started

Update `myriadmesh-protocol` with Phase 2 features:

- [ ] Add `RoutingFlags` bitflags
- [ ] Add content tag system
- [ ] Change Priority from enum to u8 (0-255 range)
- [ ] Add DHT-specific message types
- [ ] Add routing metadata structures
- [ ] Update MessageFrame structure

**Estimated Time**: 3-5 days

### 2. DHT Implementation (Weeks 1-2)

**Status**: Not Started

Create `myriadmesh-dht` crate:

- [ ] Kademlia routing table (256 k-buckets, k=20)
- [ ] Node reputation system
- [ ] DHT operations:
  - [ ] FIND_NODE
  - [ ] STORE
  - [ ] FIND_VALUE
- [ ] DHT storage layer
- [ ] Maintenance tasks (bucket refresh, republish, health checks)
- [ ] Unit tests
- [ ] Integration tests

**Estimated Time**: 2 weeks

### 3. Message Router (Weeks 3-4)

**Status**: Not Started

Create `myriadmesh-routing` crate:

- [ ] Priority queue system (5 levels)
- [ ] Message deduplication cache
- [ ] Routing engine:
  - [ ] Direct routing
  - [ ] Multi-hop routing
  - [ ] Store-and-forward
- [ ] Relay logic with content filtering
- [ ] Rate limiting
- [ ] Unit tests
- [ ] Integration tests

**Estimated Time**: 2 weeks

### 4. Network Abstraction (Weeks 5-6)

**Status**: Not Started

Create `myriadmesh-network` crate:

- [ ] NetworkAdapter trait
- [ ] AdapterManager
- [ ] Adapter selection logic
- [ ] Performance metrics tracking
- [ ] Health monitoring
- [ ] Unit tests

**Estimated Time**: 2 weeks

### 5. Ethernet Adapter (Weeks 7-8)

**Status**: Not Started

Create `myriadmesh-adapters/ethernet` crate:

- [ ] UDP socket implementation (port 4001)
- [ ] Multicast discovery (239.255.77.77)
- [ ] IPv4/IPv6 support
- [ ] MTU handling (1400 bytes)
- [ ] Peer discovery
- [ ] Unit tests
- [ ] Integration tests

**Estimated Time**: 2 weeks

### 6. Privacy Protections (Week 9)

**Status**: Not Started

Add privacy layer to routing:

- [ ] Route randomization (top-k selection)
- [ ] Relay rotation
- [ ] Network-adaptive message padding
- [ ] Privacy notification system
- [ ] Unit tests

**Estimated Time**: 1 week

### 7. Onion Routing (Week 10)

**Status**: Not Started

Implement lightweight onion routing for SENSITIVE messages:

- [ ] 3-hop onion message construction
- [ ] Layer encryption/decryption
- [ ] Relay selection for onion routes
- [ ] Sender opt-out support
- [ ] Recipient notifications
- [ ] Unit tests

**Estimated Time**: 1 week

### 8. Integration & Testing (Weeks 11-12)

**Status**: Not Started

- [ ] End-to-end message delivery test
- [ ] Multi-hop routing test
- [ ] Store-and-forward test
- [ ] Content tag filtering test
- [ ] Privacy protection tests
- [ ] Performance benchmarks
- [ ] Stress tests

**Estimated Time**: 2 weeks

### 9. Security Review & Hardening (Weeks 13-14)

**Status**: Not Started

- [ ] Security audit of implementation
- [ ] Replay attack prevention validation
- [ ] Signature verification tests
- [ ] Rate limiting tuning
- [ ] Resource limit validation
- [ ] Bug fixes
- [ ] Documentation updates

**Estimated Time**: 2 weeks

## Dependencies

```
Week 1:  Protocol Updates
Week 1-2: DHT (depends on Protocol)
Week 3-4: Routing (depends on Protocol, DHT)
Week 5-6: Network Abstraction (depends on Protocol)
Week 7-8: Ethernet Adapter (depends on Network Abstraction)
Week 9: Privacy (depends on Routing)
Week 10: Onion Routing (depends on Routing, Privacy)
Week 11-12: Integration Tests (depends on all)
Week 13-14: Security Review (depends on all)
```

## New Crate Structure

```
myriadmesh/
├── crates/
│   ├── myriadmesh-core/         (existing, integration)
│   ├── myriadmesh-crypto/       (existing, Phase 1)
│   ├── myriadmesh-protocol/     (existing, updates needed)
│   ├── myriadmesh-dht/          (NEW - Week 1-2)
│   ├── myriadmesh-routing/      (NEW - Week 3-4)
│   ├── myriadmesh-network/      (NEW - Week 5-6)
│   └── myriadmesh-adapters/
│       └── ethernet/            (NEW - Week 7-8)
```

## Success Criteria

Phase 2 is complete when:

- ✅ Two nodes can discover each other via multicast
- ✅ Nodes can exchange messages via Ethernet adapter
- ✅ DHT stores and retrieves node records
- ✅ Messages route via multi-hop (at least 3 hops)
- ✅ Store-and-forward works for offline nodes
- ✅ Content tag filtering works as specified
- ✅ Privacy protections are functional
- ✅ All tests pass (unit, integration, security)
- ✅ Performance meets targets (>1000 msg/sec)
- ✅ Documentation is complete

## Current Progress

**Week 1 - Day 1** (2025-11-12)
- Created implementation plan
- Ready to begin protocol updates

## Next Steps

1. Update Cargo workspace with new crates
2. Update myriadmesh-protocol with Phase 2 features
3. Begin DHT implementation
