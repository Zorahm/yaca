package com.yaca.android.notify

import android.app.Notification
import android.content.Context
import android.util.Log
import androidx.core.app.NotificationManagerCompat
import com.yaca.android.attention.AttentionTracker
import com.yaca.android.attention.ChatKey
import com.yaca.android.attention.HostAttention
import com.yaca.android.attention.VisibleChat
import com.yaca.android.settings.AppSettings
import com.yaca.core.client.HostClient
import com.yaca.core.model.CommandAnswerPermission
import com.yaca.core.model.PermissionAnswer
import com.yaca.core.model.RequestId
import com.yaca.core.protocol.AttentionElicitation
import com.yaca.core.protocol.AttentionPermission
import com.yaca.core.protocol.AttentionQuestion
import com.yaca.core.protocol.AttentionTurnFinished
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.withTimeout

/**
 * Turns the attention pushes of the hosts into notifications: one per chat, which the chat's next
 * event replaces. Nothing is posted for a chat the human is looking at, and a chat that is opened
 * loses its notification.
 */
class AttentionNotifier(
    private val context: Context,
    private val visibleChat: VisibleChat,
    private val settings: StateFlow<AppSettings>,
    private val attention: AttentionTracker,
    private val clientFor: (fingerprint: String) -> HostClient?,
) {
    fun onPush(push: HostAttention) {
        val event = push.event
        val key = ChatKey(push.fingerprint, event.session)
        if (visibleChat.isOnScreen(key)) return
        when (val reason = event.reason) {
            is AttentionPermission -> post(key, AttentionContent.permission(context, key, event, reason))
            is AttentionQuestion -> post(key, AttentionContent.question(context, key, event, reason))
            is AttentionElicitation -> post(key, AttentionContent.elicitation(context, key, event))
            is AttentionTurnFinished -> if (settings.value.notifyTurnFinished) {
                post(key, AttentionContent.turnFinished(context, key, event))
            } else {
                // The chat no longer waits for the human, so a permission notification from before is stale.
                cancel(key)
            }
        }
    }

    /** Removes the chat's notification: the chat was opened, or it no longer waits. */
    fun cancel(key: ChatKey) {
        NotificationManagerCompat.from(context).cancel(key.tag, NOTIFICATION_ID)
    }

    /**
     * Sends the human's answer to a permission request. An answer that is sent clears the chat's
     * waiting mark and its notification. One that is not sent replaces the notification with one
     * that offers to open the chat: the request still waits for the human.
     */
    suspend fun answerPermission(key: ChatKey, requestId: RequestId, answer: PermissionAnswer): Boolean {
        val client = clientFor(key.fingerprint)
        val sent = client != null && sendAnswer(client, key, requestId, answer)
        if (sent) {
            attention.markSeen(key.fingerprint, key.session)
            cancel(key)
        } else {
            post(key, AttentionContent.answerFailed(context, key))
        }
        return sent
    }

    private suspend fun sendAnswer(
        client: HostClient,
        key: ChatKey,
        requestId: RequestId,
        answer: PermissionAnswer,
    ): Boolean =
        try {
            withTimeout(ANSWER_TIMEOUT_MS) {
                client.send(key.session, CommandAnswerPermission(requestId, answer))
            }
            true
        } catch (timeout: TimeoutCancellationException) {
            Log.w(TAG, "The answer to a permission request timed out")
            false
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (failure: Exception) {
            // The class name only: the message is the host's text, and the credentials never leave the client.
            Log.w(TAG, "The answer to a permission request was not sent (${failure.javaClass.simpleName})")
            false
        }

    private fun post(key: ChatKey, notification: Notification) {
        postNotification(context, key.tag, NOTIFICATION_ID, notification)
    }

    private companion object {
        const val TAG = "AttentionNotifier"

        /** One id for every chat: the tag is what tells the chats apart. */
        const val NOTIFICATION_ID = 1

        /** A notification action has about ten seconds in a broadcast receiver; the answer must end inside that. */
        const val ANSWER_TIMEOUT_MS = 8_000L
    }
}
