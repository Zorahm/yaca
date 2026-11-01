package com.yaca.core.feed

import com.yaca.core.model.AgentEventCheckpointCreated
import com.yaca.core.model.AgentEventCheckpointCreating
import com.yaca.core.model.AgentEventCheckpointCreationFinished
import com.yaca.core.model.AgentEventCheckpointPreviewed
import com.yaca.core.model.AgentEventCheckpointRestored
import com.yaca.core.model.AgentEventCheckpointsUpdated
import com.yaca.core.model.AgentEventContextCompacted
import com.yaca.core.model.AgentEventSummaryEdited
import com.yaca.core.model.CheckpointReasonMessage

// The feed's dividers and the lists behind them: checkpoints, rollback previews, restores,
// and context compaction.

internal fun FeedDraft.onCheckpointCreated(event: AgentEventCheckpointCreated) {
    placePoint(FeedBlock.Checkpoint(key("ckpt"), event.checkpoint))
}

internal fun FeedDraft.onCheckpointCreating(event: AgentEventCheckpointCreating) {
    val block = FeedBlock.CheckpointCreating(key("ckpt-live"), event.id)
    if (event.reason is CheckpointReasonMessage) {
        var at = blocks.lastIndex
        while (at >= 0) {
            val seen = blocks[at]
            if (seen !is FeedBlock.Hook || seen.blocked) break
            at--
        }
        if (at >= 0 && blocks[at] is FeedBlock.Human) {
            blocks.add(at + 1, block)
            return
        }
    }
    blocks.add(block)
}

internal fun FeedDraft.onCheckpointCreationFinished(event: AgentEventCheckpointCreationFinished) {
    blocks.removeAll { it is FeedBlock.CheckpointCreating && it.id == event.id }
}

internal fun FeedDraft.onCheckpointsUpdated(event: AgentEventCheckpointsUpdated) {
    checkpoints = event.checkpoints
}

internal fun FeedDraft.onCheckpointPreviewed(event: AgentEventCheckpointPreviewed) {
    preview = CheckpointPreview(event.id, event.files)
}

internal fun FeedDraft.onCheckpointRestored(event: AgentEventCheckpointRestored) {
    preview = null
    blocks.add(FeedBlock.Restored(key("back"), event.at, event.files))
}

internal fun FeedDraft.onContextCompacted(event: AgentEventContextCompacted) {
    blocks.add(FeedBlock.Compacted(key("compact"), event.summary, event.freed))
}

/**
 * The summary is edited, not written again: the divider stays the same.
 */
internal fun FeedDraft.onSummaryEdited(event: AgentEventSummaryEdited) {
    val at = blocks.indexOfLast { it is FeedBlock.Compacted }
    val block = blocks.getOrNull(at) as? FeedBlock.Compacted ?: return
    blocks[at] = block.copy(summary = event.summary)
}

/**
 * A message checkpoint is shown immediately below that message.
 *
 * The core sends the message's card first, so the feed is not empty while the snapshot is
 * taken, and the point after it, perhaps with the notes of the prompt hooks in between.
 * Journals from before that order carry the point first, and then the last card is an older
 * message with a turn under it, so the divider simply goes at the end. A blocked hook ends its
 * message without a point: the card above it is not this point's either.
 */
private fun FeedDraft.placePoint(block: FeedBlock.Checkpoint) {
    if (block.checkpoint.reason is CheckpointReasonMessage) {
        var at = blocks.lastIndex
        while (at >= 0) {
            val seen = blocks[at]
            if (seen !is FeedBlock.Hook || seen.blocked) break
            at -= 1
        }
        if (at >= 0 && blocks[at] is FeedBlock.Human) {
            blocks.add(at + 1, block)
            return
        }
    }
    blocks.add(block)
}
