package com.yaca.core.feed

import com.yaca.core.model.AgentEventFileViewLoaded
import com.yaca.core.model.AgentEventFilesFound
import com.yaca.core.model.AgentEventLspUpdated
import com.yaca.core.model.AgentEventMcpUpdated
import com.yaca.core.model.AgentEventProcessOutput
import com.yaca.core.model.AgentEventProcessOutputReset
import com.yaca.core.model.AgentEventProcessesUpdated
import com.yaca.core.model.AgentEventQueueUpdated
import com.yaca.core.model.AgentEventStateChanged
import com.yaca.core.model.AgentEventTodosUpdated
import com.yaca.core.model.AgentEventUsageUpdated

// Side state the interface shows beside the feed: the session's bottom row, the context ring,
// the processes and their output, the MCP and LSP lists, the queue, the file panel, search
// answers, and the todo list.

internal fun FeedDraft.onStateChanged(event: AgentEventStateChanged) {
    state = event.state
}

internal fun FeedDraft.onUsageUpdated(event: AgentEventUsageUpdated) {
    usage = event.usage
}

internal fun FeedDraft.onProcessesUpdated(event: AgentEventProcessesUpdated) {
    processes = event.processes
}

/** The human picked a row: the whole buffer, dropping whatever was there before. */
internal fun FeedDraft.onProcessOutputReset(event: AgentEventProcessOutputReset) {
    processOutput = ProcessOutputBuffer(event.id, event.text, event.dropped)
}

/**
 * The live tail arrives only for the selected row, and a selection is always announced by a
 * reset. A chunk with someone else's id is not appended: it would replace the buffer of the one
 * being looked at.
 */
internal fun FeedDraft.onProcessOutput(event: AgentEventProcessOutput) {
    val current = processOutput ?: return
    if (current.id != event.id) return
    processOutput = current.copy(text = current.text + event.chunk)
}

internal fun FeedDraft.onMcpUpdated(event: AgentEventMcpUpdated) {
    mcp = event.servers
}

internal fun FeedDraft.onLspUpdated(event: AgentEventLspUpdated) {
    lsp = event.servers
}

internal fun FeedDraft.onQueueUpdated(event: AgentEventQueueUpdated) {
    queued = event.queued
}

internal fun FeedDraft.onFilesFound(event: AgentEventFilesFound) {
    found = FileSearchResult(event.query, event.paths)
}

internal fun FeedDraft.onFileViewLoaded(event: AgentEventFileViewLoaded) {
    fileView = FileViewSlot(event.path, event.view)
}

/**
 * Updated in place; no new block is created: the todo list is one block for the whole session,
 * and it stays where it first appeared.
 */
internal fun FeedDraft.onTodosUpdated(event: AgentEventTodosUpdated) {
    val at = blocks.indexOfFirst { it is FeedBlock.Todos }
    val block = FeedBlock.Todos(blocks.getOrNull(at)?.key ?: key("todos"), event.items)
    if (at == -1) blocks.add(block) else blocks[at] = block
}
