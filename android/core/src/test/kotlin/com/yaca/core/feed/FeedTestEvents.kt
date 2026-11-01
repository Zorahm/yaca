package com.yaca.core.feed

import com.yaca.core.model.AgentEvent
import com.yaca.core.model.AgentEventFileChanged
import com.yaca.core.model.AgentEventTextDelta
import com.yaca.core.model.AgentEventThinkingDelta
import com.yaca.core.model.AgentEventToolCallFinished
import com.yaca.core.model.AgentEventToolCallStarted
import com.yaca.core.model.AgentEventUserMessage
import com.yaca.core.model.BlockId
import com.yaca.core.model.CallId
import com.yaca.core.model.ChangeKind
import com.yaca.core.model.FileDiff
import com.yaca.core.model.FileFact
import com.yaca.core.model.ToolMeta
import com.yaca.core.model.TurnId
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject

// Builders of the events the feed tests fold. They mirror the helpers at the top of each
// `*.test.ts` file, so a test reads the same in both languages.

internal fun runFeed(vararg events: AgentEvent): Feed = reduceAll(Feed.EMPTY, events.toList())

/** The events as the work view draws them: reduced, then visible, then folded. */
internal fun itemsOf(vararg events: AgentEvent): List<FeedItem> = fold(visible(runFeed(*events)))

internal fun said(text: String): AgentEvent =
    AgentEventUserMessage(TurnId("t1"), text, emptyList())

internal fun wrote(blockId: String, text: String): AgentEvent =
    AgentEventTextDelta(TurnId("t1"), BlockId(blockId), text)

internal fun thought(blockId: String, text: String): AgentEvent =
    AgentEventThinkingDelta(TurnId("t1"), BlockId(blockId), text)

internal fun started(callId: String, tool: String = "Read", parent: String? = null): AgentEvent =
    AgentEventToolCallStarted(
        turnId = TurnId("t1"),
        callId = CallId(callId),
        tool = tool,
        description = "Read the file",
        target = "src/a.rs",
        input = JsonObject(emptyMap()),
        parent = parent?.let { CallId(it) },
    )

internal fun finished(callId: String): AgentEvent =
    AgentEventToolCallFinished(
        callId = CallId(callId),
        meta = ToolMeta(lines = 3, items = null, truncated = false, durationMs = 12),
    )

/** A file the call changed, with the same counts in its fact and its diff. */
internal fun changed(
    callId: String,
    path: String,
    added: Int = 2,
    removed: Int = 1,
    kind: ChangeKind = ChangeKind.MODIFIED,
): AgentEvent = AgentEventFileChanged(
    callId = CallId(callId),
    fact = FileFact(path = path, kind = kind, movedFrom = null, added = added, removed = removed),
    diff = FileDiff(hunks = emptyList(), added = added, removed = removed, truncated = false),
)

/** The `Task` input of the TS helpers: `{ prompt }`, with any JSON value as the prompt. */
internal fun promptOf(prompt: JsonElement): JsonObject = buildJsonObject { put("prompt", prompt) }

internal fun promptOf(prompt: String): JsonObject = promptOf(JsonPrimitive(prompt))

/** The action group at the top of the feed, for tests that expect exactly one. */
internal fun firstActions(feed: Feed): FeedBlock.Actions =
    checkNotNull(feed.blocks.first() as? FeedBlock.Actions) { "the first block is not an action group" }
