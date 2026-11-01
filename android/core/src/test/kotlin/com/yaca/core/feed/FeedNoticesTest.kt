package com.yaca.core.feed

import com.yaca.core.model.AgentEvent
import com.yaca.core.model.AgentEventCheckpointCreated
import com.yaca.core.model.AgentEventHookReported
import com.yaca.core.model.AgentEventRouteDecided
import com.yaca.core.model.AgentEventTextShown
import com.yaca.core.model.AgentEventTurnCancelled
import com.yaca.core.model.AgentEventTurnFinished
import com.yaca.core.model.AgentEventTurnStarted
import com.yaca.core.model.CheckpointId
import com.yaca.core.model.CheckpointReason
import com.yaca.core.model.CheckpointReasonMessage
import com.yaca.core.model.CheckpointReasonRollback
import com.yaca.core.model.CheckpointSummary
import com.yaca.core.model.StopReason
import com.yaca.core.model.TsMillis
import com.yaca.core.model.TurnId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

/**
 * Ports of the `turn time`, `shown file text`, `plugin hooks in the feed`, `the point taken for a
 * message` and `the model router in the feed` groups of `src/core/feed.test.ts`.
 */
class FeedNoticesTest {
    private val begin: AgentEvent = AgentEventTurnStarted(TurnId("t1"), TsMillis(1_000))

    private fun finish(at: Long): AgentEvent =
        AgentEventTurnFinished(TurnId("t1"), StopReason.END_TURN, TsMillis(at))

    private fun shown(text: String): AgentEvent = AgentEventTextShown(
        path = "notes.md",
        command = null,
        firstLine = 2,
        lines = 2,
        total = 5,
        text = text,
    )

    private fun point(reason: CheckpointReason = CheckpointReasonMessage): AgentEvent =
        AgentEventCheckpointCreated(
            CheckpointSummary(
                id = CheckpointId("ckpt_${reason.javaClass.simpleName}"),
                at = TsMillis(1),
                reason = reason,
                tracked = 3,
            ),
        )

    private fun note(blocked: Boolean): AgentEvent =
        AgentEventHookReported("tools", "UserPromptSubmit", "noted", blocked)

    private fun kinds(feed: Feed): List<Any> = feed.blocks.map { it::class }

    private fun routed(model: String, route: String?): AgentEvent =
        AgentEventRouteDecided(route = route, provider = "gateway", model = model, matched = emptyList())

    // turn time

    @Test
    fun `a running turn carries its start time, a finished one clears it`() {
        assertEquals(RunningTurn(TsMillis(1_000)), runFeed(begin).turn)
        val done = runFeed(begin, finish(5_000))
        assertEquals(null, done.turn)
    }

    @Test
    fun `a line with the duration and the end time stands under the answer`() {
        val feed = runFeed(begin, wrote("b1", "done"), finish(134_000))

        val last = assertIs<FeedBlock.TurnTime>(feed.blocks.last())
        assertEquals(TsMillis(1_000), last.startedAt)
        assertEquals(TsMillis(134_000), last.endedAt)
    }

    @Test
    fun `a stopped turn also shows how long it ran`() {
        val feed = runFeed(begin, AgentEventTurnCancelled(TurnId("t1"), TsMillis(4_000)))
        assertEquals(listOf(FeedBlock.Stopped::class, FeedBlock.TurnTime::class), kinds(feed))
    }

    @Test
    fun `without an end time (a journal from an older build) there is no line`() {
        val feed = runFeed(begin, finish(0))
        assertTrue(feed.blocks.none { it is FeedBlock.TurnTime })
    }

    // shown file text

    @Test
    fun `appears as its own block with the path and the lines`() {
        val feed = runFeed(shown("two\nthree"))

        assertEquals(1, feed.blocks.size)
        val block = assertIs<FeedBlock.Shown>(feed.blocks[0])
        assertEquals("notes.md", block.path)
        assertEquals(2, block.firstLine)
        assertEquals(2, block.lines)
        assertEquals(5, block.total)
        assertEquals("two\nthree", block.text)
    }

    @Test
    fun `does not merge with the agent's text around it`() {
        val feed = runFeed(wrote("b1", "Here is the file:"), shown("two"), wrote("b1", "That's all."))

        assertEquals(listOf(FeedBlock.Agent::class, FeedBlock.Shown::class, FeedBlock.Agent::class), kinds(feed))
        assertEquals("That's all.", assertIs<FeedBlock.Agent>(feed.blocks[2]).text)
    }

    // plugin hooks in the feed

    @Test
    fun `a hook that blocked a message stands under that message`() {
        val feed = runFeed(
            said("deploy"),
            AgentEventHookReported("tools", "UserPromptSubmit", "not today", blocked = true),
        )

        assertEquals(listOf(FeedBlock.Human::class, FeedBlock.Hook::class), kinds(feed))
        val hook = assertIs<FeedBlock.Hook>(feed.blocks[1])
        assertTrue(hook.blocked)
        assertEquals("not today", hook.text)
    }

    // the point taken for a message

    @Test
    fun `stands below the card it follows in the stream`() {
        assertEquals(
            listOf(FeedBlock.Human::class, FeedBlock.Checkpoint::class),
            kinds(runFeed(said("one"), point())),
        )
    }

    @Test
    fun `stands below the card before the notes of the prompt hooks`() {
        assertEquals(
            listOf(FeedBlock.Human::class, FeedBlock.Checkpoint::class, FeedBlock.Hook::class),
            kinds(runFeed(said("one"), note(false), point())),
        )
    }

    @Test
    fun `stays where it is in a journal written point first`() {
        val feed = runFeed(point(), said("one"), wrote("b1", "done"), point(), said("two"))

        assertEquals(
            listOf(
                FeedBlock.Checkpoint::class,
                FeedBlock.Human::class,
                FeedBlock.Agent::class,
                FeedBlock.Checkpoint::class,
                FeedBlock.Human::class,
            ),
            kinds(feed),
        )
    }

    @Test
    fun `does not climb over a message a hook refused`() {
        val feed = runFeed(said("deploy"), note(true), point(), said("two"))

        assertEquals(
            listOf(FeedBlock.Human::class, FeedBlock.Hook::class, FeedBlock.Checkpoint::class, FeedBlock.Human::class),
            kinds(feed),
        )
    }

    @Test
    fun `leaves points of other reasons where they come`() {
        assertEquals(
            listOf(FeedBlock.Human::class, FeedBlock.Checkpoint::class),
            kinds(runFeed(said("one"), point(CheckpointReasonRollback))),
        )
    }

    // the model router in the feed

    @Test
    fun `puts which model answers ahead of its text`() {
        val feed = runFeed(said("hi"), routed("big", "hard-code"), wrote("b1", "answer"))
        assertEquals(listOf(FeedBlock.Human::class, FeedBlock.Route::class, FeedBlock.Agent::class), kinds(feed))

        val note = assertIs<FeedBlock.Route>(feed.blocks[1])
        assertEquals("hard-code", note.route)
        assertEquals("gateway", note.provider)
        assertEquals("big", note.model)
    }

    @Test
    fun `keeps the default route, which has no name`() {
        val feed = runFeed(said("hi"), routed("big", null))
        assertEquals(null, assertIs<FeedBlock.Route>(feed.blocks[1]).route)
    }

    @Test
    fun `shows each turn's model, since the next message may go elsewhere`() {
        val feed = runFeed(said("one"), routed("fast", null), said("two"), routed("big", "hard-code"))
        assertEquals(2, feed.blocks.count { it is FeedBlock.Route })
    }
}
