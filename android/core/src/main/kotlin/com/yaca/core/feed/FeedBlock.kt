package com.yaca.core.feed

import com.yaca.core.model.BlockId
import com.yaca.core.model.CheckpointSummary
import com.yaca.core.model.ElicitationRequest
import com.yaca.core.model.PermissionRequest
import com.yaca.core.model.QuestionReply
import com.yaca.core.model.QuestionRequest
import com.yaca.core.model.TodoItem
import com.yaca.core.model.TsMillis

/** Where an `AskUser` question stands. `open` waits for the human; the others are what happened. */
enum class QuestionStatus { OPEN, ANSWERED, DISMISSED, UNANSWERED }

/** Where an MCP server's form stands. `EXPIRED`: the server stopped waiting before anyone answered. */
enum class ElicitationStatus { OPEN, ACCEPTED, DECLINED, CANCELLED, EXPIRED }

/**
 * One entry of the feed in the order it happened. Every block has a [key] of its own, not an
 * index: blocks outlive a redraw.
 *
 * Reasoning and actions are only ever drawn inside a work item (see `FeedItem.Work`), so they
 * implement [FeedBlock] alone. Every other variant is also a [FeedItem] and passes through
 * `fold` unchanged.
 */
sealed interface FeedBlock {
    val key: String

    data class Human(
        override val key: String,
        val text: String,
        val attachments: List<String>,
    ) : FeedBlock, FeedItem

    data class Agent(
        override val key: String,
        val blockId: BlockId,
        val text: String,
    ) : FeedBlock, FeedItem

    data class Reasoning(
        override val key: String,
        val blockId: BlockId,
        val text: String,
    ) : FeedBlock

    /** Actions in a row are one entry: a reply or reasoning between calls splits them into two blocks. */
    data class Actions(
        override val key: String,
        val rows: List<ActionRow>,
    ) : FeedBlock

    data class Todos(
        override val key: String,
        val items: List<TodoItem>,
    ) : FeedBlock, FeedItem

    data class Permission(
        override val key: String,
        val request: PermissionRequest,
        val open: Boolean,
    ) : FeedBlock, FeedItem

    /**
     * [reply] is what was picked, when there is an answer; `null` while open or unanswered.
     * The block stays in the feed in every status: an answered question is a line of the
     * conversation, not a request to be dismissed.
     */
    data class Question(
        override val key: String,
        val request: QuestionRequest,
        val status: QuestionStatus,
        val reply: QuestionReply?,
    ) : FeedBlock, FeedItem

    data class Elicitation(
        override val key: String,
        val request: ElicitationRequest,
        val status: ElicitationStatus,
    ) : FeedBlock, FeedItem

    data class Checkpoint(
        override val key: String,
        val checkpoint: CheckpointSummary,
    ) : FeedBlock, FeedItem

    data class Restored(
        override val key: String,
        val at: TsMillis,
        val files: Int,
    ) : FeedBlock, FeedItem

    data class Compacted(
        override val key: String,
        val summary: String,
        val freed: Int,
    ) : FeedBlock, FeedItem

    data class Plan(
        override val key: String,
        val plan: String,
    ) : FeedBlock, FeedItem

    /**
     * A file's text the agent put in front of the human (`SendText`). The text
     * is the one the event carried: the file may have changed since.
     */
    data class Shown(
        override val key: String,
        val path: String,
        /** The command line, when this is a background command's output. */
        val command: String?,
        /** 1-indexed. */
        val firstLine: Int,
        val lines: Int,
        /** Lines in the file: `lines == total` is the whole of it. */
        val total: Int,
        val text: String,
    ) : FeedBlock, FeedItem

    data class Error(
        override val key: String,
        val code: String,
        val message: String,
        val retryable: Boolean,
    ) : FeedBlock, FeedItem

    data class Retry(
        override val key: String,
        val attempt: Int,
        val of: Int,
        val delayMs: Int,
    ) : FeedBlock, FeedItem

    /** Which model answers a turn. [route] is `null` for the default route, which has no name. */
    data class Route(
        override val key: String,
        val route: String?,
        val provider: String,
        val model: String,
    ) : FeedBlock, FeedItem

    data class Stopped(
        override val key: String,
    ) : FeedBlock, FeedItem

    /** What a plugin hook said where no call row stands (`hookReported`). */
    data class Hook(
        override val key: String,
        val plugin: String,
        val event: String,
        val text: String,
        val blocked: Boolean,
    ) : FeedBlock, FeedItem

    /**
     * How long a turn took and when it ended, under what it said. Made of
     * `turnStarted.at` and the end event's `at`: the core's clock, not the
     * window's, so a reopened session shows the same figures it did live.
     */
    data class TurnTime(
        override val key: String,
        val startedAt: TsMillis,
        val endedAt: TsMillis,
    ) : FeedBlock, FeedItem
}
