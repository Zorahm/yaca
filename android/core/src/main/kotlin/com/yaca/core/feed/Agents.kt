package com.yaca.core.feed

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

// Subagents, as the feed's rows tell of them. A subagent is not a second state: it is the `Task`
// row that sent it. The row's input holds the prompt, its children are what the subagent did, its
// output is the report it handed back, and `tokens` is the newest `subagentUsage` the core sent for
// it. Everything the plate in the feed and the agents panel show is read from those rows here.

/** The tool that sends a subagent. */
const val AGENT_TOOL: String = "Task"

fun isAgent(row: ActionRow): Boolean = row.tool == AGENT_TOOL

/** Every subagent of the conversation, in the order they were sent. */
fun agentsOf(feed: Feed): List<ActionRow> =
    feed.blocks.filterIsInstance<FeedBlock.Actions>().flatMap { block -> block.rows.filter(::isAgent) }

/** The task the agent wrote for it, or nothing if the call carried none. */
fun agentPrompt(row: ActionRow): String {
    val prompt = (row.input as? JsonObject)?.get("prompt") as? JsonPrimitive ?: return ""
    return if (prompt.isString) prompt.content.trim() else ""
}

/**
 * What a subagent has spent: every token that went through the model for it, cached or not.
 * `null` until the first figure arrives, which is not the same as zero: a journal written before
 * the figure existed never has one.
 */
fun agentTokens(row: ActionRow): Long? {
    val spent = row.tokens ?: return null
    return spent.input.toLong() + spent.output + spent.cacheWrite + spent.cacheRead
}

data class AgentsTally(
    val count: Int,
    val running: Int,
    /** Ended without a report: failed, refused or stopped. */
    val failed: Int,
    /** `null` when none of the agents has reported a figure. */
    val tokens: Long?,
)

fun tally(rows: List<ActionRow>): AgentsTally {
    var tokens: Long? = null
    for (row in rows) {
        val spent = agentTokens(row) ?: continue
        tokens = (tokens ?: 0L) + spent
    }
    return AgentsTally(
        count = rows.size,
        running = rows.count { it.state == ActionState.RUNNING },
        failed = rows.count { it.state != ActionState.RUNNING && it.state != ActionState.DONE },
        tokens = tokens,
    )
}
