package com.yaca.core.feed

import com.yaca.core.model.AgentEventPermissionRequested
import com.yaca.core.model.AgentEventPermissionResolved
import com.yaca.core.model.AgentEventTurnCancelled
import com.yaca.core.model.ChangeKind
import com.yaca.core.model.PermissionKind
import com.yaca.core.model.PermissionRequest
import com.yaca.core.model.RequestId
import com.yaca.core.model.TsMillis
import com.yaca.core.model.TurnId
import com.yaca.core.model.CallId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull

/** Ports of the `subagent` group of `src/core/feed.test.ts`. */
class FeedSubagentTest {
    /** The top-level rows: what the group draws itself. */
    private fun top(feed: Feed): List<ActionRow> =
        feed.blocks.filterIsInstance<FeedBlock.Actions>().flatMap { it.rows }

    private fun asked(callId: String): PermissionRequest = PermissionRequest(
        id = RequestId("req_1"),
        callId = CallId(callId),
        kind = PermissionKind.EDIT_FILE,
        target = "notes.txt",
        detail = "",
        note = null,
        sizeBytes = null,
        files = null,
        invariant = null,
        unsaved = null,
        server = null,
        scopes = emptyList(),
        position = 1,
        total = 1,
    )

    @Test
    fun `its calls stand under the Task row, not next to it`() {
        val feed = runFeed(
            started("t1", "Task"),
            started("r1", "Read", "t1"),
            finished("r1"),
            started("r2", "Read", "t1"),
        )

        assertEquals(listOf("t1"), top(feed).map { it.callId.value })
        assertEquals(
            listOf("r1" to ActionState.DONE, "r2" to ActionState.RUNNING),
            top(feed)[0].children.map { it.callId.value to it.state },
        )
    }

    @Test
    fun `a permission request does not detach a call from its row`() {
        // The request closes the group, but a subagent's row is not pinned to the group: it goes
        // to its own Task, wherever that one was left.
        val feed = runFeed(
            started("t1", "Task"),
            started("w1", "Write", "t1"),
            AgentEventPermissionRequested(asked("w1")),
            AgentEventPermissionResolved(RequestId("req_1")),
            started("r1", "Read", "t1"),
        )

        assertEquals(listOf("t1"), top(feed).map { it.callId.value })
        assertEquals(listOf("w1", "r1"), top(feed)[0].children.map { it.callId.value })
    }

    @Test
    fun `two subagents do not mix up each other's rows`() {
        val feed = runFeed(
            started("t1", "Task"),
            started("t2", "Task"),
            started("a", "Read", "t2"),
            started("b", "Read", "t1"),
        )

        val (first, second) = top(feed)
        assertEquals(listOf("b"), first.children.map { it.callId.value })
        assertEquals(listOf("a"), second.children.map { it.callId.value })
    }

    @Test
    fun `a file created by a subagent is visible to the file panel and the touched list`() {
        val feed = runFeed(
            started("t1", "Task"),
            started("w1", "Write", "t1"),
            changed("w1", "notes.txt", added = 3, removed = 0, kind = ChangeKind.CREATED),
            finished("w1"),
        )

        assertEquals("notes.txt", top(feed)[0].children[0].fact?.path)
        assertEquals(listOf("notes.txt"), touchedFiles(feed))
        assertEquals(ChangeKind.CREATED, factOf(feed, "notes.txt")?.kind)
        assertNotNull(diffOf(feed.copy(diffs = emptyMap()), "notes.txt"))
    }

    @Test
    fun `stopping a turn also stops its unfinished calls`() {
        val feed = runFeed(
            started("t1", "Task"),
            started("r1", "Read", "t1"),
            AgentEventTurnCancelled(TurnId("t1"), TsMillis(0)),
        )

        // A live dot on a call nobody is running any more is a lie.
        assertEquals(
            listOf(ActionState.FAILED, ActionState.FAILED),
            flatRows(top(feed)).map { it.state },
        )
    }

    @Test
    fun `a call without its own Task is not lost from the feed`() {
        val feed = runFeed(started("r1", "Read", "t-none"))
        assertEquals(listOf("r1"), top(feed).map { it.callId.value })
    }
}
