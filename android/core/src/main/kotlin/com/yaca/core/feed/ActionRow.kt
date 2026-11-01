package com.yaca.core.feed

import com.yaca.core.model.CallId
import com.yaca.core.model.FileDiff
import com.yaca.core.model.FileFact
import com.yaca.core.model.TokenCounts
import com.yaca.core.model.ToolMeta
import kotlinx.serialization.json.JsonElement

/** How an action ended. [RUNNING] is the dot and the present tense. */
enum class ActionState { RUNNING, DONE, FAILED, REJECTED, DENIED, STOPPED }

/**
 * Tools whose one call changes many files (`Rename`). Each file arrives as a
 * `fileChanged` of its own under the call, and becomes a row nested under the
 * call's row instead of taking its place: the row is the rename, the rows
 * under it are the files.
 */
val MULTI_FILE_TOOLS: Set<String> = setOf("Rename")

/** One row in an action group. */
data class ActionRow(
    val callId: CallId,
    /** Tool name. Not shown in the feed; needed to pick the layout. */
    val tool: String,
    /** The model's caption, or `null` for tools the feed captions itself. */
    val description: String?,
    /** First line of the expanded row: the full path or the full command. */
    val target: String?,
    /** The call's arguments as sent; [kotlinx.serialization.json.JsonNull] for a row the feed made itself. */
    val input: JsonElement,
    val state: ActionState,
    /** The reason for a failure, a broken invariant or a refusal by the human. */
    val message: String?,
    val meta: ToolMeta?,
    val output: String,
    /** Set on file actions: the caption and the counters come from here. */
    val fact: FileFact?,
    val diff: FileDiff?,
    /**
     * Subagent calls, on the `Task` row that sent them. Rows as rows: a subagent's
     * edit is an edit to the human's files, and it gets the same diff. Nesting is
     * one level deep: a subagent does not start a subagent.
     */
    val children: List<ActionRow>,
    /**
     * What a subagent has spent so far, on its `Task` row: the newest
     * `subagentUsage`, which is a running total. `null` on every other row,
     * and on a `Task` row no figure has arrived for.
     */
    val tokens: TokenCounts?,
)

/**
 * Rows together with their nested ones, in order of appearance.
 *
 * For everything that counts actions rather than draws them: what was touched, how
 * many were created, which diff is the last. A file changed by a subagent is changed,
 * and both in the group header and in the file panel it stands on a par with the rest.
 */
fun flatRows(rows: List<ActionRow>): List<ActionRow> =
    rows.flatMap { row -> listOf(row) + flatRows(row.children) }
