package com.myriadmesh.android.domain.model

data class Message(
    val messageId: String,
    val from: String,
    val to: String,
    val payload: ByteArray,
    val timestamp: Long,
    val priority: MessagePriority,
    val status: MessageStatus,
    val metadata: Map<String, String>? = null
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (javaClass != other?.javaClass) return false

        other as Message

        if (messageId != other.messageId) return false
        if (!payload.contentEquals(other.payload)) return false

        return true
    }

    override fun hashCode(): Int {
        var result = messageId.hashCode()
        result = 31 * result + payload.contentHashCode()
        return result
    }
}

enum class MessagePriority(val value: Int) {
    LOW(0),
    NORMAL(1),
    HIGH(2),
    URGENT(3)
}

enum class MessageStatus {
    PENDING,
    SENDING,
    SENT,
    DELIVERED,
    FAILED
}

data class CachedMessage(
    val messageId: String,
    val deviceId: String,
    val direction: MessageDirection,
    val priority: MessagePriority,
    val payload: ByteArray,
    val metadata: Map<String, String>?,
    val receivedAt: Long,
    val expiresAt: Long,
    val delivered: Boolean
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (javaClass != other?.javaClass) return false

        other as CachedMessage

        if (messageId != other.messageId) return false
        if (!payload.contentEquals(other.payload)) return false

        return true
    }

    override fun hashCode(): Int {
        var result = messageId.hashCode()
        result = 31 * result + payload.contentHashCode()
        return result
    }
}

enum class MessageDirection {
    INBOUND,
    OUTBOUND
}
