# DHT and Routing Specification

## Overview

MyriadMesh uses a Distributed Hash Table (DHT) based on Kademlia for:
- Decentralized node discovery
- Distributed storage of node information
- Route caching and lookup
- Message caching for offline nodes
- Network topology awareness

## DHT Architecture

### Node ID Space

```
ID Space: 256-bit (32 bytes)
Total nodes: 2^256 possible IDs
Distance metric: XOR (Exclusive OR)

Node ID derived from: BLAKE2b-256(Ed25519_PublicKey)
```

### Distance Metric

XOR distance provides these properties:
- **Symmetric**: d(a,b) = d(b,a)
- **Identity**: d(a,a) = 0
- **Triangle inequality**: d(a,c) ≤ d(a,b) + d(b,c)
- **Unique**: Only one node at distance 0

```python
def distance(node_id_a, node_id_b):
    """Calculate XOR distance between two node IDs"""
    # XOR the byte arrays
    xor_result = bytes(a ^ b for a, b in zip(node_id_a, node_id_b))

    # Return as integer for comparison
    return int.from_bytes(xor_result, 'big')
```

### K-Buckets

Each node maintains a routing table with 256 k-buckets (one per bit):

```
Bucket 0: Nodes with 255 common prefix bits (distance 2^0 to 2^1 - 1)
Bucket 1: Nodes with 254 common prefix bits (distance 2^1 to 2^2 - 1)
...
Bucket 255: Nodes with 0 common prefix bits (distance 2^255 to 2^256 - 1)

Each bucket stores up to k nodes (k=20 recommended)
```

**Bucket Structure:**
```python
class KBucket:
    def __init__(self, k=20):
        self.nodes = []          # List of NodeInfo, max k entries
        self.last_updated = 0     # Timestamp of last activity
        self.replacement_cache = []  # Cache for potential replacements

class NodeInfo:
    node_id: bytes32             # Node identifier
    public_key: bytes32          # Ed25519 public key
    adapters: List[AdapterInfo]  # Available network adapters
    last_seen: timestamp         # Last communication
    rtt: float                   # Round-trip time (ms)
    failures: int                # Consecutive failure count
    reputation: float            # 0.0 - 1.0
```

### Bucket Maintenance

**Add Node:**
```python
def add_node(node_info):
    bucket_index = get_bucket_index(node_info.node_id)
    bucket = buckets[bucket_index]

    # If node already exists, move to tail (most recently seen)
    if node_info.node_id in bucket:
        bucket.move_to_tail(node_info)
        return

    # If bucket not full, add node
    if len(bucket.nodes) < k:
        bucket.nodes.append(node_info)
        return

    # Bucket full - ping least recently seen node (head)
    head_node = bucket.nodes[0]
    if ping(head_node):
        # Head still alive - move to tail, add new to replacement cache
        bucket.move_to_tail(head_node)
        bucket.replacement_cache.append(node_info)
    else:
        # Head dead - replace with new node
        bucket.nodes.remove(head_node)
        bucket.nodes.append(node_info)
```

**Bucket Refresh:**
```python
# Periodically refresh buckets with no recent activity
BUCKET_REFRESH_INTERVAL = 3600  # 1 hour

def refresh_buckets():
    now = time.now()
    for i, bucket in enumerate(buckets):
        if now - bucket.last_updated > BUCKET_REFRESH_INTERVAL:
            # Generate random ID in this bucket's range
            random_id = generate_random_id_in_bucket(i)

            # Perform lookup to populate bucket
            lookup_node(random_id)
```

## DHT Operations

### FIND_NODE

Locate k closest nodes to a target ID.

**Request:**
```python
def find_node_request(target_id):
    return {
        "type": "FIND_NODE",
        "target": target_id,
        "query_id": generate_query_id()
    }
```

**Response:**
```python
def find_node_response(query_id, target_id):
    # Return k closest nodes from routing table
    closest = get_k_closest_nodes(target_id, k=20)

    return {
        "type": "FIND_NODE_RESPONSE",
        "query_id": query_id,
        "nodes": [
            {
                "node_id": node.node_id,
                "public_key": node.public_key,
                "adapters": node.adapters,
                "last_seen": node.last_seen
            }
            for node in closest
        ]
    }
```

**Iterative Lookup:**
```python
def lookup_node(target_id):
    """Iterative lookup to find k closest nodes to target"""

    # Start with k closest from our routing table
    closest = get_k_closest_nodes(target_id, k=20)
    queried = set()
    pending = []

    # Initial parallel queries (alpha=3)
    alpha = 3
    for node in closest[:alpha]:
        pending.append(query_async(node, "FIND_NODE", target_id))
        queried.add(node.node_id)

    while pending:
        # Wait for next response
        response = await_any(pending)
        pending.remove(response)

        # Add newly discovered nodes
        for node_info in response.nodes:
            if node_info.node_id not in queried:
                # Add to closest list (sorted by distance)
                insert_sorted(closest, node_info, target_id)

                # Query if closer than furthest queried
                if len(queried) < k or is_closer(node_info, furthest_queried, target_id):
                    pending.append(query_async(node_info, "FIND_NODE", target_id))
                    queried.add(node_info.node_id)

        # Limit concurrent queries
        while len(pending) < alpha and len(closest) > len(queried):
            next_node = get_next_unqueried(closest, queried)
            if next_node:
                pending.append(query_async(next_node, "FIND_NODE", target_id))
                queried.add(next_node.node_id)
            else:
                break

    return closest[:k]
```

### STORE

Store a key-value pair in the DHT.

**Storage Rules:**
- Values stored at k closest nodes to the key
- Each node stores values where key is "close" to node ID
- TTL (time-to-live) for each value
- Periodic republishing before expiry

**Request:**
```python
def store_request(key, value, ttl):
    # Find k closest nodes to key
    closest_nodes = lookup_node(key)

    # Send STORE to each
    for node in closest_nodes:
        send_message(node, {
            "type": "STORE",
            "key": key,
            "value": value,
            "ttl": ttl,
            "signature": sign(key + value + ttl)
        })
```

**Storage:**
```python
class DHTStorage:
    def __init__(self):
        self.data = {}  # key -> (value, expiry, signature)

    def store(self, key, value, ttl, signature):
        # Only store if key is "close" to our node ID
        if not is_responsible_for(key):
            return False

        # Verify signature
        if not verify_signature(key, value, ttl, signature):
            return False

        # Store with expiration
        expiry = time.now() + ttl
        self.data[key] = (value, expiry, signature)

        return True

    def is_responsible_for(self, key):
        # Node is responsible if key distance < threshold
        dist = distance(self.node_id, key)
        threshold = 2^256 / total_network_size_estimate
        return dist < threshold
```

**Republishing:**
```python
# Republish stored values before expiry
REPUBLISH_INTERVAL = 3600  # 1 hour

def republish_loop():
    while True:
        time.sleep(REPUBLISH_INTERVAL)

        for key, (value, expiry, signature) in storage.data.items():
            if expiry - time.now() < 2 * REPUBLISH_INTERVAL:
                # Find new closest nodes (topology may have changed)
                closest_nodes = lookup_node(key)

                # Republish to k closest
                for node in closest_nodes:
                    store_at_node(node, key, value, expiry - time.now())
```

### FIND_VALUE

Retrieve a value from the DHT.

**Request:**
```python
def find_value(key):
    """Find value for key in DHT"""

    # Start with k closest nodes
    closest = get_k_closest_nodes(key, k=20)
    queried = set()
    pending = []

    alpha = 3
    for node in closest[:alpha]:
        pending.append(query_async(node, "FIND_VALUE", key))
        queried.add(node.node_id)

    while pending:
        response = await_any(pending)
        pending.remove(response)

        # Check if value found
        if response.type == "VALUE_FOUND":
            # Verify signature
            if verify_value_signature(response.value):
                # Cache at closest node that didn't have it
                cache_value(closest[0], key, response.value)
                return response.value

        # Otherwise treat as FIND_NODE response
        for node_info in response.nodes:
            if node_info.node_id not in queried:
                insert_sorted(closest, node_info, key)

                if len(queried) < k or is_closer(node_info, furthest_queried, key):
                    pending.append(query_async(node_info, "FIND_VALUE", key))
                    queried.add(node_info.node_id)

        # Continue searching
        while len(pending) < alpha and len(closest) > len(queried):
            next_node = get_next_unqueried(closest, queried)
            if next_node:
                pending.append(query_async(next_node, "FIND_VALUE", key))
                queried.add(next_node.node_id)
            else:
                break

    # Value not found
    return None
```

## DHT Data Types

### Node Records

Stored in DHT for node discovery:

```python
NodeRecord = {
    "node_id": bytes32,
    "public_key": bytes32,
    "adapters": [
        {
            "type": "ethernet",
            "address": "192.168.1.100:4001"
        },
        {
            "type": "lora",
            "address": "0x12345678"
        }
    ],
    "location": {  # Optional
        "lat": 37.7749,
        "lon": -122.4194,
        "accuracy": 100.0  # meters
    },
    "capabilities": {
        "relay": true,
        "cache": true,
        "i2p": true
    },
    "timestamp": 1636721234567,
    "signature": bytes64  # Signed by node's private key
}

Key: BLAKE2b-256("node:" + node_id)
```

### Route Records

Performance metrics for routes between node pairs:

```python
RouteRecord = {
    "source_node": bytes32,
    "dest_node": bytes32,
    "adapter": "lora",
    "metrics": {
        "latency_ms": 245.5,
        "bandwidth_bps": 12500,
        "reliability": 0.95,  # 95% success rate
        "last_test": 1636721234567,
        "sample_count": 100
    },
    "timestamp": 1636721234567,
    "signature": bytes64  # Signed by source node
}

Key: BLAKE2b-256("route:" + source_node + dest_node + adapter)
```

### Cached Messages

Messages for offline nodes:

```python
CachedMessage = {
    "message_id": bytes16,
    "dest_node": bytes32,
    "encrypted_payload": bytes,
    "priority": 128,
    "expires_at": 1636807634567,
    "cache_node": bytes32,  # Node storing this cache
    "timestamp": 1636721234567
}

Key: BLAKE2b-256("cache:" + dest_node + message_id)
```

## Routing Algorithms

### Direct Route

When nodes have direct connectivity:

```python
def route_direct(destination_node_id, message):
    # Lookup destination node in DHT
    node_record = dht.find_value("node:" + destination_node_id)

    if not node_record:
        return Error("Node not found")

    # Get available adapters
    common_adapters = intersect(local_adapters, node_record.adapters)

    if not common_adapters:
        return Error("No common network adapters")

    # Select best adapter based on metrics
    best_adapter = select_best_adapter(
        common_adapters,
        destination_node_id,
        message.priority
    )

    # Send message
    return send_via_adapter(best_adapter, destination_node_id, message)
```

### Multi-Hop Routing

When direct route unavailable:

```python
def route_multihop(destination_node_id, message):
    # Check TTL
    if message.ttl <= 0:
        return Error("TTL exceeded")

    # Lookup destination in DHT
    node_record = dht.find_value("node:" + destination_node_id)

    if not node_record:
        # Cache message for later delivery
        cache_message(destination_node_id, message)
        return "Cached"

    # Find relay nodes closer to destination
    closer_nodes = dht.lookup_node(destination_node_id)

    for relay_node in closer_nodes:
        # Skip if we are closer than relay
        if distance(our_id, destination_node_id) < distance(relay_node.id, destination_node_id):
            continue

        # Check if relay can reach destination
        route_key = "route:" + relay_node.id + destination_node_id
        route_record = dht.find_value(route_key)

        if route_record and route_record.metrics.reliability > 0.7:
            # Forward to relay
            message.ttl -= 1
            return send_to_relay(relay_node, message)

    # No relay found - cache message
    cache_message(destination_node_id, message)
    return "Cached"
```

### Geographic Routing

When location data available:

```python
def route_geographic(destination_node_id, message):
    # Get destination location
    dest_node = dht.find_value("node:" + destination_node_id)

    if not dest_node or not dest_node.location:
        # Fall back to DHT routing
        return route_multihop(destination_node_id, message)

    dest_location = dest_node.location

    # Find nodes closer to destination geographically
    nearby_nodes = find_nodes_near_location(
        dest_location.lat,
        dest_location.lon,
        radius_km=50
    )

    # Sort by geographic distance
    nearby_nodes.sort(key=lambda n: haversine_distance(
        our_location, n.location
    ))

    # Try to route through geographically closer node
    for node in nearby_nodes:
        if can_reach(node):
            message.ttl -= 1
            return send_to_relay(node, message)

    # Fall back to DHT routing
    return route_multihop(destination_node_id, message)
```

## Message Caching

### Store-and-Forward

When destination is offline:

```python
def cache_message_for_offline_node(dest_node_id, message):
    # Encrypt message (if not already)
    if not message.encrypted:
        encrypted_msg = encrypt_message(message)
    else:
        encrypted_msg = message

    # Create cache record
    cache_record = {
        "message_id": message.id,
        "dest_node": dest_node_id,
        "encrypted_payload": encrypted_msg.payload,
        "priority": message.priority,
        "expires_at": time.now() + message.ttl * 1000,
        "cache_node": our_node_id,
        "timestamp": time.now()
    }

    # Store in DHT at nodes close to destination
    cache_key = "cache:" + dest_node_id + message.id
    dht.store(cache_key, cache_record, ttl=message.ttl)

    # Also store locally for faster delivery
    local_cache.store(cache_key, cache_record)
```

### Cache Retrieval

When node comes online:

```python
def retrieve_cached_messages(node_id):
    # Query DHT for cached messages
    prefix = "cache:" + node_id
    cached = dht.find_values_by_prefix(prefix)

    messages = []
    for cache_record in cached:
        # Decrypt message
        message = decrypt_message(
            cache_record.encrypted_payload,
            get_shared_key(cache_record.source_node)
        )

        # Check expiration
        if cache_record.expires_at > time.now():
            messages.append(message)

        # Delete from DHT
        dht.delete(cache_record.key)

    return messages
```

### Primary Node Caching

User's designated primary node caches all messages:

```python
def forward_to_primary(user_id, message):
    # Lookup user's primary node
    user_record = dht.find_value("user:" + user_id)

    if not user_record or not user_record.primary_node:
        return Error("No primary node configured")

    primary_node_id = user_record.primary_node

    # Forward message to primary node
    route_direct(primary_node_id, message)

    # Primary node caches until user retrieves
```

## DHT Security

### Sybil Attack Mitigation

**Proof of Work (PoW) for Node IDs:**
```python
def generate_node_id_with_pow(public_key, difficulty=20):
    nonce = 0
    while True:
        candidate = BLAKE2b(public_key + nonce)

        # Check if first 'difficulty' bits are zero
        if count_leading_zeros(candidate) >= difficulty:
            return candidate, nonce

        nonce += 1
```

**Reputation System:**
```python
class NodeReputation:
    def __init__(self):
        self.successful_relays = 0
        self.failed_relays = 0
        self.uptime = 0
        self.ledger_participation = 0

    def score(self):
        reliability = self.successful_relays / (self.successful_relays + self.failed_relays + 1)
        uptime_score = min(self.uptime / (90 * 86400), 1.0)  # Up to 90 days
        participation = min(self.ledger_participation / 1000, 1.0)

        return (reliability * 0.5 + uptime_score * 0.3 + participation * 0.2)

# Only trust nodes with reputation > threshold
MIN_REPUTATION = 0.5
```

### Eclipse Attack Prevention

**Diverse Routing Table:**
```python
def add_node_diverse(node_info):
    # Reject if too many nodes from same network
    same_network_count = count_nodes_in_network(node_info.network)

    if same_network_count > MAX_NODES_PER_NETWORK:
        return False

    # Add to routing table
    add_node(node_info)
```

**Bootstrap from Multiple Sources:**
```python
BOOTSTRAP_NODES = [
    "bootstrap1.myriadmesh.org",
    "bootstrap2.myriadmesh.org",
    "bootstrap3.myriadmesh.org",
    # ... geographically diverse set
]
```

### Poisoning Attack Prevention

**Signature Verification:**
```python
def store_value(key, value, signature):
    # Extract node ID from value
    node_id = value.node_id

    # Verify signature with node's public key
    public_key = get_public_key(node_id)

    if not verify_signature(public_key, key + value, signature):
        return Error("Invalid signature")

    # Store value
    storage.put(key, value)
```

**Value Majority:**
```python
def find_value_secure(key):
    # Query multiple nodes
    responses = query_multiple_nodes(key, count=20)

    # Group by value content
    value_counts = {}
    for response in responses:
        value_hash = BLAKE2b(response.value)
        if value_hash not in value_counts:
            value_counts[value_hash] = []
        value_counts[value_hash].append(response)

    # Return most common value (if > 50%)
    for value_hash, responses in value_counts.items():
        if len(responses) > len(responses) / 2:
            return responses[0].value

    return None
```

## DHT Optimization

### Parallel Lookups

```python
# Query multiple nodes in parallel
ALPHA = 3  # Concurrency factor

def lookup_parallel(target_id):
    closest = get_k_closest_nodes(target_id, k=20)
    queried = set()
    pending = []

    # Start with alpha parallel queries
    for i in range(min(ALPHA, len(closest))):
        pending.append(query_async(closest[i], target_id))
        queried.add(closest[i].node_id)

    # Continue until no closer nodes found
    # ...
```

### Caching

**Response Caching:**
```python
# Cache DHT responses for short period
response_cache = LRU_Cache(max_size=1000, ttl=60)

def find_value_cached(key):
    # Check cache first
    if key in response_cache:
        return response_cache[key]

    # Query DHT
    value = dht.find_value(key)

    # Cache result
    response_cache[key] = value

    return value
```

### Bucket Splitting

For high-density regions of keyspace:

```python
def split_bucket(bucket_index):
    # Only split if bucket is full and contains our node ID range
    bucket = buckets[bucket_index]

    if len(bucket.nodes) < k:
        return

    if not bucket_contains_own_id(bucket_index):
        return

    # Create two new buckets
    lower_bucket = KBucket()
    upper_bucket = KBucket()

    # Split based on next bit
    for node in bucket.nodes:
        if get_bit(node.node_id, bucket_index + 1) == 0:
            lower_bucket.add(node)
        else:
            upper_bucket.add(node)

    # Replace old bucket with two new buckets
    buckets[bucket_index] = lower_bucket
    buckets.insert(bucket_index + 1, upper_bucket)
```

## DHT Monitoring and Maintenance

### Health Checks

```python
# Periodic health checks
CHECK_INTERVAL = 300  # 5 minutes

def health_check_loop():
    while True:
        time.sleep(CHECK_INTERVAL)

        for bucket in buckets:
            for node in bucket.nodes:
                # Ping node
                if not ping(node, timeout=5):
                    node.failures += 1

                    # Remove after 3 failures
                    if node.failures >= 3:
                        bucket.remove(node)

                        # Promote from replacement cache
                        if bucket.replacement_cache:
                            bucket.add(bucket.replacement_cache.pop(0))
                else:
                    node.failures = 0
                    node.last_seen = time.now()
```

### Statistics

```python
class DHTStats:
    total_nodes: int
    buckets_populated: int
    total_lookups: int
    successful_lookups: int
    average_lookup_time_ms: float
    total_stored_keys: int
    cache_hit_rate: float
```

## Next Steps

- [Protocol Message Formats](specification.md)
- [Network Performance Testing](network-adapters.md#testing)
- [Implementation Guide](../implementation/dht-implementation.md)
