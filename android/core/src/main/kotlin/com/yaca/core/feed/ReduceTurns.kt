package com.yaca.core.feed

import com.yaca.core.model.AgentEventTurnCancelled
import com.yaca.core.model.AgentEventTurnFinished
import com.yaca.core.model.AgentEventTurnStarted
import com.yaca.core.model.TsMillis

// The run's lifecycle. The ring and the send button read `state.busy`, which arrives as its
// own event; the turn's time is drawn separately, by a live timer and a line under the answer.

internal fun FeedDraft.onTurnStarted(event: AgentEventTurnStarted) {
    turn = RunningTurn(event.at)
}

internal fun FeedDraft.onTurnFinished(event: AgentEventTurnFinished) {
    endTurn(event.at)
}

/** Unfinished tools end as cancelled; the feed gets a line for the stop. */
internal fun FeedDraft.onTurnCancelled(event: AgentEventTurnCancelled) {
    cancelRunning()
    blocks.add(FeedBlock.Stopped(key("stop")))
    endTurn(event.at)
}

/**
 * Closes the running turn's clock. No time line when either end is unknown: a journal from
 * before the core stamped turns carries `at: 0`, and "worked 492 000 hours" is worse than
 * nothing.
 */
private fun FeedDraft.endTurn(endedAt: TsMillis) {
    val started = turn?.startedAt?.value ?: 0L
    turn = null
    if (started <= 0L || endedAt.value < started) return
    blocks.add(FeedBlock.TurnTime(key("time"), TsMillis(started), endedAt))
}
