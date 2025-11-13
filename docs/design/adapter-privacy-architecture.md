# Network Adapter Privacy & Architecture Design

## Overview

This document outlines the privacy considerations and architectural decisions for MyriadMesh network adapters, with specific focus on IP-based adapters and privacy-aware message routing.

## Privacy Levels by Adapter Type

### High Privacy (0.8-1.0)
**Non-IP, Non-traceable Adapters**

**I2P (privacy_level: 0.95)**
- **Architecture**: Onion routing through I2P network
- **Traceability**: Very low - multi-layered encryption, no IP exposure
- **Use Case**: SENSITIVE messages, anonymous communication
- **Status**: Phase 4 (integrated)

**Bluetooth Mesh (privacy_level: 0.85)**
- **Architecture**: Local peer-to-peer mesh, no infrastructure
- **Traceability**: Low - MAC addresses only, local range
- **Use Case**: SENSITIVE local messages, air-gapped communication
- **Status**: Phase 3 (stub implementation)

**LoRa/Meshtastic (privacy_level: 0.90)**
- **Architecture**: Radio frequency mesh, no IP layer
- **Traceability**: Very low - RF only, no central infrastructure
- **Use Case**: Long-range anonymous communication
- **Status**: Phase 5 (planned)

### Medium Privacy (0.4-0.7)
**Limited Infrastructure Dependency**

**Bluetooth LE (privacy_level: 0.70)**
- **Architecture**: Point-to-point or small mesh
- **Traceability**: Medium - BLE advertisements can be tracked
- **Use Case**: IoT sensors, periodic updates
- **Status**: Phase 3 (stub implementation)

**Bluetooth Classic (privacy_level: 0.65)**
- **Architecture**: RFCOMM connections, SDP discovery
- **Traceability**: Medium - device pairing required
- **Use Case**: Device-to-device file transfer
- **Status**: Phase 3 (stub implementation)

### Low Privacy (0.1-0.3)
**IP-based, Traceable Adapters**

**Ethernet/Wi-Fi (privacy_level: 0.15)**
- **Architecture**: UDP over IP infrastructure (see below)
- **Traceability**: High - IP addresses logged by routers
- **Use Case**: Fast local network communication
- **Status**: Phase 2 (fully implemented)
- **Concerns**: See "Current Architecture Issues" below

**Cellular (privacy_level: 0.10)**
- **Architecture**: Carrier network with IMSI/IMEI tracking
- **Traceability**: Very high - tracked by carriers, government access
- **Use Case**: Wide-area connectivity when no alternative
- **Status**: Phase 3 (stub implementation)

---

## Current Ethernet/Wi-Fi Architecture (Phase 2)

### Implementation Details
**File**: `crates/myriadmesh-network/src/adapters/ethernet.rs`

**Current Mode**: Infrastructure Mode Only
- Binds to 0.0.0.0 (all interfaces)
- Uses existing IP network infrastructure
- Multicast discovery: 239.255.42.1:4002
- UDP transport on port 4001

**What's Missing**:
1. ❌ No ad-hoc/master mode support
2. ❌ No temporary IP mesh addressing
3. ❌ No detection of backhaul usage
4. ❌ No privacy considerations

### Architecture Issues

#### Issue 1: Self-Communication Over IP
**Problem**: Nodes can attempt to communicate with themselves over Wi-Fi when both sender and receiver are on the same machine.

**Current Behavior**:
- No self-loop detection
- Packets sent to own IP address
- Wasted bandwidth and resources

**Solution** (Future):
- Detect local NodeId before sending
- Use in-process IPC for same-node communication
- Skip IP layer for local messages

#### Issue 2: No Backhaul Detection
**Problem**: Cannot detect if Wi-Fi adapter is busy as backhaul (uplink to internet).

**Current Behavior**:
- Always uses Wi-Fi if available
- May interfere with user's internet connection
- No awareness of adapter role

**Solution** (Future):
- Query routing tables for default gateway
- Detect if interface is primary uplink
- Disable mesh mode on backhaul interfaces
- Configuration option: `allow_backhaul_mesh: bool`

#### Issue 3: No Ad-Hoc Networking
**Problem**: Relies on existing IP infrastructure, cannot create mesh networks.

**Current Behavior**:
- Requires existing router/AP
- Nodes must be on same subnet
- No direct node-to-node communication

**Proposed Solutions**:

##### Option A: Wi-Fi Direct Mode
```yaml
ethernet:
  mode: "direct"  # infrastructure | direct | hybrid
  direct_config:
    ssid: "myriadmesh-{node_id_prefix}"
    password: "{derived_from_node_key}"
    channel: 6
```

**Pros**:
- Direct device-to-device
- No infrastructure needed
- Better privacy than traditional Wi-Fi

**Cons**:
- Not all devices support Wi-Fi Direct
- Limited to 1-to-1 or small groups
- Requires elevated privileges

##### Option B: Temporary IP Mesh
```yaml
ethernet:
  mode: "mesh"
  mesh_config:
    subnet: "169.254.0.0/16"  # Link-local
    dhcp_mode: "adhoc"
    ttl: 10  # Max hops
```

**Pros**:
- Works with standard hardware
- Multi-hop routing
- Dynamic address assignment

**Cons**:
- Still uses IP (traceable)
- Complex routing required
- NAT traversal issues

##### Option C: Hybrid Mode (Recommended)
```yaml
ethernet:
  mode: "hybrid"
  prefer_infrastructure: true
  fallback_to_adhoc: true
  backhaul_detection: true
```

**Behavior**:
1. Check if interface is backhaul (default gateway)
2. If backhaul → disable mesh, only respond to direct queries
3. If not backhaul → check for existing AP
4. If AP exists → use infrastructure mode
5. If no AP → create ad-hoc network

**Pros**:
- Adapts to environment
- Best of both worlds
- Respects user's network

**Cons**:
- More complex logic
- Requires careful testing

---

## Privacy-Aware Message Routing

### Scoring Algorithm with Privacy

**Default Weights** (balanced):
```rust
latency: 0.25      // 25%
bandwidth: 0.20    // 20%
reliability: 0.30  // 30%
power: 0.10        // 10%
privacy: 0.15      // 15%
```

**Privacy-Optimized Weights** (for SENSITIVE messages):
```rust
latency: 0.10      // 10%
bandwidth: 0.05    // 5%
reliability: 0.20  // 20%
power: 0.10        // 10%
privacy: 0.55      // 55% ← Prioritized
```

### Message Classification

**SENSITIVE Messages**:
- User explicitly marks message as sensitive
- Contains personal information
- Privacy required by application
- → Uses privacy-optimized scoring

**NORMAL Messages**:
- Default message type
- Performance prioritized
- → Uses default/performance scoring

**URGENT Messages**:
- Low latency required
- Performance over privacy
- → Uses performance-optimized scoring

### Routing Decision Example

**Scenario**: Sending a SENSITIVE message

**Available Adapters**:
1. Ethernet (fast, 0.15 privacy) → Score: 0.35
2. Bluetooth (medium, 0.85 privacy) → Score: 0.78
3. I2P (slow, 0.95 privacy) → Score: 0.82

**Decision**: Route via I2P (highest score with privacy weight)

**Fallback**: If I2P unavailable → Bluetooth → Ethernet (only if no alternative)

---

## Implementation Roadmap

### Phase 3.5 (Current - Privacy Enhancements)
- ✅ Privacy scoring algorithm
- ✅ Privacy levels for each adapter
- ✅ Privacy-optimized weight profile
- ✅ SENSITIVE message classification support

### Phase 4 (Advanced Features)
- [ ] Backhaul detection for IP adapters
- [ ] Self-loop detection and IPC fallback
- [ ] I2P integration (already designed)
- [ ] Message-level privacy enforcement

### Phase 5 (Radio & Mesh)
- [ ] LoRa/Meshtastic adapter (high privacy)
- [ ] APRS adapter (medium privacy)
- [ ] Ad-hoc Wi-Fi mode
- [ ] Temporary IP mesh networking

### Phase 6 (Production)
- [ ] Privacy audit
- [ ] Metadata protection
- [ ] Traffic analysis resistance
- [ ] Timing attack mitigation

---

## Configuration Examples

### Privacy-First Configuration
```yaml
network:
  scoring:
    mode: "privacy"
    weight_privacy: 0.55

  adapters:
    ethernet:
      enabled: true
      auto_start: false      # Don't auto-start IP adapter
      backhaul_detection: true
      allow_backhaul_mesh: false

    bluetooth:
      enabled: true
      auto_start: true       # Prefer non-IP adapter

    i2p:
      enabled: true
      auto_start: true       # Maximum privacy
```

### Performance-First Configuration
```yaml
network:
  scoring:
    mode: "performance"
    weight_privacy: 0.10     # Lower priority

  adapters:
    ethernet:
      enabled: true
      auto_start: true       # Fast connection
      mode: "infrastructure"

    bluetooth:
      enabled: false         # Slower, disable
```

### Balanced Configuration (Default)
```yaml
network:
  scoring:
    mode: "default"
    weight_latency: 0.25
    weight_bandwidth: 0.20
    weight_reliability: 0.30
    weight_power: 0.10
    weight_privacy: 0.15
```

---

## Security Considerations

### IP Adapter Privacy Risks
1. **Traffic Analysis**: IP packets can be correlated
2. **ISP Logging**: All IP traffic logged by infrastructure
3. **Metadata Leakage**: Timing, size, destination
4. **Government Access**: Subpoenas can unmask users

### Mitigation Strategies
1. **Avoid IP for SENSITIVE**: Use scoring to prefer non-IP
2. **Onion Routing**: Use I2P when available
3. **Local Mesh**: Prefer Bluetooth/LoRa for local
4. **Encryption**: Always encrypt (already implemented)

### Future Enhancements
1. **Padding**: Add padding to hide message sizes
2. **Timing**: Random delays to resist timing attacks
3. **Cover Traffic**: Fake messages to hide patterns
4. **Mixing**: Mix messages from different sources

---

## Testing Requirements

### Privacy Routing Tests
- [ ] SENSITIVE messages prefer high-privacy adapters
- [ ] Privacy-optimized scoring selects I2P over Ethernet
- [ ] Fallback to lower privacy if no alternative
- [ ] Configuration switching (privacy/performance modes)

### Architecture Tests
- [ ] Self-loop detection prevents same-node IP communication
- [ ] Backhaul detection correctly identifies uplink interface
- [ ] Ad-hoc mode fallback when no infrastructure available
- [ ] Hybrid mode switches correctly based on environment

### Integration Tests
- [ ] Multi-adapter scoring with privacy weights
- [ ] SENSITIVE message routing end-to-end
- [ ] Failover maintains privacy preferences
- [ ] Performance vs privacy tradeoff validation

---

## References

- **Phase 2 Implementation**: `crates/myriadmesh-network/src/adapters/ethernet.rs`
- **Scoring Algorithm**: `crates/myriadnode/src/scoring.rs`
- **I2P Design**: `docs/design/phase2-detailed-design.md`
- **Protocol Spec**: `docs/protocol/message-format.md`

---

**Last Updated**: 2025-11-12
**Status**: Design Review
**Next Review**: Phase 4 Planning
