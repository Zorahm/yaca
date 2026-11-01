package com.yaca.android.service

import android.app.Activity
import android.app.Application
import android.content.Context
import android.os.Bundle
import com.yaca.android.AppContainer

/**
 * Turns the app's first started activity into "foreground" and its last stopped one into
 * "background". There is no ProcessLifecycleOwner in the dependencies, so the activities are counted
 * here, through the application's callbacks.
 *
 * Background, with a computer paired and "Stay connected in the background" on: the connection service
 * starts, and the connections go on. Background without that setting: the connections stop. Foreground:
 * the service stops, and every connection is (re)started, which is a no-op for a live one.
 *
 * The service starts in onActivityStopped, the last point where Android still counts the start as coming
 * from the foreground. A rotation stops the activity without leaving the app; it is not a background.
 */
class ConnectionLifecycle(
    private val context: Context,
    private val container: AppContainer,
) : Application.ActivityLifecycleCallbacks {
    private var started = 0

    override fun onActivityStarted(activity: Activity) {
        started++
        if (started == 1) enterForeground()
    }

    override fun onActivityStopped(activity: Activity) {
        started--
        if (started == 0 && !activity.isChangingConfigurations) enterBackground()
    }

    private fun enterForeground() {
        container.visibleChat.setForeground(true)
        ConnectionService.stop(context)
        container.connectAll()
    }

    private fun enterBackground() {
        container.visibleChat.setForeground(false)
        val stayConnected = container.settings.value.stayConnected
        val hasComputers = container.hosts.value.orEmpty().isNotEmpty()
        if (stayConnected && hasComputers) {
            ConnectionService.start(context)
        } else {
            container.disconnectAll()
        }
    }

    override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) = Unit

    override fun onActivityResumed(activity: Activity) = Unit

    override fun onActivityPaused(activity: Activity) = Unit

    override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) = Unit

    override fun onActivityDestroyed(activity: Activity) = Unit
}
