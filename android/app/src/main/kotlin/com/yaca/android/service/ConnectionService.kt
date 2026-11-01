package com.yaca.android.service

import android.app.Notification
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleService
import com.yaca.android.AppContainer
import com.yaca.android.R
import com.yaca.android.YacaApplication
import com.yaca.android.notify.Channels
import com.yaca.android.notify.ChatIntents
import com.yaca.android.notify.postNotification
import com.yaca.core.client.ConnectionState
import com.yaca.core.client.HostRecord
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.launch

/**
 * Keeps the computers connected while the app is in the background, so their questions reach the
 * phone with the screen off. It is a foreground service with the remote-messaging type, and its
 * ongoing notification says which computers are connected, with a Disconnect action.
 *
 * It runs only while the app is in the background and the setting allows it; [ConnectionLifecycle]
 * starts and stops it. The connections themselves belong to the container, not to this service: the
 * service only keeps the process alive and tells the human what is connected.
 */
class ConnectionService : LifecycleService() {
    private var statusJob: Job? = null

    private val container: AppContainer
        get() = (application as YacaApplication).container

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        if (intent?.action == ACTION_DISCONNECT) {
            container.disconnectAll()
            stopForegroundAndSelf()
            return START_NOT_STICKY
        }
        // Android requires startForeground within five seconds of a foreground start, so the
        // notification is shown at once; the status job then updates it in place.
        ServiceCompat.startForeground(this, NOTIFICATION_ID, notification(getString(R.string.notify_connection_connecting)), foregroundType())
        if (statusJob == null) statusJob = watchConnections()
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        statusJob?.cancel()
        statusJob = null
        super.onDestroy()
    }

    /** Re-draws the notification whenever a computer connects or drops; with no computer paired the service has no reason to run. */
    private fun watchConnections(): Job = container.scope.launch {
        container.hosts.filterNotNull().collectLatest { records ->
            if (records.isEmpty()) {
                stopForegroundAndSelf()
                return@collectLatest
            }
            val states = records.map { container.clientFor(it).state }
            combine(states) { values -> records.zip(values.toList()) }.collect { pairs ->
                postNotification(this@ConnectionService, null, NOTIFICATION_ID, notification(statusText(pairs)))
            }
        }
    }

    private fun statusText(pairs: List<Pair<HostRecord, ConnectionState>>): String {
        val names = pairs.mapNotNull { (_, state) -> (state as? ConnectionState.Connected)?.host?.name }
        return if (names.isEmpty()) {
            getString(R.string.notify_connection_connecting)
        } else {
            getString(R.string.notify_connection_connected, names.joinToString(", "))
        }
    }

    private fun notification(text: String): Notification =
        NotificationCompat.Builder(this, Channels.CONNECTION)
            .setSmallIcon(R.drawable.ic_stat_yaca)
            .setContentTitle(getString(R.string.app_name))
            .setContentText(text)
            .setContentIntent(ChatIntents.openApp(this))
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .addAction(0, getString(R.string.notify_connection_disconnect), disconnectIntent())
            .build()

    private fun disconnectIntent(): PendingIntent {
        val intent = Intent(this, ConnectionService::class.java).setAction(ACTION_DISCONNECT)
        return PendingIntent.getService(this, REQUEST_DISCONNECT, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    private fun stopForegroundAndSelf() {
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    /**
     * The type passed to startForeground. Android 14 added the remote-messaging type and from then
     * on requires a type the manifest declares, so passing none there would throw. Below it types
     * are optional and the manifest's attribute is all there is.
     */
    private fun foregroundType(): Int =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            ServiceInfo.FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING
        } else {
            0
        }

    companion object {
        const val ACTION_DISCONNECT = "com.yaca.android.action.DISCONNECT"
        private const val TAG = "ConnectionService"
        private const val NOTIFICATION_ID = 7
        private const val REQUEST_DISCONNECT = 1002

        /**
         * Starts the service as a foreground service. Called from the activity's onStop of the last
         * started activity, which Android allows for a foreground-service start. A refused start is
         * logged, not thrown: the connections stay up for as long as the process lives.
         */
        fun start(context: Context) {
            try {
                ContextCompat.startForegroundService(context, Intent(context, ConnectionService::class.java))
            } catch (refused: IllegalStateException) {
                // ForegroundServiceStartNotAllowedException (Android 12+) is a subclass of this; catching the
                // superclass keeps the code valid below API 31, where the subclass does not exist.
                Log.w(TAG, "The connection service could not start in the background (${refused.javaClass.simpleName})")
            }
        }

        /** Stops the service when the app comes to the foreground: the app's own connections need no service then. */
        fun stop(context: Context) {
            context.stopService(Intent(context, ConnectionService::class.java))
        }
    }
}
