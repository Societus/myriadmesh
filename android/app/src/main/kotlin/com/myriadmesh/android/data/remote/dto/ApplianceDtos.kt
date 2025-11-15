package com.myriadmesh.android.data.remote.dto

import com.google.gson.annotations.SerializedName

// ===== Information & Status DTOs =====

data class ApplianceInfoDto(
    @SerializedName("node_id") val nodeId: String,
    @SerializedName("node_name") val nodeName: String,
    @SerializedName("version") val version: String,
    @SerializedName("appliance_mode") val applianceMode: Boolean,
    @SerializedName("capabilities") val capabilities: CapabilitiesDto,
    @SerializedName("pairing") val pairing: PairingInfoDto,
    @SerializedName("stats") val stats: StatsDto
)

data class CapabilitiesDto(
    @SerializedName("message_caching") val messageCaching: Boolean,
    @SerializedName("relay") val relay: Boolean,
    @SerializedName("bridge") val bridge: Boolean,
    @SerializedName("max_cache_messages") val maxCacheMessages: Int,
    @SerializedName("max_paired_devices") val maxPairedDevices: Int
)

data class PairingInfoDto(
    @SerializedName("available") val available: Boolean,
    @SerializedName("methods") val methods: List<String>,
    @SerializedName("requires_approval") val requiresApproval: Boolean
)

data class StatsDto(
    @SerializedName("uptime_secs") val uptimeSecs: Long,
    @SerializedName("paired_devices") val pairedDevices: Int,
    @SerializedName("cached_messages") val cachedMessages: Int,
    @SerializedName("adapters_online") val adaptersOnline: Int
)

data class ApplianceStatsDto(
    @SerializedName("node_id") val nodeId: String,
    @SerializedName("uptime_secs") val uptimeSecs: Long,
    @SerializedName("total_paired_devices") val totalPairedDevices: Int,
    @SerializedName("total_cached_messages") val totalCachedMessages: Int,
    @SerializedName("cache_by_priority") val cacheByPriority: Map<String, Int>,
    @SerializedName("adapters_status") val adaptersStatus: Map<String, String>
)

// ===== Pairing DTOs =====

data class PairingRequestDto(
    @SerializedName("device_id") val deviceId: String,
    @SerializedName("device_node_id") val deviceNodeId: String,
    @SerializedName("public_key") val publicKey: String,
    @SerializedName("pairing_method") val pairingMethod: String = "qr_code"
)

data class PairingResponseDto(
    @SerializedName("pairing_token") val pairingToken: String,
    @SerializedName("challenge") val challenge: String,
    @SerializedName("expires_at") val expiresAt: Long
)

data class CompletePairingDto(
    @SerializedName("pairing_token") val pairingToken: String,
    @SerializedName("challenge_response") val challengeResponse: String
)

data class CompletePairingResponseDto(
    @SerializedName("session_token") val sessionToken: String,
    @SerializedName("device_id") val deviceId: String,
    @SerializedName("expires_at") val expiresAt: Long
)

// ===== Device Management DTOs =====

data class DevicesResponseDto(
    @SerializedName("devices") val devices: List<PairedDeviceDto>
)

data class PairedDeviceDto(
    @SerializedName("device_id") val deviceId: String,
    @SerializedName("device_node_id") val deviceNodeId: String,
    @SerializedName("public_key") val publicKey: String,
    @SerializedName("paired_at") val pairedAt: Long,
    @SerializedName("last_seen") val lastSeen: Long,
    @SerializedName("preferences") val preferences: DevicePreferencesDto?
)

data class DevicePreferencesDto(
    @SerializedName("message_caching") val messageCaching: Boolean? = null,
    @SerializedName("cache_priority") val cachePriority: String? = null,
    @SerializedName("routing_preference") val routingPreference: String? = null
)

// ===== Message Caching DTOs =====

data class StoreCachedMessageDto(
    @SerializedName("device_id") val deviceId: String,
    @SerializedName("direction") val direction: String,
    @SerializedName("priority") val priority: Int,
    @SerializedName("payload") val payload: String, // Base64 encoded
    @SerializedName("metadata") val metadata: Map<String, String>? = null,
    @SerializedName("ttl_days") val ttlDays: Int? = null
)

data class RetrieveCachedMessagesDto(
    @SerializedName("messages") val messages: List<CachedMessageDto>
)

data class CachedMessageDto(
    @SerializedName("message_id") val messageId: String,
    @SerializedName("device_id") val deviceId: String,
    @SerializedName("direction") val direction: String,
    @SerializedName("priority") val priority: Int,
    @SerializedName("payload") val payload: String, // Base64 encoded
    @SerializedName("metadata") val metadata: Map<String, String>? = null,
    @SerializedName("received_at") val receivedAt: Long,
    @SerializedName("expires_at") val expiresAt: Long,
    @SerializedName("delivered") val delivered: Boolean
)

data class MarkDeliveredDto(
    @SerializedName("message_ids") val messageIds: List<String>
)

data class CacheStatsDto(
    @SerializedName("device_id") val deviceId: String,
    @SerializedName("total_cached") val totalCached: Int,
    @SerializedName("by_priority") val byPriority: Map<String, Int>,
    @SerializedName("undelivered") val undelivered: Int,
    @SerializedName("oldest_message_age_secs") val oldestMessageAgeSecs: Long
)
