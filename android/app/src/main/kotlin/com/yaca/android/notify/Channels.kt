package com.yaca.android.notify

import android.content.Context
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationManagerCompat
import com.yaca.android.R

/**
 * The three channels of Yaca. Creating a channel that exists only updates its name and
 * description, so this runs at every start of the process.
 */
object Channels {
    /** A permission, a question or a form waits for the human: heads-up, with sound. */
    const val NEEDS_YOU = "needs_you"

    /** A finished turn is news, not a request: visible, but quiet. */
    const val TURN_FINISHED = "turn_finished"

    /** The ongoing notification of the connection service: low, so it never makes a sound. */
    const val CONNECTION = "connection"

    fun create(context: Context) {
        val manager = NotificationManagerCompat.from(context)
        manager.createNotificationChannel(
            NotificationChannelCompat.Builder(NEEDS_YOU, NotificationManagerCompat.IMPORTANCE_HIGH)
                .setName(context.getString(R.string.notify_channel_needs_you))
                .setDescription(context.getString(R.string.notify_channel_needs_you_desc))
                .build(),
        )
        manager.createNotificationChannel(
            NotificationChannelCompat.Builder(TURN_FINISHED, NotificationManagerCompat.IMPORTANCE_DEFAULT)
                .setName(context.getString(R.string.notify_channel_turn_finished))
                .setDescription(context.getString(R.string.notify_channel_turn_finished_desc))
                .build(),
        )
        manager.createNotificationChannel(
            NotificationChannelCompat.Builder(CONNECTION, NotificationManagerCompat.IMPORTANCE_LOW)
                .setName(context.getString(R.string.notify_channel_connection))
                .setDescription(context.getString(R.string.notify_channel_connection_desc))
                .build(),
        )
    }
}
