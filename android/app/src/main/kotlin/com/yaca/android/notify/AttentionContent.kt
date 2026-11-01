package com.yaca.android.notify

import android.app.Notification
import android.content.Context
import androidx.core.app.NotificationCompat
import com.yaca.android.R
import com.yaca.android.attention.ChatKey
import com.yaca.core.client.AttentionEvent
import com.yaca.core.protocol.AttentionPermission
import com.yaca.core.protocol.AttentionQuestion

/**
 * What each notification says. The host's own words (a target, a question, a chat title, a
 * project) are shown as the host sent them; only the frame around them is localized.
 */
internal object AttentionContent {
    fun permission(
        context: Context,
        key: ChatKey,
        event: AttentionEvent,
        reason: AttentionPermission,
    ): Notification =
        base(context, Channels.NEEDS_YOU, key, title = context.getString(R.string.notify_permission_title), text = reason.target, subText = chatLine(context, event))
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_MESSAGE)
            .addAction(
                0,
                context.getString(R.string.notify_action_allow_once),
                ChatIntents.answerPermission(context, key, reason.request, ChatIntents.PermissionAction.ALLOW_ONCE),
            )
            .addAction(
                0,
                context.getString(R.string.notify_action_deny),
                ChatIntents.answerPermission(context, key, reason.request, ChatIntents.PermissionAction.DENY),
            )
            .build()

    /** A question has options the notification cannot offer, so it only opens the chat. */
    fun question(
        context: Context,
        key: ChatKey,
        event: AttentionEvent,
        reason: AttentionQuestion,
    ): Notification =
        base(context, Channels.NEEDS_YOU, key, title = context.getString(R.string.notify_question_title), text = reason.text, subText = chatLine(context, event))
            .setStyle(NotificationCompat.BigTextStyle().bigText(reason.text))
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_MESSAGE)
            .build()

    fun elicitation(context: Context, key: ChatKey, event: AttentionEvent): Notification =
        base(context, Channels.NEEDS_YOU, key, title = context.getString(R.string.notify_elicitation_title), text = chatLine(context, event), subText = null)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_MESSAGE)
            .build()

    /** Quiet by design: the chat is done, nothing is asked of the human right now. */
    fun turnFinished(context: Context, key: ChatKey, event: AttentionEvent): Notification =
        base(
            context,
            Channels.TURN_FINISHED,
            key,
            title = context.getString(R.string.notify_turn_finished_title, titleOf(context, event)),
            text = event.project,
            subText = null,
        )
            .setSilent(true)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setCategory(NotificationCompat.CATEGORY_STATUS)
            .build()

    /** The request still waits, so the notification stays, and offers the chat instead of the two buttons. */
    fun answerFailed(context: Context, key: ChatKey): Notification =
        base(
            context,
            Channels.NEEDS_YOU,
            key,
            title = context.getString(R.string.notify_permission_title),
            text = context.getString(R.string.notify_answer_failed),
            subText = null,
        )
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_MESSAGE)
            .addAction(0, context.getString(R.string.notify_action_open), ChatIntents.openChat(context, key))
            .build()

    /** Shared by every notification: the small icon, the tap that opens the chat, and dismissal on tap. */
    private fun base(
        context: Context,
        channel: String,
        key: ChatKey,
        title: String,
        text: CharSequence,
        subText: String?,
    ): NotificationCompat.Builder {
        val builder = NotificationCompat.Builder(context, channel)
            .setSmallIcon(R.drawable.ic_stat_yaca)
            .setContentTitle(title)
            .setContentText(text)
            .setContentIntent(ChatIntents.openChat(context, key))
            .setAutoCancel(true)
        if (subText != null) builder.setSubText(subText)
        return builder
    }

    private fun chatLine(context: Context, event: AttentionEvent): String =
        context.getString(R.string.notify_chat_line, titleOf(context, event), event.project)

    private fun titleOf(context: Context, event: AttentionEvent): String =
        event.title?.takeIf { it.isNotBlank() } ?: context.getString(R.string.chat_untitled)
}
