package com.yaca.android

import android.app.Application
import com.yaca.android.notify.Channels
import com.yaca.android.service.ConnectionLifecycle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/**
 * The process. Holds the one [AppContainer] and the scope its connections live in: that
 * scope outlives every screen, so a connection does not die when the human leaves a chat.
 */
class YacaApplication : Application() {
    private val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    val container: AppContainer by lazy { AppContainer(this, appScope) }

    override fun onCreate() {
        super.onCreate()
        // Channels exist before any notification or the connection service can post one.
        Channels.create(this)
        registerActivityLifecycleCallbacks(ConnectionLifecycle(this, container))
    }
}
