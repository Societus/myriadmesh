package com.myriadmesh.android.domain.model

data class ApplianceInfo(
    val nodeId: String,
    val nodeName: String,
    val version: String,
    val applianceMode: Boolean,
    val capabilities: ApplianceCapabilities,
    val pairing: PairingInfo,
    val stats: ApplianceStats
)

data class ApplianceCapabilities(
    val messageCaching: Boolean,
    val relay: Boolean,
    val bridge: Boolean,
    val maxCacheMessages: Int,
    val maxPairedDevices: Int
)

data class PairingInfo(
    val available: Boolean,
    val methods: List<String>,
    val requiresApproval: Boolean
)

data class ApplianceStats(
    val uptimeSecs: Long,
    val pairedDevices: Int,
    val cachedMessages: Int,
    val adaptersOnline: Int
)

data class PairedDevice(
    val deviceId: String,
    val deviceNodeId: String,
    val publicKey: String,
    val pairedAt: Long,
    val lastSeen: Long,
    val preferences: DevicePreferences?
)

data class DevicePreferences(
    val messageCaching: Boolean,
    val cachePriority: String,
    val routingPreference: String
)
