package com.yaca.core.feed

import com.yaca.core.model.FileDiff
import java.util.concurrent.atomic.AtomicLong

/**
 * A block's key sequence. Process-wide, not per feed: a key is an identity the interface
 * keeps while a block grows, and two feeds must never mint the same one.
 */
private val keySequence = AtomicLong()

/**
 * The mutable builder one batch of events is folded into. [reduceAll] makes one of these per
 * batch, and [freeze] turns it into the immutable snapshot the interface reads. Nothing outside
 * this package sees it, so the reducer files may mutate it freely.
 *
 * The block list is copied once per batch rather than once per event: the snapshot the batch
 * started from is never written to.
 */
internal class FeedDraft(base: Feed) {
    val blocks: MutableList<FeedBlock> = base.blocks.toMutableList()
    var state = base.state
    var usage = base.usage
    var processes = base.processes
    var mcp = base.mcp
    var lsp = base.lsp
    var processOutput = base.processOutput
    /** Replaced, not mutated, on every write: the same map value is shared with earlier snapshots. */
    var diffs: Map<String, FileDiff> = base.diffs
    var writes = base.writes
    var fileView = base.fileView
    var checkpoints = base.checkpoints
    var request = base.request
    var question = base.question
    var preview = base.preview
    var queued = base.queued
    var found = base.found
    var turn = base.turn

    /** A new key for a block made now. */
    fun key(prefix: String): String = "${prefix}_${keySequence.incrementAndGet()}"

    fun freeze(): Feed = Feed(
        blocks = blocks.toList(),
        state = state,
        usage = usage,
        processes = processes,
        mcp = mcp,
        lsp = lsp,
        processOutput = processOutput,
        diffs = diffs,
        writes = writes,
        fileView = fileView,
        checkpoints = checkpoints,
        request = request,
        question = question,
        preview = preview,
        queued = queued,
        found = found,
        turn = turn,
    )
}
