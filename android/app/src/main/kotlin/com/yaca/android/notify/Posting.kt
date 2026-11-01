package com.yaca.android.notify

import android.Manifest
import android.annotation.SuppressLint
import android.app.Notification
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

/** Whether Android lets Yaca show a notification now: the permission (Android 13+) and the switch in the system settings. */
fun canPostNotifications(context: Context): Boolean {
    if (notificationPermissionMissing(context)) return false
    return NotificationManagerCompat.from(context).areNotificationsEnabled()
}

/** Android 13 made the notification permission a runtime one; below 13 it is granted with the install. */
fun notificationPermissionMissing(context: Context): Boolean =
    Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
        ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) !=
        PackageManager.PERMISSION_GRANTED

/**
 * Posts or replaces the notification of [tag] and [id]. Without [canPostNotifications] nothing is
 * posted: the notifications are not shown anyway, and the connection itself does not depend on them.
 */
@SuppressLint("MissingPermission") // canPostNotifications checks the permission this call needs.
fun postNotification(context: Context, tag: String?, id: Int, notification: Notification) {
    if (canPostNotifications(context)) {
        NotificationManagerCompat.from(context).notify(tag, id, notification)
    }
}
