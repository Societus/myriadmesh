package com.myriadmesh.android.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.myriadmesh.android.MyriadMeshApplication
import com.myriadmesh.android.R
import com.myriadmesh.android.presentation.MainActivity
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.*
import timber.log.Timber
import javax.inject.Inject
import javax.inject.Named
import java.io.File

@AndroidEntryPoint
class MyriadMeshService : Service() {

    @Inject
    @Named("dataDir")
    lateinit var dataDir: File

    private val serviceScope = CoroutineScope(Dispatchers.Default + SupervisorJob())
    private var isRunning = false

    override fun onCreate() {
        super.onCreate()
        Timber.d("MyriadMesh Service created")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Timber.d("MyriadMesh Service started")

        if (!isRunning) {
            isRunning = true
            startForeground(NOTIFICATION_ID, createNotification())
            startMeshNetworking()
        }

        return START_STICKY
    }

    private fun startMeshNetworking() {
        serviceScope.launch {
            try {
                Timber.d("Starting mesh networking...")
                // TODO: Initialize MyriadNode via JNI
                // val node = MyriadNode.initialize(configPath, dataDir.absolutePath)
                // node?.start()

                // For now, just keep the service running
                while (isActive) {
                    delay(10000) // Heartbeat every 10 seconds
                    Timber.d("Mesh service running...")
                }
            } catch (e: Exception) {
                Timber.e(e, "Error in mesh networking")
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        Timber.d("MyriadMesh Service destroyed")
        isRunning = false
        serviceScope.cancel()
    }

    override fun onBind(intent: Intent?): IBinder? {
        return null
    }

    private fun createNotification(): Notification {
        val notificationIntent = Intent(this, MainActivity::class.java)
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            notificationIntent,
            PendingIntent.FLAG_IMMUTABLE
        )

        return NotificationCompat.Builder(this, MyriadMeshApplication.CHANNEL_SERVICE)
            .setContentTitle(getString(R.string.service_notification_title))
            .setContentText(getString(R.string.service_notification_text))
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .build()
    }

    companion object {
        private const val NOTIFICATION_ID = 1001
    }
}
