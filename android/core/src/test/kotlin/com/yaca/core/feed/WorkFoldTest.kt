package com.yaca.core.feed

import com.yaca.core.model.AgentEvent
import com.yaca.core.model.AgentEventPermissionRequested
import com.yaca.core.model.AgentEventPermissionResolved
import com.yaca.core.model.AgentEventToolCallStarted
import com.yaca.core.model.AgentEventTodosUpdated
import com.yaca.core.model.CallId
import com.yaca.core.model.PermissionKind
import com.yaca.core.model.PermissionRequest
import com.yaca.core.model.RequestId
import com.yaca.core.model.TodoItem
import com.yaca.core.model.TodoStatus
import com.yaca.core.model.TurnId
import kotlinx.serialization.json.JsonObject
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

/** Ports of the `fold` group of `src/core/work.test.ts`. */
class WorkFoldTest {
    private fun sent(callId: String): AgentEvent = AgentEventToolCallStarted(
        turnId = TurnId("t1"),
        callId = CallId(callId),
        tool = "Task",
        description = "Looked around",
        target = null,
        input = promptOf("look around"),
        parent = null,
    )

    private fun under(parent: String, callId: String): AgentEvent = AgentEventToolCallStarted(
        turnId = TurnId("t1"),
        callId = CallId(callId),
        tool = "Read",
        description = null,
        target = "src/a.rs",
        input = JsonObject(emptyMap()),
        parent = CallId(parent),
    )

    private fun asked(id: String, callId: String): AgentEvent = AgentEventPermissionRequested(
        PermissionRequest(
            id = RequestId(id),
            callId = CallId(callId),
            kind = PermissionKind.RUN_COMMAND,
            target = "npm",
            detail = "npm test",
            note = null,
            sizeBytes = null,
            files = null,
            invariant = null,
            unsaved = null,
            server = null,
            scopes = emptyList(),
            position = 1,
            total = 1,
        ),
    )

    private fun steps(item: FeedItem): List<Any> {
        val work = assertIs<FeedItem.Work>(item)
        return work.steps.map { it::class }
    }

    @Test
    fun `folds calls and thoughts into one item, and text divides the work`() {
        val feed = itemsOf(
            said("fix it"),
            thought("t1", "where is it"),
            started("c1"),
            started("c2"),
            thought("t2", "one more"),
            started("c3"),
            wrote("b1", "Now the tests."),
            thought("t3", "which ones"),
            started("c4"),
            wrote("b2", "Done."),
        )

        assertEquals(
            listOf(FeedBlock.Human::class, FeedItem.Work::class, FeedBlock.Agent::class, FeedItem.Work::class, FeedBlock.Agent::class),
            feed.map { it::class },
        )
        assertEquals(
            listOf(
                FeedItem.WorkStep.Thought::class,
                FeedItem.WorkStep.Action::class,
                FeedItem.WorkStep.Action::class,
                FeedItem.WorkStep.Thought::class,
                FeedItem.WorkStep.Action::class,
            ),
            steps(feed[1]),
        )
        assertEquals(listOf(FeedItem.WorkStep.Thought::class, FeedItem.WorkStep.Action::class), steps(feed[3]))
    }

    @Test
    fun `takes subagents out of the work, as one item for those sent together`() {
        val feed = itemsOf(
            said("survey it"),
            started("c1"),
            sent("a1"),
            sent("a2"),
            under("a1", "a1c1"),
            started("c2"),
            wrote("b1", "Done."),
        )

        assertEquals(
            listOf(FeedBlock.Human::class, FeedItem.Work::class, FeedItem.Agents::class, FeedItem.Work::class, FeedBlock.Agent::class),
            feed.map { it::class },
        )
        assertEquals(listOf(FeedItem.WorkStep.Action::class), steps(feed[1]))
        val agents = assertIs<FeedItem.Agents>(feed[2])
        assertEquals(listOf("a1", "a2"), agents.rows.map { it.callId.value })
        // What a subagent did stays on its row, for the panel; it is no step here.
        assertEquals(listOf("a1c1"), agents.rows[0].children.map { it.callId.value })
        assertEquals(listOf(FeedItem.WorkStep.Action::class), steps(feed[3]))
    }

    @Test
    fun `gives agents sent after other work an item of their own`() {
        val feed = itemsOf(sent("a1"), started("c1"), sent("a2"))

        assertEquals(listOf(FeedItem.Agents::class, FeedItem.Work::class, FeedItem.Agents::class), feed.map { it::class })
        assertEquals(3, feed.map { it.key }.toSet().size)
    }

    @Test
    fun `keeps the item's key while it grows`() {
        // One feed, reduced further: keys are minted per block, not per content.
        val one = reduceAll(Feed.EMPTY, listOf(thought("t1", "hm"), started("c1")))
        val two = reduceAll(one, listOf(thought("t2", "next"), started("c2")))
        assertEquals(fold(visible(one))[0].key, fold(visible(two))[0].key)
    }

    @Test
    fun `folds a thought with no calls`() {
        val feed = itemsOf(said("why"), thought("t1", "because"), wrote("b1", "Because."))

        assertEquals(listOf(FeedBlock.Human::class, FeedItem.Work::class, FeedBlock.Agent::class), feed.map { it::class })
        assertEquals(listOf(FeedItem.WorkStep.Thought::class), steps(feed[1]))
    }

    @Test
    fun `stands the todo list under the work it appeared in`() {
        val feed = itemsOf(
            started("c1"),
            AgentEventTodosUpdated(listOf(TodoItem("one", TodoStatus.PENDING))),
            started("c2"),
            wrote("b1", "Done."),
        )

        assertEquals(listOf(FeedItem.Work::class, FeedBlock.Todos::class, FeedBlock.Agent::class), feed.map { it::class })
        assertEquals(listOf(FeedItem.WorkStep.Action::class, FeedItem.WorkStep.Action::class), steps(feed[0]))
    }

    @Test
    fun `is cut by an open permission request and joined once it is answered`() {
        val waiting = reduceAll(Feed.EMPTY, listOf(started("c1"), asked("r1", "c2"), started("c2")))
        val open = fold(visible(waiting))
        assertEquals(listOf(FeedItem.Work::class, FeedBlock.Permission::class, FeedItem.Work::class), open.map { it::class })

        val answered = reduceAll(waiting, listOf(AgentEventPermissionResolved(RequestId("r1"))))
        val closed = fold(visible(answered))
        assertEquals(listOf(FeedItem.Work::class), closed.map { it::class })
        assertEquals(open[0].key, closed[0].key)
    }
}
