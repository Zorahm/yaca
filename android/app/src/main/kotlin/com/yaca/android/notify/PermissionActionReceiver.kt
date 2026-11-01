package com.yaca.android.notify

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.yaca.android.YacaApplication
import com.yaca.core.model.PermissionAnswer
import com.yaca.core.model.PermissionAnswerAllow
import com.yaca.core.model.PermissionAnswerDeny
import com.yaca.core.model.RequestId
import kotlinx.coroutines.launch

/**
 * The "Allow once" and "Deny" buttons of a permission notification. The answer is a call to the
 * computer, so the receiver hands it to the app's scope and keeps the broadcast open until the answer
 * is sent or has failed; [AttentionNotifier] then shows the outcome in the notification.
 */
class PermissionActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val key = ChatIntents.chatKey(intent) ?: return
        val requestId = intent.getStringExtra(ChatIntents.EXTRA_REQUEST_ID)?.let { RequestId(it) } ?: return
        val answer: PermissionAnswer = when (intent.action) {
            ACTION_ALLOW_ONCE -> PermissionAnswerAllow
            // No reason: a notification button cannot take one. Without a note the agent may simply ask again.
            ACTION_DENY -> PermissionAnswerDeny()
            else -> return
        }
        val container = (context.applicationContext as YacaApplication).container
        val pending = goAsync()
        container.scope.launch {
            try {
                container.answerPermission(key, requestId, answer)
            } finally {
                pending.finish()
            }
        }
    }

    companion object {
        const val ACTION_ALLOW_ONCE = "com.yaca.android.action.ALLOW_ONCE"
        const val ACTION_DENY = "com.yaca.android.action.DENY"
    }
}
