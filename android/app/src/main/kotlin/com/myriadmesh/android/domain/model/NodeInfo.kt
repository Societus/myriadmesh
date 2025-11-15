package com.myriadmesh.android.domain.model

data class NodeInfo(
    val nodeId: String,
    val publicKey: String,
    val adapters: List<AdapterInfo>,
    val reputation: Int,
    val uptime: Long
)

data class AdapterInfo(
    val name: String,
    val type: AdapterType,
    val status: AdapterStatus,
    val metrics: AdapterMetrics?
)

enum class AdapterType {
    ETHERNET,
    WIFI,
    WIFI_DIRECT,
    BLUETOOTH_CLASSIC,
    BLUETOOTH_LE,
    CELLULAR,
    I2P,
    UNKNOWN
}

enum class AdapterStatus {
    ONLINE,
    OFFLINE,
    ERROR,
    INITIALIZING
}

data class AdapterMetrics(
    val latencyMs: Long,
    val bandwidthBps: Long,
    val packetLoss: Double,
    val reliability: Double
)
