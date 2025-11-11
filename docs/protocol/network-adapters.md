# Network Adapter Specifications

## Overview

Network adapters are modular components that provide a unified interface for communicating over diverse physical and protocol layers. Each adapter translates between the MyriadMesh protocol and a specific network technology.

## Adapter Interface

All adapters must implement the following interface:

```rust
trait NetworkAdapter {
    // Lifecycle management
    fn initialize(&mut self, config: AdapterConfig) -> Result<(), Error>;
    fn start(&mut self) -> Result<(), Error>;
    fn stop(&mut self) -> Result<(), Error>;
    fn shutdown(&mut self) -> Result<(), Error>;

    // Core operations
    fn send(&self, destination: Address, frame: &Frame) -> Result<MessageId, Error>;
    fn receive(&self) -> Result<(Address, Frame), Error>;

    // Discovery and status
    fn discover_peers(&self) -> Result<Vec<PeerInfo>, Error>;
    fn get_status(&self) -> AdapterStatus;
    fn get_capabilities(&self) -> AdapterCapabilities;

    // Performance testing
    fn test_connection(&self, destination: Address) -> Result<TestResults, Error>;

    // Address management
    fn get_local_address(&self) -> Address;
    fn parse_address(&self, addr_str: &str) -> Result<Address, Error>;
}
```

## Adapter Types

### 1. Ethernet Adapter

**Technology**: IEEE 802.3 wired Ethernet and IP networks

**Configuration:**
```yaml
ethernet:
  enabled: true
  interface: "eth0"          # Network interface name
  bind_address: "0.0.0.0"    # Listen on all interfaces
  port: 4001                 # UDP port
  multicast_discovery: true  # Use multicast for local discovery
  multicast_group: "239.255.77.77"
  mtu: 1500
```

**Address Format:**
- `ipv4:192.168.1.100:4001`
- `ipv6:[2001:db8::1]:4001`
- `hostname:myriadnode.local:4001`

**Characteristics:**
- High bandwidth: 100 Mbps - 100 Gbps
- Low latency: 1-10 ms
- High reliability
- Limited range: physical cable length
- No power constraints (typically)
- Low cost per byte: effectively free

**Transport Protocol:**
- UDP for messages
- TCP for bulk transfers (optional)
- Multicast for local discovery

**MTU Handling:**
- Fragments messages >1400 bytes
- Supports jumbo frames if available

**Discovery:**
- Multicast announcements on local network
- mDNS/DNS-SD service registration
- DHT for Internet-wide discovery

**Security:**
- Standard MyriadMesh encryption
- Optional TLS for TCP connections
- Network-level firewalling

**Error Handling:**
- Automatic reconnection on link down
- ICMP monitoring for path MTU discovery

### 2. Bluetooth Classic Adapter

**Technology**: Bluetooth BR/EDR (Basic Rate/Enhanced Data Rate)

**Configuration:**
```yaml
bluetooth:
  enabled: true
  device_name: "MyriadNode-BT"
  discoverable: true
  connectable: true
  pin: "0000"                # For legacy pairing
  max_connections: 7         # Bluetooth piconet limit
  service_uuid: "00004d59-0000-1000-8000-00805f9b34fb"
```

**Address Format:**
- `bt:XX:XX:XX:XX:XX:XX` (MAC address)
- `bt-name:MyriadNode-Device`

**Characteristics:**
- Medium bandwidth: 1-3 Mbps
- Medium latency: 10-100 ms
- Good reliability
- Short range: 10-100 meters
- Moderate power consumption
- No data cost

**Transport Protocol:**
- RFCOMM for stream-based communication
- L2CAP for packet-based communication
- SDP for service discovery

**Discovery:**
- Active scanning for discoverable devices
- Service discovery via SDP
- Maintains list of paired devices

**Security:**
- Bluetooth pairing required
- Link-level encryption (AES-128)
- Additional MyriadMesh encryption

**Error Handling:**
- Auto-reconnect on disconnection
- Fallback to other adapters if pairing fails

### 3. Bluetooth Low Energy (BLE) Adapter

**Technology**: Bluetooth LE 4.0+

**Configuration:**
```yaml
ble:
  enabled: true
  device_name: "MyriadNode-BLE"
  advertising_interval: 100  # ms
  connection_interval: 30    # ms
  service_uuid: "4d590001-5f9b-4000-8000-00805f9b34fb"
  characteristic_uuid: "4d590002-5f9b-4000-8000-00805f9b34fb"
  mtu: 247                   # BLE 4.2+ supports up to 247
```

**Address Format:**
- `ble:XX:XX:XX:XX:XX:XX`
- `ble-name:MyriadNode-BLE`

**Characteristics:**
- Low bandwidth: 125-1000 Kbps
- Higher latency: 50-500 ms
- Good reliability in mesh mode
- Short range: 10-100 meters (up to 1000m with BLE 5.0)
- Very low power consumption
- No data cost

**Transport Protocol:**
- GATT characteristics for data transfer
- Mesh networking support (BLE Mesh)

**Discovery:**
- Advertisement scanning
- GATT service discovery

**Mesh Networking:**
- Implements BLE Mesh for multi-hop
- Managed flooding for broadcasts
- Friend/Low Power Node relationships

**Security:**
- BLE pairing with numeric comparison
- Link encryption (AES-CCM)
- MyriadMesh encryption

**Optimization:**
- Connection parameters tuned for latency vs power
- MTU negotiation for larger packets
- Notification-based data transfer

### 4. Cellular (4G/5G) Adapter

**Technology**: LTE, 5G NR

**Configuration:**
```yaml
cellular:
  enabled: true
  apn: "internet"
  username: ""
  password: ""
  preferred_mode: "5G"       # 4G, 5G, auto
  roaming_allowed: false
  data_limit_mb: 1000        # Monthly limit
  cost_per_mb: 0.10          # USD
  priority: 100              # Lower priority due to cost
```

**Address Format:**
- Uses IP addressing over cellular connection
- NAT traversal via DHT or relay nodes

**Characteristics:**
- High bandwidth: 10-1000 Mbps
- Low latency: 20-50 ms (5G), 30-100 ms (4G)
- Good reliability
- Wide area coverage
- Moderate power consumption
- **Expensive data cost** (prioritize accordingly)

**Transport Protocol:**
- TCP/UDP over IP
- HTTP/3 (QUIC) for efficiency

**Discovery:**
- Requires Internet connectivity
- DHT-based node lookup
- Bootstrap node connections

**Security:**
- Cellular network encryption
- MyriadMesh encryption
- VPN optional

**Cost Management:**
- Track data usage per destination
- Prefer for high-priority messages only
- Automatic fallback to cheaper networks
- Compression for all payloads

**Connection Management:**
- Keep-alive optimization
- Fast dormancy support
- Background data restrictions

### 5. Wi-Fi HaLoW (802.11ah) Adapter

**Technology**: IEEE 802.11ah sub-1 GHz Wi-Fi

**Configuration:**
```yaml
wifi_halow:
  enabled: true
  interface: "wlan1"
  frequency: 900             # MHz (varies by region)
  channel_width: 2           # MHz (1, 2, 4, 8, 16)
  tx_power: 20               # dBm
  mode: "mesh"               # infrastructure, mesh, ad-hoc
```

**Address Format:**
- Similar to Ethernet: `halow:ipv6-link-local:port`

**Characteristics:**
- Low-medium bandwidth: 150 Kbps - 40 Mbps
- Medium latency: 10-100 ms
- Good reliability
- Long range: up to 1 km
- Low power consumption
- No data cost

**Transport Protocol:**
- IP over 802.11ah
- Mesh networking via 802.11s

**Discovery:**
- Beacon frames
- Mesh peering management

**Use Cases:**
- IoT sensor networks
- Long-range backhaul
- Rural connectivity

**Power Optimization:**
- Target Wake Time (TWT)
- Power Save Mode (PSM)

### 6. LoRaWAN/Meshtastic Adapter

**Technology**: LoRa modulation, LoRaWAN protocol, Meshtastic mesh

**Configuration:**
```yaml
lora:
  enabled: true
  frequency: 915.0           # MHz (varies by region)
  spreading_factor: 7        # 7-12 (higher = longer range, slower)
  bandwidth: 125             # kHz
  coding_rate: 5             # 5-8
  tx_power: 20               # dBm (max 30)
  preamble_length: 8
  sync_word: 0x12
  crc_enabled: true

  # Meshtastic-specific
  meshtastic_enabled: true
  meshtastic_relay: true     # Relay Meshtastic packets
  meshtastic_channel: "LongFast"
```

**Address Format:**
- `lora:device-id` (4-byte device ID)
- `meshtastic:node-id` (hex node ID)

**Characteristics:**
- Very low bandwidth: 0.3 - 50 Kbps
- High latency: 1-10 seconds
- Variable reliability (depends on conditions)
- Very long range: 2-15 km (up to 100+ km line-of-sight)
- Very low power consumption
- No data cost
- License-free spectrum

**Transport Protocol:**
- LoRaWAN for infrastructure mode
- Meshtastic protocol for mesh mode
- Custom packet format for direct P2P

**Packet Size:**
- Maximum payload: 222 bytes (SF7) - 51 bytes (SF12)
- MyriadMesh frames must be fragmented

**Fragmentation:**
```
Fragment Header:
[Frag ID (2B)][Frag Index (1B)][Total Frags (1B)][Data]

Example:
Message 1000 bytes -> 5 fragments of ~200 bytes each
```

**Duty Cycle:**
- Europe: 1% duty cycle limit
- US: No duty cycle limit, but FCC power limits
- Implements fair access queuing

**Mesh Networking:**
- Multi-hop routing
- Flood-based broadcasts
- Route discovery via test packets

**Meshtastic Integration:**
- Translate Meshtastic packets to MyriadMesh
- Relay Meshtastic traffic for community support
- Dual-protocol operation

**Use Cases:**
- Off-grid communication
- Emergency networks
- Rural IoT
- Hiking/outdoor activities

### 7. FRS/GMRS Adapter

**Technology**: Family Radio Service / General Mobile Radio Service

**Configuration:**
```yaml
frs_gmrs:
  enabled: true
  radio_device: "/dev/ttyUSB0"  # Serial connection to radio
  frequency: 462.5625         # MHz (FRS/GMRS channel 1)
  channel: 1                  # 1-22
  ctcss_tone: 67.0            # Hz (privacy code)
  squelch: 5                  # 0-9
  tx_power: 2                 # Watts (FRS limited to 2W)
  vox_enabled: false
```

**Address Format:**
- `frs:channel:ctcss` (e.g., `frs:1:67.0`)
- Group communication (no direct addressing)

**Characteristics:**
- Very low bandwidth: 1200 bps (digital mode)
- High latency: 5-30 seconds
- Variable reliability
- Medium range: 1-5 km
- Low power consumption
- No data cost
- License-free (FRS) or licensed (GMRS)

**Transport Protocol:**
- AFSK (Audio Frequency Shift Keying)
- Digital mode encoding (e.g., FreeDV, Codec2)

**Data Encoding:**
- Voice-band modem: 300-1200 bps
- Error correction: Reed-Solomon
- Packet structure: AX.25-like

**Discovery:**
- Periodic beacons on configured channel
- Channel scanning for activity

**Limitations:**
- Simplex only (half-duplex)
- Shared channel (collision avoidance needed)
- Short messages only (max ~100 bytes)

**Use Cases:**
- Short-range coordination
- Emergency backup
- Hiking group communication

### 8. Amateur Packet Radio (APRS) Adapter

**Technology**: AX.25 protocol over VHF/UHF amateur radio

**Configuration:**
```yaml
aprs:
  enabled: true
  callsign: "N0CALL"          # Amateur radio callsign (required!)
  ssid: 7                     # 0-15 (7 = portable)
  frequency: 144.390          # MHz (APRS frequency varies by region)
  radio_device: "/dev/ttyUSB0"
  tnc_device: "/dev/ttyUSB1"  # Terminal Node Controller
  beacon_interval: 600        # seconds
  path: "WIDE1-1,WIDE2-1"     # Digipeater path
  symbol: "/"                 # APRS symbol table
  symbol_code: "["            # APRS symbol (computer)
```

**Address Format:**
- `aprs:CALLSIGN-SSID` (e.g., `aprs:N0CALL-7`)

**Characteristics:**
- Low bandwidth: 1200 bps (VHF), 9600 bps (UHF)
- Medium latency: 1-60 seconds
- Good reliability with digipeaters
- Long range: 10-100+ km
- Low power consumption
- No data cost
- **Requires amateur radio license**

**Transport Protocol:**
- AX.25 link layer protocol
- APRS application layer
- UI frames for data

**Packet Format:**
```
AX.25 Header + APRS Data Field
Data: :RECIPIENT:message content{msg_id}
```

**Discovery:**
- Position beacons include capabilities
- Station lists via iGate/APRS-IS

**Digipeater Support:**
- Automatic routing via WIDE paths
- Fills in digipeater callsigns

**APRS-IS Integration:**
- Optional Internet gateway
- Worldwide message relay
- Historical data access

**Location Services:**
- GPS integration
- Position beacons
- Map integration

**Use Cases:**
- Emergency communication (EMCOMM)
- Long-distance messaging
- Asset tracking
- Weather reporting

### 9. CB/Shortwave Radio Adapter

**Technology**: Citizens Band (CB) and HF shortwave radio

**Configuration:**
```yaml
cb_shortwave:
  enabled: true
  mode: "cb"                  # cb, hf
  radio_device: "/dev/ttyUSB0"
  frequency: 27.185           # MHz (CB channel 19)
  modulation: "SSB"           # AM, FM, SSB, CW
  bandwidth: 3000             # Hz
  tx_power: 4                 # Watts (CB limited to 4W)
```

**Address Format:**
- Broadcast only (channel-based)
- `cb:channel` or `hf:frequency`

**Characteristics:**
- Very low bandwidth: 50-300 bps
- Very high latency: 10-120 seconds
- Variable reliability (depends on propagation)
- Variable range: 5-50 km (CB), 100-10000+ km (HF)
- Low power consumption
- No data cost
- License-free (CB) or licensed (HF)

**Transport Protocol:**
- PSK31, RTTY, or MFSK for digital modes
- Slow-speed modem protocols
- Forward error correction essential

**Data Encoding:**
- Varicode for efficient text
- Binary encoding for structured data
- Compression critical due to low bandwidth

**Propagation:**
- CB: Ground wave and skip (sporadic)
- HF: Ionospheric propagation (time/season dependent)
- NVIS (Near Vertical Incidence Skywave) for regional

**Discovery:**
- CQ beacons on specific frequencies
- Scheduled contact windows

**Use Cases:**
- Long-distance emergency communication
- Remote area connectivity
- Disaster response
- Maritime communication

### 10. Dial-up / PPPoE Adapter

**Technology**: Modem dial-up, PPPoE over phone/DSL

**Configuration:**
```yaml
dialup:
  enabled: true
  device: "/dev/ttyS0"        # Serial port
  phone_number: "5551234"
  username: "user"
  password: "pass"
  baud_rate: 56000            # Max 56k for V.90
  init_string: "ATZ"
  dial_on_demand: true
  idle_timeout: 300           # seconds
```

**Address Format:**
- Uses IP addressing once connected

**Characteristics:**
- Very low bandwidth: 28.8 - 56 Kbps
- High latency: 100-300 ms
- Moderate reliability
- Limited range: phone line reach
- No additional power (uses phone line power)
- Low data cost (local call) or expensive (long distance)

**Transport Protocol:**
- PPP (Point-to-Point Protocol)
- TCP/IP over PPP

**Connection Management:**
- Dial on demand for efficiency
- Disconnect after idle timeout
- Automatic redial on failure

**Optimization:**
- Header compression (Van Jacobson)
- Data compression (V.42bis)
- Prioritize small messages

**Use Cases:**
- Legacy system support
- Rural areas with no broadband
- Backup connectivity

### 11. i2p Overlay Adapter

**Technology**: Invisible Internet Project anonymity network

**Configuration:**
```yaml
i2p:
  enabled: true
  sam_host: "127.0.0.1"
  sam_port: 7656              # SAM bridge port
  tunnel_length: 3            # Hops (3 recommended)
  tunnel_quantity: 2          # Redundant tunnels
  tunnel_backup: 1
  destination_public: true    # Publish in network database
```

**Address Format:**
- `i2p:base64-destination.b32.i2p`
- Example: `i2p:ukeu3k5oycgaauneqgtnvselmt4yemvoilkln7jpvamvfx7dnkdq.b32.i2p`

**Characteristics:**
- Medium bandwidth: 100 Kbps - 1 Mbps
- Very high latency: 1-10 seconds
- Good reliability
- Global reach
- Moderate power consumption
- No data cost
- **Strong anonymity/privacy**

**Transport Protocol:**
- I2CP (I2P Control Protocol)
- SAM (Simple Anonymous Messaging) bridge
- Streaming or datagram mode

**Discovery:**
- Address book subscriptions
- Network database lookups
- Manual destination exchange

**Tunnels:**
- Inbound and outbound tunnels
- Each tunnel uses multiple hops
- Tunnels rebuilt periodically

**Security:**
- End-to-end encryption
- Onion-like layered encryption
- No central authorities
- Traffic mixing

**Privacy Features:**
- Source IP hidden
- Destination IP hidden
- Traffic analysis resistant

**Use Cases:**
- Privacy-conscious communication
- Censorship resistance
- Anonymous messaging
- Secure file sharing

## Adapter Selection Strategy

### Multi-Criteria Decision Making

When multiple adapters are available, select based on:

```python
def select_adapter(destination, message, available_adapters):
    scores = {}
    for adapter in available_adapters:
        # Get metrics
        metrics = get_metrics(adapter, destination)

        # Calculate weighted score based on message priority
        if message.priority == EMERGENCY:
            score = (
                metrics.reliability * 0.6 +
                metrics.availability * 0.3 +
                (1 - normalize(metrics.latency)) * 0.1
            )
        elif message.priority == HIGH:
            score = (
                (1 - normalize(metrics.latency)) * 0.4 +
                metrics.reliability * 0.3 +
                normalize(metrics.bandwidth) * 0.2 +
                (1 - normalize(metrics.cost)) * 0.1
            )
        elif message.size > 1000000:  # Large file
            score = (
                normalize(metrics.bandwidth) * 0.5 +
                (1 - normalize(metrics.cost)) * 0.3 +
                metrics.reliability * 0.2
            )
        elif message.anonymous:  # Privacy required
            if adapter.type == 'i2p':
                score = 1.0
            else:
                score = 0.0
        else:  # Normal message
            score = (
                (1 - normalize(metrics.latency)) * 0.25 +
                metrics.reliability * 0.25 +
                normalize(metrics.bandwidth) * 0.2 +
                (1 - normalize(metrics.cost)) * 0.2 +
                metrics.availability * 0.1
            )

        scores[adapter] = score

    # Return sorted list for failover
    return sorted(scores.items(), key=lambda x: x[1], reverse=True)
```

### Adapter Tier System

Based on continuous testing, adapters are assigned tiers:

**Tier 1: Primary**
- Best overall performance
- High reliability
- Acceptable cost

**Tier 2: Secondary**
- Good performance
- Automatic failover from Tier 1

**Tier 3: Fallback**
- Lower performance but available
- Used when Tiers 1-2 unavailable

**Tier 4: Last Resort**
- Very limited capabilities
- Emergency use only

**Tier 5: Offline**
- Store-and-forward
- Eventual delivery

## Adapter Development Guide

### Creating a New Adapter

1. **Implement the NetworkAdapter trait**
2. **Define adapter-specific configuration**
3. **Implement frame encapsulation for transport**
4. **Handle address parsing and formatting**
5. **Implement discovery mechanism**
6. **Add performance testing**
7. **Document capabilities and limitations**
8. **Write integration tests**

### Testing Requirements

Each adapter must pass:
- Unit tests for core functions
- Integration tests with real hardware (where possible)
- Performance benchmarks
- Stress tests
- Error recovery tests

### Documentation Requirements

- Configuration schema
- Address format specification
- Characteristics and limitations
- Use case recommendations
- Troubleshooting guide

## Next Steps

- [Protocol Specification](specification.md)
- [DHT and Routing](dht-routing.md)
- [Security Considerations](../security/cryptography.md)
