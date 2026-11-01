package com.yaca.core.feed

import com.yaca.core.model.AgentEventFileChanged
import com.yaca.core.model.AgentEventSubagentUsage
import com.yaca.core.model.AgentEventToolCallDenied
import com.yaca.core.model.AgentEventToolCallFailed
import com.yaca.core.model.AgentEventToolCallFinished
import com.yaca.core.model.AgentEventToolCallProgress
import com.yaca.core.model.AgentEventToolCallRejected
import com.yaca.core.model.AgentEventToolCallStarted
import com.yaca.core.model.CallId
import com.yaca.core.model.FileDiff
import com.yaca.core.model.FileFact
import kotlinx.serialization.json.JsonNull

// Tool calls: their rows, their outcomes, their output, and the files they changed.

internal fun FeedDraft.onToolCallStarted(event: AgentEventToolCallStarted) {
    val row = ActionRow(
        callId = event.callId,
        tool = event.tool,
        description = event.description,
        target = event.target,
        input = event.input,
        state = ActionState.RUNNING,
        message = null,
        meta = null,
        output = "",
        fact = null,
        diff = null,
        children = emptyList(),
        tokens = null,
    )
    // A subagent call goes under its own `Task` row, wherever that row is: a permission
    // request may have come between them, and it closes the group. A parent that is not in
    // the feed is no reason to lose the action.
    val parent = event.parent
    val nested = parent != null && patchRow(parent) { task -> task.copy(children = task.children + row) }
    if (!nested) startCall(row)
}

internal fun FeedDraft.onSubagentUsage(event: AgentEventSubagentUsage) {
    patchRow(event.callId) { row -> row.copy(tokens = event.tokens) }
}

internal fun FeedDraft.onToolCallProgress(event: AgentEventToolCallProgress) {
    patchRow(event.callId) { row -> row.copy(output = row.output + event.chunk) }
}

internal fun FeedDraft.onToolCallFinished(event: AgentEventToolCallFinished) {
    patchRow(event.callId) { row -> row.copy(state = ActionState.DONE, meta = event.meta) }
}

internal fun FeedDraft.onToolCallFailed(event: AgentEventToolCallFailed) {
    patchRow(event.callId) { row -> row.copy(state = ActionState.FAILED, message = event.message) }
}

internal fun FeedDraft.onToolCallRejected(event: AgentEventToolCallRejected) {
    patchRow(event.callId) { row -> row.copy(state = ActionState.REJECTED, message = event.reason) }
}

/**
 * A refused permission and a stopped running call are one kind of outcome (the human's
 * decision), but not one phrase.
 */
internal fun FeedDraft.onToolCallDenied(event: AgentEventToolCallDenied) {
    patchRow(event.callId) { row ->
        row.copy(
            state = if (event.stopped) ActionState.STOPPED else ActionState.DENIED,
            message = event.reason,
        )
    }
}

/**
 * The file panel's index is kept here too: the panel shows the latest thing the core sent
 * for this path.
 */
internal fun FeedDraft.onFileChanged(event: AgentEventFileChanged) {
    diffs = diffs + (event.fact.path to event.diff)
    writes += 1
    patchRow(event.callId) { row ->
        if (row.tool in MULTI_FILE_TOOLS) {
            row.copy(children = withFile(row, event.fact, event.diff))
        } else {
            row.copy(fact = event.fact, diff = event.diff)
        }
    }
}

/** Appends a row to the last action group if the last block is one; otherwise opens a new group. */
private fun FeedDraft.startCall(row: ActionRow) {
    val last = blocks.lastOrNull()
    if (last is FeedBlock.Actions) {
        blocks[blocks.lastIndex] = last.copy(rows = last.rows + row)
        return
    }
    blocks.add(FeedBlock.Actions(key("acts"), listOf(row)))
}

/**
 * Changes a row by its id, its own or nested; says whether it was found. Searches from the
 * newest group backwards: an outcome belongs to the call most recently started.
 */
internal fun FeedDraft.patchRow(callId: CallId, change: (ActionRow) -> ActionRow): Boolean {
    for (i in blocks.indices.reversed()) {
        val block = blocks[i] as? FeedBlock.Actions ?: continue
        val rows = patchIn(block.rows, callId, change) ?: continue
        blocks[i] = block.copy(rows = rows)
        return true
    }
    return false
}

/** The list with the row changed, its own or nested; `null` if it is not here. */
private fun patchIn(
    rows: List<ActionRow>,
    callId: CallId,
    change: (ActionRow) -> ActionRow,
): List<ActionRow>? {
    for (at in rows.indices.reversed()) {
        val row = rows[at]
        val next = if (row.callId == callId) {
            change(row)
        } else {
            val children = patchIn(row.children, callId, change) ?: continue
            row.copy(children = children)
        }
        return rows.toMutableList().apply { this[at] = next }
    }
    return null
}

/**
 * Stopping a turn marks its unfinished calls as failed, in every group that has any. A live
 * dot on a call nobody is running any more is a lie.
 */
internal fun FeedDraft.cancelRunning() {
    for (i in blocks.indices.reversed()) {
        val block = blocks[i] as? FeedBlock.Actions ?: continue
        if (flatRows(block.rows).none { it.state == ActionState.RUNNING }) continue
        blocks[i] = block.copy(rows = block.rows.map(::cancelled))
    }
}

private fun cancelled(row: ActionRow): ActionRow {
    val running = row.state == ActionState.RUNNING
    return row.copy(
        state = if (running) ActionState.FAILED else row.state,
        message = if (running) null else row.message,
        children = row.children.map(::cancelled),
    )
}

/**
 * The rows under a many-file call, with this file's in place: an edit of the file it already
 * has replaces that row rather than adding a second.
 */
private fun withFile(row: ActionRow, fact: FileFact, diff: FileDiff): List<ActionRow> {
    val file = ActionRow(
        callId = CallId("${row.callId.value}#${fact.path}"),
        tool = "Edit",
        description = null,
        target = fact.path,
        input = JsonNull,
        state = ActionState.DONE,
        message = null,
        meta = null,
        output = "",
        fact = fact,
        diff = diff,
        children = emptyList(),
        tokens = null,
    )
    val at = row.children.indexOfFirst { it.fact?.path == fact.path }
    if (at == -1) return row.children + file
    return row.children.toMutableList().apply { this[at] = file }
}
