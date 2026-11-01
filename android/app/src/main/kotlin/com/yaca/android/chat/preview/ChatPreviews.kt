package com.yaca.android.chat.preview

import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import com.yaca.android.chat.ChatUi
import com.yaca.android.chat.FeedActions
import com.yaca.android.chat.composer.Composer
import com.yaca.android.chat.ComposerActions
import com.yaca.android.chat.ComposerDraft
import com.yaca.android.chat.feed.FeedList
import com.yaca.android.chat.sheets.PermissionSheet
import com.yaca.android.ui.theme.YacaTheme
import com.yaca.core.feed.Feed
import com.yaca.core.feed.fold
import com.yaca.core.feed.reduceAll
import com.yaca.core.feed.visible
import com.yaca.core.model.AgentEvent
import com.yaca.core.model.AgentEventFileChanged
import com.yaca.core.model.AgentEventTextDelta
import com.yaca.core.model.AgentEventToolCallFinished
import com.yaca.core.model.AgentEventToolCallStarted
import com.yaca.core.model.AgentEventTurnFinished
import com.yaca.core.model.AgentEventTurnStarted
import com.yaca.core.model.AgentEventUserMessage
import com.yaca.core.model.BlockId
import com.yaca.core.model.CallId
import com.yaca.core.model.ChangeKind
import com.yaca.core.model.FileDiff
import com.yaca.core.model.FileFact
import com.yaca.core.model.PermissionKind
import com.yaca.core.model.PermissionRequest
import com.yaca.core.model.RequestId
import com.yaca.core.model.ScopeKeyCommandPrefix
import com.yaca.core.model.StopReason
import com.yaca.core.model.ToolMeta
import com.yaca.core.model.TsMillis
import com.yaca.core.model.TurnId
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/** A short turn as the reducer folds it: a message, a read, an edit with its counters, and the answer. */
internal fun sampleFeed(): Feed {
    val turn = TurnId("t1")
    val events: List<AgentEvent> = listOf(
        AgentEventUserMessage(turnId = turn, text = "Fix the parser on empty blocks and add a test.", attachments = listOf("src/lexer.rs")),
        AgentEventTurnStarted(turnId = turn, at = TsMillis(1_000L)),
        AgentEventTextDelta(
            turnId = turn,
            blockId = BlockId("b1"),
            text = "Found it: `skip_blank` reads past the end of the block.\n\nThe fix is a bounds check:\n\n```rust\nif self.pos == start { return None; }\n```",
        ),
        AgentEventToolCallStarted(
            turnId = turn,
            callId = CallId("c1"),
            tool = "Read",
            target = "src/lexer.rs",
            input = JsonObject(mapOf("path" to JsonPrimitive("src/lexer.rs"))),
        ),
        AgentEventToolCallFinished(callId = CallId("c1"), meta = ToolMeta(lines = 2, truncated = false, durationMs = 14)),
        AgentEventToolCallStarted(
            turnId = turn,
            callId = CallId("c2"),
            tool = "Edit",
            description = "Add the bounds check",
            target = "src/lexer.rs",
            input = JsonObject(emptyMap()),
        ),
        AgentEventFileChanged(
            callId = CallId("c2"),
            fact = FileFact(path = "src/lexer.rs", kind = ChangeKind.MODIFIED, added = 6, removed = 1),
            diff = FileDiff(hunks = emptyList(), added = 6, removed = 1, truncated = false),
        ),
        AgentEventToolCallFinished(callId = CallId("c2"), meta = ToolMeta(lines = 0, truncated = false, durationMs = 40)),
        AgentEventTextDelta(turnId = turn, blockId = BlockId("b2"), text = "An empty block now yields a token."),
        AgentEventTurnFinished(turnId = turn, stopReason = StopReason.END_TURN, at = TsMillis(9_000L)),
    )
    return reduceAll(Feed.EMPTY, events)
}

@Preview(name = "Feed, light", showBackground = true)
@Composable
private fun FeedPreview() {
    val feed = sampleFeed()
    YacaTheme(dynamic = false) {
        FeedList(
            ui = ChatUi(feed = feed, items = fold(visible(feed))),
            listState = rememberLazyListState(),
            actions = FeedActions(onOpenDiff = {}, onOpenFile = {}, onOpenRequest = {}, onOpenCheckpoints = {}),
        )
    }
}

@Preview(name = "Composer, light", showBackground = true)
@Composable
private fun ComposerPreview() {
    YacaTheme(dynamic = false) {
        Composer(
            feed = sampleFeed(),
            models = emptyList(),
            draft = ComposerDraft(text = "Add a test for a block of spaces", attachments = listOf("src/lexer.rs")),
            actions = ComposerActions(
                onTextChange = {},
                onSend = {},
                onStop = {},
                onMention = {},
                onRemoveAttachment = {},
                onMode = {},
                onModel = {},
                onEffort = {},
                onCancelQueued = {},
            ),
        )
    }
}

@Preview(name = "Permission sheet, light")
@Composable
private fun PermissionPreview() {
    YacaTheme(dynamic = false) {
        PermissionSheet(
            request = PermissionRequest(
                id = RequestId("r1"),
                callId = CallId("c3"),
                kind = PermissionKind.RUN_COMMAND,
                target = "git",
                detail = "git commit -m \"fix(parser): skip blank block\"",
                note = "in the project root",
                scopes = listOf(ScopeKeyCommandPrefix(prefix = "git commit")),
                position = 1,
                total = 1,
            ),
            reason = "Save the fix once the tests pass.",
            onAnswer = {},
            onDismiss = {},
        )
    }
}
