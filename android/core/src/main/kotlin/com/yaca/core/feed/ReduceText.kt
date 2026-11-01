package com.yaca.core.feed

import com.yaca.core.model.AgentEventError
import com.yaca.core.model.AgentEventHookReported
import com.yaca.core.model.AgentEventPlanProposed
import com.yaca.core.model.AgentEventRetryScheduled
import com.yaca.core.model.AgentEventRouteDecided
import com.yaca.core.model.AgentEventTextDelta
import com.yaca.core.model.AgentEventTextShown
import com.yaca.core.model.AgentEventThinkingDelta
import com.yaca.core.model.AgentEventUserMessage

// The model's words and the notices around them: the human's message, the replies and
// thoughts, shown files, plans, routing, plugin hooks, retries and errors.

internal fun FeedDraft.onUserMessage(event: AgentEventUserMessage) {
    blocks.add(FeedBlock.Human(key("human"), event.text, event.attachments))
}

internal fun FeedDraft.onTextDelta(event: AgentEventTextDelta) {
    val last = blocks.lastOrNull()
    if (last is FeedBlock.Agent && last.blockId == event.blockId) {
        blocks[blocks.lastIndex] = last.copy(text = last.text + event.text)
        return
    }
    blocks.add(FeedBlock.Agent(key("text"), event.blockId, event.text))
}

internal fun FeedDraft.onThinkingDelta(event: AgentEventThinkingDelta) {
    val last = blocks.lastOrNull()
    if (last is FeedBlock.Reasoning && last.blockId == event.blockId) {
        blocks[blocks.lastIndex] = last.copy(text = last.text + event.text)
        return
    }
    // No duration: the reasoning row asks for "reasoning · 12 s", but the stream has no
    // start time, and measuring it with the window's clock would show, in a session
    // reopened later, the time of a different run.
    blocks.add(FeedBlock.Reasoning(key("think"), event.blockId, event.text))
}

internal fun FeedDraft.onPlanProposed(event: AgentEventPlanProposed) {
    blocks.add(FeedBlock.Plan(key("plan"), event.plan))
}

internal fun FeedDraft.onTextShown(event: AgentEventTextShown) {
    blocks.add(
        FeedBlock.Shown(
            key = key("shown"),
            path = event.path,
            command = event.command,
            firstLine = event.firstLine,
            lines = event.lines,
            total = event.total,
            text = event.text,
        ),
    )
}

internal fun FeedDraft.onError(event: AgentEventError) {
    blocks.add(FeedBlock.Error(key("err"), event.code, event.message, event.retryable))
}

internal fun FeedDraft.onRetryScheduled(event: AgentEventRetryScheduled) {
    blocks.add(FeedBlock.Retry(key("retry"), event.attempt, event.of, event.delayMs))
}

/**
 * Which model answers this turn: a line of its own before the text, so the reader knows who
 * wrote it. The core sends it once per user message, not once per request of the tool loop.
 */
internal fun FeedDraft.onRouteDecided(event: AgentEventRouteDecided) {
    blocks.add(FeedBlock.Route(key("route"), event.route, event.provider, event.model))
}

internal fun FeedDraft.onHookReported(event: AgentEventHookReported) {
    blocks.add(FeedBlock.Hook(key("hook"), event.plugin, event.event, event.text, event.blocked))
}
