package com.yaca.core.feed

/**
 * The work of a turn, folded into feed items (see `WorkFold.kt`). What the feed draws is a
 * [FeedItem]: the work and subagent items of this file, and every [FeedBlock] except reasoning
 * and actions, which exist only inside a [Work] item.
 */
sealed interface FeedItem {
    val key: String

    /** One line of a work item, in the order it happened. */
    sealed interface WorkStep {
        val key: String

        data class Thought(
            override val key: String,
            val text: String,
        ) : WorkStep

        data class Action(
            override val key: String,
            val row: ActionRow,
        ) : WorkStep

        /** A snapshot taken mid-turn (before a command) keeps its place in time. */
        data class Checkpoint(
            override val key: String,
            val block: FeedBlock.Checkpoint,
        ) : WorkStep
    }

    /** A stretch of the agent's calls and thoughts, between what it said. */
    data class Work(
        override val key: String,
        val steps: List<WorkStep>,
    ) : FeedItem

    /** Subagents sent one after another: the `Task` rows, in order. */
    data class Agents(
        override val key: String,
        val rows: List<ActionRow>,
    ) : FeedItem
}
