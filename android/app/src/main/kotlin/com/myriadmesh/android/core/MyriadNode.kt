package com.myriadmesh.android.core

import timber.log.Timber
import java.io.File

/**
 * JNI bridge to the Rust MyriadNode implementation.
 * This class provides access to the core mesh networking functionality.
 */
class MyriadNode private constructor(private val nodePtr: Long) {

    /**
     * Start the node and begin mesh networking operations.
     */
    fun start(): Boolean {
        return try {
            nativeStart(nodePtr)
        } catch (e: Exception) {
            Timber.e(e, "Failed to start MyriadNode")
            false
        }
    }

    /**
     * Stop the node and cleanup resources.
     */
    fun stop(): Boolean {
        return try {
            nativeStop(nodePtr)
        } catch (e: Exception) {
            Timber.e(e, "Failed to stop MyriadNode")
            false
        }
    }

    /**
     * Send a message to a destination node.
     *
     * @param destination The destination node ID
     * @param payload The message payload
     * @param priority Message priority (0=low, 1=normal, 2=high, 3=urgent)
     * @return true if message was queued successfully
     */
    fun sendMessage(destination: String, payload: ByteArray, priority: Int = 1): Boolean {
        return try {
            nativeSendMessage(nodePtr, destination, payload, priority)
        } catch (e: Exception) {
            Timber.e(e, "Failed to send message")
            false
        }
    }

    /**
     * Get the node's public ID.
     */
    fun getNodeId(): String {
        return nativeGetNodeId(nodePtr)
    }

    /**
     * Get current node status as JSON string.
     */
    fun getStatus(): String {
        return nativeGetStatus(nodePtr)
    }

    /**
     * Cleanup native resources.
     */
    fun destroy() {
        try {
            nativeDestroy(nodePtr)
        } catch (e: Exception) {
            Timber.e(e, "Failed to destroy MyriadNode")
        }
    }

    companion object {
        private const val LIBRARY_NAME = "myriadmesh_android"

        init {
            try {
                System.loadLibrary(LIBRARY_NAME)
                Timber.d("Loaded native library: $LIBRARY_NAME")
            } catch (e: UnsatisfiedLinkError) {
                Timber.e(e, "Failed to load native library: $LIBRARY_NAME")
                throw e
            }
        }

        /**
         * Initialize a new MyriadNode instance.
         *
         * @param configPath Path to the configuration file
         * @param dataDir Path to the data directory
         * @return A new MyriadNode instance or null if initialization failed
         */
        fun initialize(configPath: String, dataDir: String): MyriadNode? {
            return try {
                val configFile = File(configPath)
                if (!configFile.exists()) {
                    Timber.e("Config file does not exist: $configPath")
                    return null
                }

                val dataDirFile = File(dataDir)
                if (!dataDirFile.exists()) {
                    dataDirFile.mkdirs()
                }

                val nodePtr = nativeInit(configPath, dataDir)
                if (nodePtr == 0L) {
                    Timber.e("Native initialization returned null pointer")
                    null
                } else {
                    MyriadNode(nodePtr)
                }
            } catch (e: Exception) {
                Timber.e(e, "Failed to initialize MyriadNode")
                null
            }
        }

        // Native method declarations
        @JvmStatic
        private external fun nativeInit(configPath: String, dataDir: String): Long

        @JvmStatic
        private external fun nativeStart(nodePtr: Long): Boolean

        @JvmStatic
        private external fun nativeStop(nodePtr: Long): Boolean

        @JvmStatic
        private external fun nativeSendMessage(
            nodePtr: Long,
            destination: String,
            payload: ByteArray,
            priority: Int
        ): Boolean

        @JvmStatic
        private external fun nativeGetNodeId(nodePtr: Long): String

        @JvmStatic
        private external fun nativeGetStatus(nodePtr: Long): String

        @JvmStatic
        private external fun nativeDestroy(nodePtr: Long)
    }
}
