# Phase 2 Design - Executive Summary

**Version**: 2.0 (Updated with Comprehensive Privacy Protections)
**Status**: Ready for Review
**Full Design**: [phase2-detailed-design.md](./phase2-detailed-design.md)
**Privacy Details**: [phase2-privacy-protections.md](./phase2-privacy-protections.md)

## Key Design Decisions

### 1. ✅ No Proof-of-Work (PoW)

**Decision**: Skip PoW for node identity generation

**Rationale**:
- Optimized for embedded/low-power devices (Raspberry Pi, IoT)
- PoW only applies to node ID generation (one-time), provides limited benefit
- Reputation-based Sybil resistance is more effective for this protocol
- Can add optional "verified node" status with PoW in Phase 4 if needed

### 2. ✅ E2E Encryption with Optional Content Tags

**Decision**: All payloads are E2E encrypted. Optional cleartext metadata tags enable relay filtering.

**Architecture**:
```
Message Structure:
├── Cleartext Header
│   ├── Source/Dest Node IDs
│   ├── Message Type, Priority, TTL
│   ├── Routing Flags (NEW)
│   └── Content Tags (NEW - Optional)
├── E2E Encrypted Payload
│   └── Only sender/receiver can decrypt
└── Signature
```

**Three Operating Modes**:
1. **E2E_STRICT** (default): No tags, relays blindly forward
2. **SENSITIVE**: User-designated, relays MUST forward regardless of policy
3. **RELAY_FILTERABLE**: Optional tags, relays MAY filter

**Example Tags**: `nsfw`, `political`, `commercial`, `media:video`, `size:large`

**Security Properties**:
- ✅ Perfect forward secrecy (ephemeral keys)
- ✅ Relays CANNOT read content (always encrypted)
- ✅ Tags are hints only (untrusted, sender can lie)
- ✅ Availability-first: default is blind relay
- ✅ User control: can mark messages as SENSITIVE to bypass filtering

### 3. ✅ Reputation-Based Trust

**Decision**: Track relay node reputation instead of PoW

**Reputation Score** (0.0 - 1.0):
- 50% weight: Relay success rate
- 30% weight: Uptime (max 90 days)
- 20% weight: Age (max 30 days)

**Minimum reputation**: 0.3 to be considered trustworthy

**Sybil Resistance**:
- New nodes start with neutral reputation (0.5)
- Must prove reliability over time
- Bad actors are pruned from routing tables

### 4. ✅ Strict Resource Limits

**DHT Storage**:
- 100MB max per node
- 10,000 keys max
- 1MB max value size

**Message Caching** (Store-and-Forward):
- 100 messages max per destination
- 24 hours max TTL
- 10,000 total cached messages
- 1MB max message size

**Rate Limiting**:
- 1,000 messages/min per node
- 10,000 messages/min globally

### 5. ✅ Availability-First Protocol

**Design Principle**: Maximize message delivery while respecting user privacy preferences

**Default Behavior**:
- Relay all messages (no filtering)
- E2E encryption always on
- No content inspection

**User Can Opt-In To**:
- Content tagging for bandwidth optimization
- Relay filtering policies (as relay operator)
- Anonymous routing (via i2p, Phase 4)

### 6. ✅ **NEW**: Comprehensive Privacy Protections

**Decision**: Implement layered defense-in-depth privacy protections that adapt to network constraints

**Key Innovation**: Network-adaptive privacy with user transparency

**Privacy Layers** (see [detailed privacy document](./phase2-privacy-protections.md)):

1. **Route Randomization** (Always On, Free)
   - Select from top 5 relays instead of always "best"
   - Limits surveillance to 1/5 of traffic per relay

2. **Relay Rotation** (Always On, Free)
   - Change relays every hour or 100 messages
   - Prevents long-term surveillance

3. **Iterative DHT Lookup Privacy** (Optional)
   - Don't reveal who you're looking up
   - Start from random nodes, use blinded targets

4. **Network-Adaptive Message Padding** (Always On, Intelligent)
   - Ethernet/Cellular: 30% overhead, buckets [512, 2K, 8K, 32K, 128K]
   - LoRaWAN: 10% overhead, buckets [51, 115, 222] - **NOTIFY USER** if exceeds
   - Dial-up/APRS: Disabled - **NOTIFY USER**
   - User gets explicit notification when padding reduced/disabled

5. **Context-Aware Timing Obfuscation** (Optional)
   - Only for single-recipient messages (not groups/broadcasts)
   - Random 0-500ms delay
   - Prevents request/response correlation

6. **Lightweight Onion Routing** (SENSITIVE messages, Can Opt-Out)
   - 3-hop onion routing for SENSITIVE flag
   - Sender can disable with `NO_ONION_ROUTING` flag
   - **Both sender and recipient notified** when disabled
   - Each relay only knows prev/next hop

7. **HVT-Based Adaptive Decoy Traffic** (Off by Default, Opt-In)
   - Network-aware rates:
     - Ethernet: 60/hour
     - LoRaWAN: 1/hour (duty cycle!)
     - APRS: 1 per 10 hours (shared spectrum)
   - Only for designated High-Value Targets
   - Prevents traffic analysis

8. **Full i2p Integration** (Phase 4, Multiple Modes)
   - Application-level: Apps choose i2p routing
   - Relay mode: Share bandwidth to help i2p network
   - Exit mode: Allow i2p → clearnet (legal considerations)

**Critical Features**:
- ✅ **User Transparency**: Explicit notifications when privacy reduced
- ✅ **Network-Aware**: Adapts to LoRa/Dial-up constraints
- ✅ **Availability-First**: Privacy degrades gracefully, not availability
- ✅ **User Control**: Sender can opt-out with full disclosure

**Example**: LoRaWAN Privacy
```
User sends 200-byte message on LoRa:
1. Padding would bump to 222 bytes (10% overhead) ✅
2. If padding exceeds duty cycle budget:
   → User notified: "Padding exceeds LoRa spectrum budget"
   → Options:
     - Reduce to minimum priority (send later)
     - Resend without padding (privacy loss warning)
     - Queue for better adapter (Ethernet/Cellular)
```

**Privacy vs Performance Matrix**:

| Feature | Bandwidth | Latency | Privacy Gain | Default |
|---------|-----------|---------|--------------|---------|
| Route Randomization | 0% | 0-5% | Medium | ✅ ON |
| Relay Rotation | 0% | 0% | Medium | ✅ ON |
| DHT Lookup Privacy | 0% | 50-100% | Low | ⚠️ OFF |
| Message Padding | 0-30% | 0% | High | ✅ ON |
| Timing Obfuscation | 0% | 10-20% | Low-Med | ⚠️ OFF |
| Onion Routing (3-hop) | 200-300% | 200-300% | Very High | ✅ SENSITIVE |
| Decoy Traffic | User-defined | 0% | High | ⚠️ OFF |
| i2p Integration | ~100% | 1000-5000% | Maximum | ⏸️ Phase 4 |

**Timeline Impact**: +2 weeks (14 weeks total)

---

## Components to Build

### 1. DHT Manager (Kademlia)
- 256 k-buckets (k=20 nodes per bucket)
- XOR distance metric
- FIND_NODE, STORE, FIND_VALUE operations
- Reputation tracking
- Maintenance loops

### 2. Message Router
- 5-level priority queue (Emergency → Background)
- Direct routing
- Multi-hop routing (via DHT lookup)
- Store-and-forward for offline nodes
- Message deduplication (LRU cache)
- Content tag filtering (optional)

### 3. Network Abstraction Layer
- NetworkAdapter trait/interface
- Adapter registration and lifecycle
- Performance metric tracking
- Adapter selection algorithm

### 4. Ethernet Adapter (First Implementation)
- UDP transport on port 4001
- Multicast discovery (239.255.77.77)
- IPv4/IPv6 support
- MTU 1400 (safe default)

---

## Protocol Updates (from Phase 1)

### New Message Fields

```rust
pub struct MessageFrame {
    // ... existing fields ...

    // NEW: Routing metadata
    pub routing_flags: RoutingFlags,   // E2E_STRICT, SENSITIVE, RELAY_FILTERABLE
    pub content_tags: Vec<String>,     // Optional tags (max 10, 32 bytes each)

    // ... payload, signature ...
}
```

### New Routing Flags

```
E2E_STRICT        (0x01) - Strictly E2E encrypted (default)
SENSITIVE         (0x02) - User-designated sensitive (MUST relay)
RELAY_FILTERABLE  (0x04) - Relays may filter based on tags
MULTI_PATH        (0x08) - Multi-path routing (future)
ANONYMOUS         (0x10) - Route via i2p (Phase 4)
```

---

## Open Questions for User

### 1. Content Tag Namespace

**Question**: Should we define a formal tag schema, or allow freeform tags?

**Option A**: Strict schema (only predefined tags allowed)
- Pros: Consistent, easy to filter
- Cons: Less flexible

**Option B**: Freeform tags (any string)
- Pros: Extensible, flexible
- Cons: No standardization

**Recommendation**: Start with standard tags, allow custom tags with prefix `custom:`

### 2. DHT Bootstrap

**Question**: How should new nodes discover the DHT network?

**Options**:
- A: Hardcoded bootstrap nodes (simple, centralized)
- B: DNS-based discovery (flexible, still somewhat centralized)
- C: Local multicast only (fully decentralized, limited range)

**Recommendation**: Hybrid - local multicast first, fallback to DNS bootstrap

### 3. Storage Replication Factor

**Question**: How many nodes should store each DHT value?

**Current**: k=20 (might be too high)

**Recommendation**: k=3 for Phase 2 (can increase later)

### 4. Message Priority Enforcement

**Question**: Can relay nodes downgrade message priority?

**Options**:
- A: Relays must preserve priority (sender controls)
- B: Relays can downgrade (relay protects itself)

**Recommendation**: Must preserve (sender controls, relay can refuse entirely)

### 5. Phase 2 Scope

**Question**: Should we add basic i2p/Tor support in Phase 2?

**Options**:
- A: Add now (more complex, better privacy)
- B: Wait for Phase 4 (simpler, focused)

**Recommendation**: Wait for Phase 4 (keep Phase 2 focused)

---

## Testing Strategy

### Unit Tests
- All core functions
- Error conditions
- Edge cases
- Resource limit enforcement

### Integration Tests
- End-to-end message delivery
- Multi-hop routing (3+ hops)
- Store-and-forward
- Content tag filtering
- DHT operations

### Performance Tests
- Message throughput (target: >1,000 msg/sec)
- DHT lookup latency (target: <500ms)
- Memory usage under load

### Security Tests
- Replay attack prevention
- Signature verification
- Rate limiting
- Storage exhaustion prevention

---

## Timeline

**Total**: 14 weeks (~3.5 months)

- Week 1-2: DHT Implementation
- Week 3-4: Message Router
- Week 5-6: Network Abstraction
- Week 7-8: Ethernet Adapter
- **Week 9: Privacy Layer Integration** (NEW)
  - Adaptive padding system
  - Route randomization/rotation
  - Privacy notification system
- **Week 10: Onion Routing** (NEW)
  - 3-hop onion routing implementation
  - Sender opt-out mechanism
  - Recipient notifications
- Week 11-12: Integration & Testing (expanded scope)
  - Privacy protection testing
  - Network-adaptive behavior testing
- Week 13-14: Security Review & Hardening (expanded scope)
  - Privacy layer security audit
  - Metadata leakage analysis

---

## Success Criteria

Phase 2 is complete when:

- ✅ Two nodes discover each other via multicast
- ✅ Nodes exchange messages via Ethernet
- ✅ DHT stores and retrieves node records
- ✅ Multi-hop routing works (3+ hops)
- ✅ Store-and-forward works for offline nodes
- ✅ Content tag filtering works correctly
- ✅ All tests pass
- ✅ Performance meets targets (>1000 msg/sec)
- ✅ Documentation complete

---

## Implementation Plan

### New Crates

```
myriadmesh/
├── crates/
│   ├── myriadmesh-core/          # Phase 1 (minor updates)
│   ├── myriadmesh-crypto/        # Phase 1 (no changes)
│   ├── myriadmesh-protocol/      # Phase 1 (minor updates for new flags)
│   ├── myriadmesh-dht/           # Phase 2 (NEW)
│   ├── myriadmesh-routing/       # Phase 2 (NEW)
│   ├── myriadmesh-network/       # Phase 2 (NEW)
│   └── myriadmesh-adapters/
│       └── ethernet/             # Phase 2 (NEW)
```

### Compatibility

- Phase 2 nodes can communicate with Phase 1 nodes (direct only)
- Phase 1 nodes cannot use DHT or relay
- Gradual migration supported

---

## Key Risks & Mitigations

### Risk: DHT Sybil Attack
- **Mitigation**: Reputation-based filtering, can add PoW later

### Risk: Message Flooding DoS
- **Mitigation**: Rate limiting per-node and global

### Risk: Storage Exhaustion
- **Mitigation**: Strict storage limits (100MB DHT, 100 msgs cached)

### Risk: Malicious Relays Dropping Messages
- **Mitigation**: Reputation tracking, multi-path routing (Phase 4)

### Risk: Privacy Leak via Tags
- **Mitigation**: Tags optional, SENSITIVE flag bypasses filtering

---

## Next Steps

1. **Review this summary** and full design document
2. **Answer open questions** above
3. **Provide feedback** on any design decisions
4. **Approve to begin implementation** or request changes

---

## Questions for You

1. Do the design decisions align with your vision for an "availability-first" protocol?

2. Is the content tagging system sufficient for your use case (allowing relays to filter NSFW/commercial content)?

3. Are the resource limits appropriate for your target deployment (embedded devices)?

4. Should we start Phase 2 implementation, or do you want to see more detailed specs for specific components first?

5. Any concerns about the 12-week timeline?

---

**Ready to proceed?** Let me know your thoughts, and I'll begin implementation or iterate on the design as needed.
