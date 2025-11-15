# Appliance API Guide

This guide provides examples for using the MyriadNode Appliance API endpoints. The appliance functionality enables MyriadNodes to act as gateways for mobile devices, providing message caching, configuration synchronization, and relay services.

## Table of Contents

1. [Configuration](#configuration)
2. [API Endpoints](#api-endpoints)
3. [Pairing Workflow](#pairing-workflow)
4. [Device Management](#device-management)
5. [Message Caching](#message-caching)
6. [Error Handling](#error-handling)

---

## Configuration

To enable appliance mode, add this to your `config.yaml`:

```yaml
appliance:
  enabled: true
  max_paired_devices: 10
  message_caching: true
  max_cache_messages_per_device: 1000
  max_total_cache_messages: 10000
  enable_relay: true
  enable_bridge: true
  require_pairing_approval: true
  pairing_methods:
    - qr_code
    - pin
  mdns_enabled: true
  dht_advertisement: true
```

Start your MyriadNode with appliance mode enabled:

```bash
myriadnode --config /path/to/config.yaml
```

---

## API Endpoints

Base URL: `http://localhost:3030` (or your configured API bind address)

### 1. Get Appliance Information

Get appliance capabilities and current status.

**Request:**
```bash
curl -X GET http://localhost:3030/api/appliance/info
```

**Response:**
```json
{
  "max_paired_devices": 10,
  "current_paired_devices": 2,
  "message_caching": true,
  "max_cache_messages_per_device": 1000,
  "relay_enabled": true,
  "bridge_enabled": true,
  "pairing_available": true,
  "pairing_methods": ["qr_code", "pin"]
}
```

---

### 2. Get Appliance Statistics

Get detailed statistics about the appliance.

**Request:**
```bash
curl -X GET http://localhost:3030/api/appliance/stats
```

**Response:**
```json
{
  "paired_devices": 2,
  "active_devices": 2,
  "total_cached_messages": 47,
  "pending_approvals": 0
}
```

---

## Pairing Workflow

The pairing process involves 4 steps:

### Step 1: Initiate Pairing Request (Mobile Device)

The mobile device requests a pairing token from the appliance.

**Request:**
```bash
curl -X POST http://localhost:3030/api/appliance/pair/request \
  -H "Content-Type: application/json" \
  -d '{
    "device_id": "my-phone-abc123",
    "node_id": "64-byte-hex-node-id-here...",
    "public_key": "32-byte-hex-ed25519-public-key-here...",
    "pairing_method": "qr_code"
  }'
```

**Response:**
```json
{
  "token": "550e8400-e29b-41d4-a716-446655440000",
  "challenge": "hex-encoded-32-byte-challenge",
  "node_id": "appliance-node-id",
  "timestamp": 1699564800,
  "expires_at": 1699565100,
  "signature": "hex-encoded-signature"
}
```

**QR Code Display:**
The appliance can encode this token as a QR code for the mobile app to scan:

```bash
# Example using qrencode (install: apt-get install qrencode)
echo '{"token":"550e8400-e29b-41d4-a716-446655440000","challenge":"..."}' | qrencode -t UTF8
```

---

### Step 2: Approve Pairing (Appliance Owner)

The appliance owner approves the pairing request (if `require_pairing_approval: true`).

**Request:**
```bash
curl -X POST http://localhost:3030/api/appliance/pair/approve/550e8400-e29b-41d4-a716-446655440000
```

**Response:**
```json
{
  "status": "approved",
  "message": "Pairing request approved. Device can now complete pairing."
}
```

**Alternative - Reject Pairing:**
```bash
curl -X POST http://localhost:3030/api/appliance/pair/reject/550e8400-e29b-41d4-a716-446655440000
```

---

### Step 3: Complete Pairing (Mobile Device)

The mobile device completes pairing by providing the signed challenge.

**Request:**
```bash
curl -X POST http://localhost:3030/api/appliance/pair/complete \
  -H "Content-Type: application/json" \
  -d '{
    "token": "550e8400-e29b-41d4-a716-446655440000",
    "device_id": "my-phone-abc123",
    "challenge_response": "hex-encoded-signature-of-challenge"
  }'
```

**Response:**
```json
{
  "session_token": "persistent-session-token-for-authentication",
  "device_id": "my-phone-abc123",
  "paired_at": 1699564900,
  "preferences": {
    "routing": {
      "default_policy": "balanced",
      "adapter_priority": ["i2p", "wifi", "cellular"],
      "qos_class_default": "normal",
      "multipath_enabled": true,
      "geographic_routing_enabled": false
    },
    "messages": {
      "cache_on_appliance": true,
      "cache_priority_default": "normal",
      "auto_forward_to_appliance": true,
      "store_and_forward": true,
      "ttl_days": 7
    },
    "power": {
      "offload_dht_to_appliance": true,
      "offload_ledger_sync": true,
      "mobile_heartbeat_interval": 300,
      "appliance_as_proxy": true
    },
    "privacy": {
      "always_use_i2p_via_appliance": false,
      "clearnet_allowed_on_mobile": true,
      "require_appliance_for_sensitive": true,
      "trusted_nodes_only": false
    }
  }
}
```

**Important:** Save the `session_token` securely on the mobile device. Use it for all subsequent API requests via the `X-Session-Token` header.

---

## Device Management

### List Paired Devices

**Request:**
```bash
curl -X GET http://localhost:3030/api/appliance/devices
```

**Response:**
```json
[
  {
    "device_id": "my-phone-abc123",
    "node_id": "64-byte-hex-node-id",
    "paired_at": 1699564900,
    "last_seen": 1699568500,
    "active": true,
    "cached_messages": 23
  },
  {
    "device_id": "my-tablet-xyz789",
    "node_id": "another-64-byte-hex-node-id",
    "paired_at": 1699550000,
    "last_seen": 1699567800,
    "active": true,
    "cached_messages": 5
  }
]
```

---

### Get Device Details

**Request:**
```bash
curl -X GET http://localhost:3030/api/appliance/devices/my-phone-abc123 \
  -H "X-Session-Token: your-session-token"
```

**Response:**
```json
{
  "device_id": "my-phone-abc123",
  "node_id": "64-byte-hex-node-id",
  "paired_at": 1699564900,
  "last_seen": 1699568500,
  "active": true,
  "cached_messages": 23
}
```

---

### Update Device Preferences

Mobile devices can update their preferences at any time.

**Request:**
```bash
curl -X POST http://localhost:3030/api/appliance/devices/my-phone-abc123/preferences \
  -H "Content-Type: application/json" \
  -H "X-Session-Token: your-session-token" \
  -d '{
    "routing": {
      "default_policy": "privacy",
      "adapter_priority": ["i2p"],
      "qos_class_default": "high",
      "multipath_enabled": false,
      "geographic_routing_enabled": false
    },
    "messages": {
      "cache_on_appliance": true,
      "cache_priority_default": "high",
      "auto_forward_to_appliance": true,
      "store_and_forward": true,
      "ttl_days": 14
    },
    "power": {
      "offload_dht_to_appliance": true,
      "offload_ledger_sync": true,
      "mobile_heartbeat_interval": 600,
      "appliance_as_proxy": true
    },
    "privacy": {
      "always_use_i2p_via_appliance": true,
      "clearnet_allowed_on_mobile": false,
      "require_appliance_for_sensitive": true,
      "trusted_nodes_only": true
    }
  }'
```

**Response:**
```json
{
  "status": "success",
  "message": "Preferences updated successfully"
}
```

---

### Unpair Device

**Request:**
```bash
curl -X POST http://localhost:3030/api/appliance/devices/my-phone-abc123/unpair \
  -H "X-Session-Token: your-session-token"
```

**Response:**
```json
{
  "status": "success",
  "message": "Device unpaired successfully. All cached messages deleted."
}
```

---

## Message Caching

### Store Cached Message

When a mobile device is offline, messages can be cached on the appliance for later retrieval.

**Request:**
```bash
curl -X POST http://localhost:3030/api/appliance/cache/store \
  -H "Content-Type: application/json" \
  -H "X-Session-Token: your-session-token" \
  -d '{
    "device_id": "my-phone-abc123",
    "message_id": "msg-uuid-12345",
    "priority": 2,
    "payload": "base64-encoded-message-payload",
    "metadata": {
      "from": "sender-node-id",
      "to": "my-phone-node-id",
      "timestamp": 1699568600,
      "content_type": "text/plain"
    }
  }'
```

**Priority Levels:**
- `0` - Low (TTL: 3 days)
- `1` - Normal (TTL: 7 days, default)
- `2` - High (TTL: 14 days)
- `3` - Urgent (TTL: 7 days)

**Response:**
```json
{
  "status": "success",
  "message_id": "msg-uuid-12345",
  "expires_at": 1700173400
}
```

---

### Retrieve Cached Messages

Mobile devices retrieve their cached messages when they come back online.

**Request:**
```bash
curl -X GET "http://localhost:3030/api/appliance/cache/retrieve?device_id=my-phone-abc123&priority=1" \
  -H "X-Session-Token: your-session-token"
```

**Query Parameters:**
- `device_id` (required) - The device ID
- `priority` (optional) - Minimum priority level (0-3)
- `limit` (optional) - Maximum number of messages to retrieve (default: 100)

**Response:**
```json
{
  "messages": [
    {
      "message_id": "msg-uuid-12345",
      "device_id": "my-phone-abc123",
      "priority": 2,
      "payload": "base64-encoded-message-payload",
      "metadata": {
        "from": "sender-node-id",
        "to": "my-phone-node-id",
        "timestamp": 1699568600,
        "content_type": "text/plain"
      },
      "stored_at": 1699568600,
      "expires_at": 1700173400
    }
  ],
  "total_count": 1,
  "has_more": false
}
```

---

### Mark Messages as Delivered

After successfully retrieving messages, the mobile device should mark them as delivered so they can be removed from the cache.

**Request:**
```bash
curl -X POST http://localhost:3030/api/appliance/cache/delivered \
  -H "Content-Type: application/json" \
  -H "X-Session-Token: your-session-token" \
  -d '{
    "device_id": "my-phone-abc123",
    "message_ids": [
      "msg-uuid-12345",
      "msg-uuid-67890"
    ]
  }'
```

**Response:**
```json
{
  "status": "success",
  "marked_count": 2
}
```

---

### Get Cache Statistics

**Request:**
```bash
curl -X GET http://localhost:3030/api/appliance/cache/stats/my-phone-abc123 \
  -H "X-Session-Token: your-session-token"
```

**Response:**
```json
{
  "device_id": "my-phone-abc123",
  "total_messages": 23,
  "by_priority": {
    "urgent": 2,
    "high": 5,
    "normal": 14,
    "low": 2
  },
  "oldest_message": 1699550000,
  "newest_message": 1699568600
}
```

---

## Error Handling

### HTTP Status Codes

- `200 OK` - Request successful
- `201 Created` - Resource created successfully
- `400 Bad Request` - Invalid request parameters
- `401 Unauthorized` - Missing or invalid session token
- `404 Not Found` - Resource not found (or appliance mode disabled)
- `409 Conflict` - Duplicate resource or state conflict
- `429 Too Many Requests` - Rate limit exceeded
- `500 Internal Server Error` - Server error

### Error Response Format

```json
{
  "error": "DeviceNotFound",
  "message": "Device not found: my-phone-abc123",
  "code": 404
}
```

### Common Error Scenarios

**1. Appliance Mode Disabled**
```bash
# Response: 404 Not Found
{
  "error": "NotFound",
  "message": "Appliance mode is not enabled on this node"
}
```

**2. Invalid Session Token**
```bash
# Response: 401 Unauthorized
{
  "error": "Unauthorized",
  "message": "Invalid or expired session token"
}
```

**3. Pairing Token Expired**
```bash
# Response: 400 Bad Request
{
  "error": "PairingExpired",
  "message": "Pairing token has expired. Please request a new token."
}
```

**4. Cache Full**
```bash
# Response: 429 Too Many Requests
{
  "error": "CacheFull",
  "message": "Message cache is full. Please retrieve existing messages first."
}
```

**5. Maximum Devices Reached**
```bash
# Response: 409 Conflict
{
  "error": "MaxDevicesReached",
  "message": "Maximum paired devices limit (10) reached"
}
```

---

## Example: Complete Pairing Workflow

Here's a complete example of the pairing workflow from start to finish:

```bash
#!/bin/bash

APPLIANCE_URL="http://localhost:3030"
DEVICE_ID="my-phone-abc123"
NODE_ID="your-64-byte-hex-node-id"
PUBLIC_KEY="your-32-byte-hex-public-key"

# Step 1: Request pairing
echo "Step 1: Requesting pairing..."
PAIRING_RESPONSE=$(curl -s -X POST "$APPLIANCE_URL/api/appliance/pair/request" \
  -H "Content-Type: application/json" \
  -d "{
    \"device_id\": \"$DEVICE_ID\",
    \"node_id\": \"$NODE_ID\",
    \"public_key\": \"$PUBLIC_KEY\",
    \"pairing_method\": \"qr_code\"
  }")

echo "Pairing response: $PAIRING_RESPONSE"
TOKEN=$(echo $PAIRING_RESPONSE | jq -r '.token')
CHALLENGE=$(echo $PAIRING_RESPONSE | jq -r '.challenge')

echo "Token: $TOKEN"
echo "Challenge: $CHALLENGE"

# Step 2: Approve pairing (on appliance)
echo -e "\nStep 2: Approving pairing (manual step on appliance)..."
read -p "Press Enter after approving on appliance..."

# Step 3: Sign challenge (in real app, use Ed25519 signature)
# For this example, we'll assume you have a signature
echo -e "\nStep 3: Completing pairing..."
CHALLENGE_RESPONSE="your-signature-of-challenge-hex"

COMPLETE_RESPONSE=$(curl -s -X POST "$APPLIANCE_URL/api/appliance/pair/complete" \
  -H "Content-Type: application/json" \
  -d "{
    \"token\": \"$TOKEN\",
    \"device_id\": \"$DEVICE_ID\",
    \"challenge_response\": \"$CHALLENGE_RESPONSE\"
  }")

echo "Complete response: $COMPLETE_RESPONSE"
SESSION_TOKEN=$(echo $COMPLETE_RESPONSE | jq -r '.session_token')

echo "Session token: $SESSION_TOKEN"

# Step 4: Test authenticated request
echo -e "\nStep 4: Testing authenticated request..."
curl -s -X GET "$APPLIANCE_URL/api/appliance/devices/$DEVICE_ID" \
  -H "X-Session-Token: $SESSION_TOKEN" | jq .

echo -e "\nPairing complete!"
```

---

## Security Best Practices

1. **Always use HTTPS in production** - The examples use HTTP for local development only
2. **Store session tokens securely** - Use platform-specific secure storage (Keychain on iOS, KeyStore on Android)
3. **Implement token rotation** - Refresh session tokens periodically
4. **Enable pairing approval** - Set `require_pairing_approval: true` to prevent unauthorized pairing
5. **Monitor paired devices** - Regularly review paired devices and unpair unused ones
6. **Use message encryption** - Encrypt message payloads before caching
7. **Implement rate limiting** - On the mobile side to avoid overwhelming the appliance
8. **Validate signatures** - Always verify cryptographic signatures during pairing

---

## Next Steps

- Read the [Architecture Documentation](ANDROID_APPLIANCE_ARCHITECTURE.md) for design details
- Review the [Configuration Guide](../README.md) for all configuration options
- Check the [API Reference](../api/README.md) for complete endpoint documentation
- See the [Security Model](ANDROID_APPLIANCE_ARCHITECTURE.md#6-security-model) for security considerations

---

**Version:** Phase 4.5 - Android-Appliance Infrastructure (Foundation)
**Last Updated:** 2024-01-15
