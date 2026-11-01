package com.yaca.android.chat

import com.yaca.core.client.ConnectionState
import com.yaca.core.feed.ActionRow
import com.yaca.core.feed.ElicitationStatus
import com.yaca.core.feed.Feed
import com.yaca.core.feed.FeedBlock
import com.yaca.core.feed.FeedItem
import com.yaca.core.feed.flatRows
import com.yaca.core.model.CallId
import com.yaca.core.model.ElicitationRequest
import com.yaca.core.model.Effort
import com.yaca.core.model.Mode
import com.yaca.core.model.ModelInfo
import com.yaca.core.model.PermissionRequest
import com.yaca.core.model.QueuedId
import com.yaca.core.model.QuestionRequest

/**
 * What the chat screen is drawn from: the host's state for this chat, folded for display, and the
 * few facts the screen needs about the host itself. Nothing here is a second copy of the state.
 */
data class ChatUi(
    val feed: Feed = Feed.EMPTY,
    /** The feed as the list draws it: work folded into groups, closed requests left out. */
    val items: List<FeedItem> = emptyList(),
    val title: String? = null,
    /** The project's last folder name, as the chats list shows it. */
    val project: String = "",
    val connection: ConnectionState = ConnectionState.Idle,
    val models: List<ModelInfo> = emptyList(),
    /** The computer was forgotten on the phone: the screen has to leave. */
    val gone: Boolean = false,
)

/** A full-window layer over the feed. One at a time; the system Back closes it. */
sealed interface ChatPanel {
    data class Diff(val path: String) : ChatPanel
    data class File(val path: String) : ChatPanel
    data object Checkpoints : ChatPanel
    data object Processes : ChatPanel
}

/**
 * A request that waits for the human, drawn as a bottom sheet. Held by the view model so a
 * rotation does not lose it; the sheet is re-openable from the request's block in the feed.
 */
sealed interface RequestSheet {
    val id: String

    data class Permission(val request: PermissionRequest) : RequestSheet {
        override val id: String get() = request.id.value
    }

    data class Question(val request: QuestionRequest) : RequestSheet {
        override val id: String get() = request.id.value
    }

    data class Elicitation(val request: ElicitationRequest) : RequestSheet {
        override val id: String get() = request.id.value
    }
}

/**
 * An action that cannot be taken back, asked for before it is sent. Stopping a process is not here:
 * the processes panel asks for it itself.
 */
sealed interface ChatConfirm {
    data object CloseChat : ChatConfirm
    data object Compact : ChatConfirm
}

/** What the composer holds. It lives in the view model, so a rotation keeps the text a person was writing. */
data class ComposerDraft(
    val text: String = "",
    /** Files picked with `@`, sent with the message. */
    val attachments: List<String> = emptyList(),
    val mentionOpen: Boolean = false,
)

/** What the feed's blocks ask the screen to open. */
class FeedActions(
    val onOpenDiff: (String) -> Unit,
    val onOpenFile: (String) -> Unit,
    val onOpenRequest: (RequestSheet) -> Unit,
    val onOpenCheckpoints: () -> Unit,
)

/** The composer's controls, as the screen answers them. */
class ComposerActions(
    val onTextChange: (String) -> Unit,
    val onSend: () -> Unit,
    val onStop: () -> Unit,
    val onMention: () -> Unit,
    val onRemoveAttachment: (String) -> Unit,
    val onMode: (Mode) -> Unit,
    val onModel: (String) -> Unit,
    val onEffort: (Effort) -> Unit,
    val onCancelQueued: (QueuedId) -> Unit,
)

/** The request a feed reads as open right now, if any: a permission, a question, then a form. */
fun openRequestOf(feed: Feed): RequestSheet? {
    feed.request?.let { return RequestSheet.Permission(it) }
    feed.question?.let { return RequestSheet.Question(it) }
    val form = feed.blocks.firstOrNull { it is FeedBlock.Elicitation && it.status == ElicitationStatus.OPEN }
    return (form as? FeedBlock.Elicitation)?.let { RequestSheet.Elicitation(it.request) }
}

/** The action row a call id names, searched through the groups and their nested rows. */
fun findRow(feed: Feed, callId: CallId): ActionRow? =
    feed.blocks.asSequence()
        .filterIsInstance<FeedBlock.Actions>()
        .firstNotNullOfOrNull { block -> flatRows(block.rows).firstOrNull { it.callId == callId } }
