package com.yaca.core.feed

import com.yaca.core.model.AgentEvent
import com.yaca.core.model.AgentEventCheckpointCreated
import com.yaca.core.model.AgentEventCheckpointCreating
import com.yaca.core.model.AgentEventCheckpointCreationFinished
import com.yaca.core.model.AgentEventCheckpointPreviewed
import com.yaca.core.model.AgentEventCheckpointRestored
import com.yaca.core.model.AgentEventCheckpointsUpdated
import com.yaca.core.model.AgentEventContextCompacted
import com.yaca.core.model.AgentEventElicitationRequested
import com.yaca.core.model.AgentEventElicitationResolved
import com.yaca.core.model.AgentEventError
import com.yaca.core.model.AgentEventFileChanged
import com.yaca.core.model.AgentEventFileViewLoaded
import com.yaca.core.model.AgentEventFilesFound
import com.yaca.core.model.AgentEventFileSent
import com.yaca.core.model.AgentEventHookReported
import com.yaca.core.model.AgentEventLspUpdated
import com.yaca.core.model.AgentEventMcpUpdated
import com.yaca.core.model.AgentEventPermissionRequested
import com.yaca.core.model.AgentEventPermissionResolved
import com.yaca.core.model.AgentEventPlanProposed
import com.yaca.core.model.AgentEventProcessOutput
import com.yaca.core.model.AgentEventProcessOutputReset
import com.yaca.core.model.AgentEventProcessesUpdated
import com.yaca.core.model.AgentEventQueueUpdated
import com.yaca.core.model.AgentEventQuestionAsked
import com.yaca.core.model.AgentEventQuestionResolved
import com.yaca.core.model.AgentEventRetryScheduled
import com.yaca.core.model.AgentEventRouteDecided
import com.yaca.core.model.AgentEventStateChanged
import com.yaca.core.model.AgentEventSubagentUsage
import com.yaca.core.model.AgentEventSummaryEdited
import com.yaca.core.model.AgentEventTextDelta
import com.yaca.core.model.AgentEventTextShown
import com.yaca.core.model.AgentEventThinkingDelta
import com.yaca.core.model.AgentEventTitleSet
import com.yaca.core.model.AgentEventTodosUpdated
import com.yaca.core.model.AgentEventToolCallDenied
import com.yaca.core.model.AgentEventToolCallFailed
import com.yaca.core.model.AgentEventToolCallFinished
import com.yaca.core.model.AgentEventToolCallProgress
import com.yaca.core.model.AgentEventToolCallRejected
import com.yaca.core.model.AgentEventToolCallStarted
import com.yaca.core.model.AgentEventTurnCancelled
import com.yaca.core.model.AgentEventTurnFinished
import com.yaca.core.model.AgentEventTurnStarted
import com.yaca.core.model.AgentEventUsageUpdated
import com.yaca.core.model.AgentEventUserMessage

/**
 * A batch of events: one snapshot of the feed. An empty batch returns [feed] itself, so the
 * interface does not redraw for nothing.
 *
 * The `when` is exhaustive over the sealed [AgentEvent] without an `else`: a variant the client
 * gains is a compile error here, not a silently dropped event.
 */
fun reduceAll(feed: Feed, events: List<AgentEvent>): Feed {
    if (events.isEmpty()) return feed
    val draft = FeedDraft(feed)
    for (event in events) draft.applyEvent(event)
    return draft.freeze()
}

private fun FeedDraft.applyEvent(event: AgentEvent) {
    when (event) {
        is AgentEventUserMessage -> onUserMessage(event)
        is AgentEventTurnStarted -> onTurnStarted(event)
        is AgentEventTextDelta -> onTextDelta(event)
        is AgentEventThinkingDelta -> onThinkingDelta(event)
        is AgentEventTurnFinished -> onTurnFinished(event)
        is AgentEventTurnCancelled -> onTurnCancelled(event)
        is AgentEventError -> onError(event)
        is AgentEventRetryScheduled -> onRetryScheduled(event)
        is AgentEventStateChanged -> onStateChanged(event)
        is AgentEventPlanProposed -> onPlanProposed(event)
        // The chat list reads the title (`src/ui/chats.ts`); the feed does not draw it.
        is AgentEventTitleSet -> Unit

        is AgentEventCheckpointCreating -> onCheckpointCreating(event)
        is AgentEventCheckpointCreationFinished -> onCheckpointCreationFinished(event)
        is AgentEventCheckpointCreated -> onCheckpointCreated(event)
        is AgentEventCheckpointsUpdated -> onCheckpointsUpdated(event)
        is AgentEventCheckpointPreviewed -> onCheckpointPreviewed(event)
        is AgentEventCheckpointRestored -> onCheckpointRestored(event)
        is AgentEventContextCompacted -> onContextCompacted(event)
        is AgentEventSummaryEdited -> onSummaryEdited(event)

        is AgentEventTextShown -> onTextShown(event)
        // The remote wire carries metadata only until it supports file transfer.
        is AgentEventFileSent -> blocks.add(FeedBlock.Shown(
            key = key("sent"), path = event.file.name, command = null,
            firstLine = 1, lines = 1, total = 1,
            text = listOfNotNull(event.file.title, event.file.caption, event.file.mime,
                "${event.file.size} bytes", "SHA-256 ${event.file.sha256}").joinToString("\n"),
        ))
        is AgentEventFileChanged -> onFileChanged(event)
        is AgentEventFileViewLoaded -> onFileViewLoaded(event)
        is AgentEventFilesFound -> onFilesFound(event)

        is AgentEventPermissionRequested -> onPermissionRequested(event)
        is AgentEventPermissionResolved -> onPermissionResolved(event)
        is AgentEventQuestionAsked -> onQuestionAsked(event)
        is AgentEventQuestionResolved -> onQuestionResolved(event)
        is AgentEventElicitationRequested -> onElicitationRequested(event)
        is AgentEventElicitationResolved -> onElicitationResolved(event)
        is AgentEventQueueUpdated -> onQueueUpdated(event)

        is AgentEventProcessesUpdated -> onProcessesUpdated(event)
        is AgentEventProcessOutputReset -> onProcessOutputReset(event)
        is AgentEventProcessOutput -> onProcessOutput(event)
        is AgentEventMcpUpdated -> onMcpUpdated(event)
        is AgentEventLspUpdated -> onLspUpdated(event)
        is AgentEventRouteDecided -> onRouteDecided(event)
        is AgentEventUsageUpdated -> onUsageUpdated(event)

        is AgentEventToolCallStarted -> onToolCallStarted(event)
        is AgentEventToolCallProgress -> onToolCallProgress(event)
        is AgentEventToolCallFinished -> onToolCallFinished(event)
        is AgentEventToolCallFailed -> onToolCallFailed(event)
        is AgentEventToolCallRejected -> onToolCallRejected(event)
        is AgentEventToolCallDenied -> onToolCallDenied(event)
        is AgentEventSubagentUsage -> onSubagentUsage(event)
        is AgentEventTodosUpdated -> onTodosUpdated(event)
        is AgentEventHookReported -> onHookReported(event)
    }
}
