package com.yaca.android.notify

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import com.yaca.android.MainActivity
import com.yaca.android.attention.ChatKey
import com.yaca.core.model.RequestId
import com.yaca.core.model.SessionId

/**
 * The intents the notifications carry. Each PendingIntent gets a request code made from its chat
 * and its action, so the PendingIntents of two chats, or of two buttons, never replace each other.
 */
object ChatIntents {
    /** A launch intent that asks the app to show a chat; [EXTRA_FINGERPRINT] and [EXTRA_SESSION] name it. */
    const val ACTION_OPEN_CHAT = "com.yaca.android.action.OPEN_CHAT"
    const val EXTRA_FINGERPRINT = "com.yaca.android.extra.FINGERPRINT"
    const val EXTRA_SESSION = "com.yaca.android.extra.SESSION"
    const val EXTRA_REQUEST_ID = "com.yaca.android.extra.REQUEST_ID"

    private const val REQUEST_OPEN_APP = 1001

    /** The buttons of a permission notification. [code] is the action part of the request code. */
    enum class PermissionAction(val intentAction: String, internal val code: Int) {
        ALLOW_ONCE(PermissionActionReceiver.ACTION_ALLOW_ONCE, code = 1),
        DENY(PermissionActionReceiver.ACTION_DENY, code = 2),
    }

    /** Opens the app on its start screen: the connection service's notification leads here. */
    fun openApp(context: Context): PendingIntent =
        PendingIntent.getActivity(
            context,
            REQUEST_OPEN_APP,
            Intent(context, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE,
        )

    /** Opens [key]'s chat in the app: the content intent of its notifications and the "Open" button. */
    fun openChat(context: Context, key: ChatKey): PendingIntent {
        val intent = Intent(context, MainActivity::class.java)
            .setAction(ACTION_OPEN_CHAT)
            .putExtra(EXTRA_FINGERPRINT, key.fingerprint)
            .putExtra(EXTRA_SESSION, key.session.value)
        return PendingIntent.getActivity(
            context,
            requestCode(key, action = 0),
            intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }

    /** Answers the permission request [requestId] of [key] from its notification, through [PermissionActionReceiver]. */
    fun answerPermission(
        context: Context,
        key: ChatKey,
        requestId: RequestId,
        button: PermissionAction,
    ): PendingIntent {
        val intent = Intent(context, PermissionActionReceiver::class.java)
            .setAction(button.intentAction)
            .putExtra(EXTRA_FINGERPRINT, key.fingerprint)
            .putExtra(EXTRA_SESSION, key.session.value)
            .putExtra(EXTRA_REQUEST_ID, requestId.value)
        return PendingIntent.getBroadcast(
            context,
            requestCode(key, button.code),
            intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }

    /** The chat an intent names, or null when it names none. */
    fun chatKey(intent: Intent): ChatKey? {
        val fingerprint = intent.getStringExtra(EXTRA_FINGERPRINT) ?: return null
        val session = intent.getStringExtra(EXTRA_SESSION) ?: return null
        return ChatKey(fingerprint, SessionId(session))
    }

    private fun requestCode(key: ChatKey, action: Int): Int = key.hashCode() * 4 + action
}
