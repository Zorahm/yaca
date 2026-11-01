package com.yaca.core.feed

import com.yaca.core.model.CheckpointChange
import com.yaca.core.model.CheckpointId
import com.yaca.core.model.CheckpointSummary
import com.yaca.core.model.FileDiff
import com.yaca.core.model.FileView
import com.yaca.core.model.LspServerSummary
import com.yaca.core.model.McpServerSummary
import com.yaca.core.model.PermissionRequest
import com.yaca.core.model.ProcessId
import com.yaca.core.model.ProcessSummary
import com.yaca.core.model.QueuedMessage
import com.yaca.core.model.QuestionRequest
import com.yaca.core.model.SessionState
import com.yaca.core.model.TsMillis
import com.yaca.core.model.UsageSnapshot

/**
 * The feed as a pure function of the event stream.
 *
 * The core promises that the interface is a function of `AgentEvent`, and here it is. Everything
 * drawn in the feed and the panels is derived from here: keeping state in components would mean
 * having a second version of the truth, and it diverges from the first at the first mismatch
 * with a file.
 *
 * Two properties keep the rest in place.
 *
 * *The list of blocks only grows.* A permission block is not removed after its answer; it is
 * marked closed and not drawn: this way block indices do not shift, and the feed
 * behaves like a journal from which it is itself rebuilt.
 *
 * *A batch is one recomputation.* The bridge sends events in batches per frame, so
 * [reduceAll] creates one new snapshot per batch, not per delta.
 *
 * Port of `src/core/feed.ts`. The fields keep the TypeScript names and their order.
 */
data class Feed(
    val blocks: List<FeedBlock> = emptyList(),
    /** The composer's bottom row: modes, model, busy state. */
    val state: SessionState? = null,
    /** The context ring. */
    val usage: UsageSnapshot? = null,
    /** The processes panel. */
    val processes: List<ProcessSummary> = emptyList(),
    /**
     * This chat's MCP servers, whole, as the core last sent them in `mcpUpdated`.
     * `null` means the list has not arrived yet.
     */
    val mcp: List<McpServerSummary>? = null,
    /**
     * Language servers of this chat, whole, as the core last sent them in `lspUpdated`.
     * `null` until the list has arrived.
     */
    val lsp: List<LspServerSummary>? = null,
    /**
     * Output of the selected process. It arrives only for the row being looked at:
     * `processOutputReset` puts the whole buffer, live chunks are appended.
     * One slot, not a map: a human can only look at one.
     */
    val processOutput: ProcessOutputBuffer? = null,
    /**
     * The latest diff for each path: the file panel's index.
     *
     * Stagings arrive without an action row too: when applying conflicts, the
     * `callId` is empty, yet the diff has to be shown. The last one per path wins:
     * the file panel shows the file as it now stands in the set, not its first edit.
     */
    val diffs: Map<String, FileDiff> = emptyMap(),
    /**
     * How many file writes arrived in the session. The file panel rereads its view
     * when the number grows: a new write changes what it shows.
     */
    val writes: Int = 0,
    /**
     * The full diff of one file for the panel, the last one to arrive. One slot:
     * there is one file panel. `view: null` means nothing in the file awaits a decision.
     */
    val fileView: FileViewSlot? = null,
    /** The checkpoints panel. */
    val checkpoints: List<CheckpointSummary> = emptyList(),
    /** The active permission request, if any. */
    val request: PermissionRequest? = null,
    /** The open `AskUser` question, if any: the bottom row and the hotkeys wait for it. */
    val question: QuestionRequest? = null,
    /**
     * What a rollback would touch, if confirmed.
     *
     * It lives in the feed, not in a button: the list arrives as an event, and the
     * checkpoint row that asked for it is the one that must show it.
     */
    val preview: CheckpointPreview? = null,
    /**
     * Messages waiting for their turn in the run, in order. The list arrives whole
     * from the core: cancelling and merging change it there, and here only the
     * latest arrival is shown.
     */
    val queued: List<QueuedMessage> = emptyList(),
    /**
     * The latest answer to an `@` search. It carries the query it answers:
     * the composer shows it only while the query is still the one typed.
     */
    val found: FileSearchResult? = null,
    /**
     * The turn that is running, from the core's `turnStarted`. The live timer
     * counts from it; `null` between turns.
     */
    val turn: RunningTurn? = null,
) {
    companion object {
        /** The feed before the first event. */
        val EMPTY: Feed = Feed()
    }
}

/** The selected process's buffer. [dropped] lines were pushed out of it; the panel says so, since silence cannot be told from no output. */
data class ProcessOutputBuffer(
    val id: ProcessId,
    val text: String,
    val dropped: Long,
)

/** The file panel's slot: the path asked for and the view the core answered with. */
data class FileViewSlot(
    val path: String,
    /** `null` when nothing in the file awaits a decision. */
    val view: FileView?,
)

/** What a rollback to [id] would change, before it is confirmed. */
data class CheckpointPreview(
    val id: CheckpointId,
    val files: List<CheckpointChange>,
)

/** The answer to an `@` search, with the query it answers. */
data class FileSearchResult(
    val query: String,
    val paths: List<String>,
)

/** A turn that has started and not ended: the live timer counts from [startedAt]. */
data class RunningTurn(
    val startedAt: TsMillis,
)
