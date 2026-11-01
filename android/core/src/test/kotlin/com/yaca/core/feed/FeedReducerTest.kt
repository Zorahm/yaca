package com.yaca.core.feed

import com.yaca.core.model.AgentEventContextCompacted
import com.yaca.core.model.AgentEventElicitationRequested
import com.yaca.core.model.AgentEventElicitationResolved
import com.yaca.core.model.AgentEventPermissionRequested
import com.yaca.core.model.AgentEventPermissionResolved
import com.yaca.core.model.AgentEventSummaryEdited
import com.yaca.core.model.AgentEventTodosUpdated
import com.yaca.core.model.AgentEventToolCallFailed
import com.yaca.core.model.AgentEventToolCallProgress
import com.yaca.core.model.AgentEventTurnCancelled
import com.yaca.core.model.CallId
import com.yaca.core.model.ElicitationReplyDecline
import com.yaca.core.model.ElicitationRequest
import com.yaca.core.model.PermissionKind
import com.yaca.core.model.PermissionRequest
import com.yaca.core.model.RequestId
import com.yaca.core.model.TodoItem
import com.yaca.core.model.TodoStatus
import com.yaca.core.model.TsMillis
import com.yaca.core.model.TurnId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertSame

/** Golden tests of the feed, a port of `src/core/feed.test.ts` ("feed" group). */
class FeedReducerTest {
    @Test
    fun `joins the deltas of one block into one reply`() {
        val feed = runFeed(wrote("b1", "Looking "), wrote("b1", "at the parser."))

        assertEquals(1, feed.blocks.size)
        assertEquals("Looking at the parser.", assertIs<FeedBlock.Agent>(feed.blocks[0]).text)
    }

    @Test
    fun `a new model block is a new reply`() {
        val feed = runFeed(wrote("b1", "one"), wrote("b2", "two"))
        assertEquals(2, feed.blocks.size)
    }

    @Test
    fun `collects consecutive actions into one group`() {
        val feed = runFeed(started("c1"), started("c2"), started("c3"))

        assertEquals(1, feed.blocks.size)
        assertEquals(3, firstActions(feed).rows.size)
    }

    @Test
    fun `a reply and reasoning between actions stand between them`() {
        // The reducer keeps the order; the turn's work is gathered into blocks by `fold`.
        val feed = runFeed(
            started("c1"),
            started("c2"),
            thought("t1", "now the tests"),
            started("c3"),
            wrote("b1", "now the styles"),
            started("c4"),
        )

        assertEquals(
            listOf(
                FeedBlock.Actions::class,
                FeedBlock.Reasoning::class,
                FeedBlock.Actions::class,
                FeedBlock.Agent::class,
                FeedBlock.Actions::class,
            ),
            feed.blocks.map { it::class },
        )
        assertEquals(2, firstActions(feed).rows.size)
    }

    @Test
    fun `an action's outcome reaches its own row`() {
        val feed = runFeed(
            started("c1"),
            started("c2"),
            finished("c1"),
            AgentEventToolCallFailed(CallId("c2"), "not found"),
        )

        val (one, two) = firstActions(feed).rows
        assertEquals(ActionState.DONE, one.state)
        assertEquals(12, one.meta?.durationMs)
        assertEquals(ActionState.FAILED, two.state)
        assertEquals("not found", two.message)
    }

    @Test
    fun `command output accumulates in its own row`() {
        val feed = runFeed(
            started("c1", "Bash"),
            AgentEventToolCallProgress(CallId("c1"), "one\n"),
            AgentEventToolCallProgress(CallId("c1"), "two\n"),
        )

        assertEquals("one\ntwo\n", firstActions(feed).rows[0].output)
    }

    @Test
    fun `an edit arrives in its row with the diff and the counters`() {
        val feed = runFeed(started("c1", "Edit"), changed("c1", "src/a.rs", added = 4, removed = 2))

        val row = firstActions(feed).rows[0]
        assertEquals(4, row.fact?.added)
        assertNotNull(row.diff)
    }

    @Test
    fun `a rename puts each file in a row under it`() {
        val feed = runFeed(
            started("c1", "Rename"),
            changed("c1", "src/a.rs", added = 2, removed = 2),
            changed("c1", "src/b.rs", added = 1, removed = 1),
            changed("c1", "src/a.rs", added = 3, removed = 3),
        )

        // The row is the rename; the files are under it, one row per file.
        val row = firstActions(feed).rows[0]
        assertNull(row.fact)
        assertEquals(listOf("src/a.rs", "src/b.rs"), row.children.map { it.fact?.path })
        assertEquals(3, row.children[0].fact?.added)
        assertNotNull(feed.diffs["src/b.rs"])
    }

    @Test
    fun `a server form appears once, however many times it arrives, and closes on an answer`() {
        val request = ElicitationRequest(
            id = RequestId("req_1"),
            server = "billing",
            message = "Which account?",
            fields = emptyList(),
        )
        val feed = runFeed(
            AgentEventElicitationRequested(request),
            AgentEventElicitationRequested(request),
            AgentEventElicitationResolved(RequestId("req_1"), ElicitationReplyDecline),
        )

        val forms = feed.blocks.filterIsInstance<FeedBlock.Elicitation>()
        assertEquals(1, forms.size)
        assertEquals(ElicitationStatus.DECLINED, forms[0].status)
    }

    @Test
    fun `the todo list is updated in place`() {
        // No new block is created: the list stays where it first appeared.
        val feed = runFeed(
            AgentEventTodosUpdated(listOf(TodoItem("one", TodoStatus.PENDING))),
            wrote("b1", "working"),
            AgentEventTodosUpdated(listOf(TodoItem("one", TodoStatus.COMPLETED))),
        )

        assertEquals(1, feed.blocks.count { it is FeedBlock.Todos })
        val todos = assertIs<FeedBlock.Todos>(feed.blocks[0])
        assertEquals(TodoStatus.COMPLETED, todos.items[0].status)
    }

    @Test
    fun `an answered permission request leaves the feed but not the history`() {
        val request = PermissionRequest(
            id = RequestId("req_1"),
            callId = CallId("c1"),
            kind = PermissionKind.RUN_COMMAND,
            target = "npm",
            detail = "npm ci",
            note = "/p",
            sizeBytes = null,
            files = null,
            invariant = null,
            unsaved = null,
            server = null,
            scopes = emptyList(),
            position = 1,
            total = 1,
        )
        val feed = runFeed(
            AgentEventPermissionRequested(request),
            AgentEventPermissionResolved(RequestId("req_1")),
        )

        assertNull(feed.request)
        assertEquals(0, visible(feed).size)
        // Blocks only get added: the indices and the replay from the journal rely on that.
        assertEquals(1, feed.blocks.size)
    }

    @Test
    fun `stopping a turn marks unfinished actions`() {
        val feed = runFeed(
            started("c1"),
            started("c2"),
            finished("c1"),
            AgentEventTurnCancelled(TurnId("t1"), TsMillis(0)),
        )

        assertEquals(listOf(ActionState.DONE, ActionState.FAILED), firstActions(feed).rows.map { it.state })
        assertIs<FeedBlock.Stopped>(feed.blocks[1])
    }

    @Test
    fun `editing the summary does not add a second divider`() {
        // The summary is edited before continuing, not written again.
        val feed = runFeed(
            AgentEventContextCompacted(summary = "first", freed = 84000),
            AgentEventSummaryEdited(summary = "edited"),
        )

        assertEquals(1, feed.blocks.size)
        val compacted = assertIs<FeedBlock.Compacted>(feed.blocks[0])
        assertEquals("edited", compacted.summary)
        assertEquals(84000, compacted.freed)
    }

    @Test
    fun `the human's message comes from the stream, not from sending`() {
        // Otherwise a reopened session would show answers without questions.
        val feed = runFeed(said("fix the date parsing"), wrote("b1", "looking"))

        assertEquals("fix the date parsing", assertIs<FeedBlock.Human>(feed.blocks[0]).text)
    }

    @Test
    fun `an empty batch creates no new snapshot`() {
        // Redraw per frame, not per event.
        val feed = runFeed(wrote("b1", "one"))
        assertSame(feed, reduceAll(feed, emptyList()))
    }
}
