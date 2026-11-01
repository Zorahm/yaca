package com.yaca.core.feed

import com.yaca.core.model.AgentEvent
import com.yaca.core.model.AgentEventTextShown
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Ports of the `isLiveWork`, `closesReply` and `replyText` groups of `src/core/work.test.ts`. */
class WorkQueriesTest {
    private fun shownFile(text: String): AgentEvent = AgentEventTextShown(
        path = "a.md",
        command = null,
        firstLine = 1,
        lines = 1,
        total = 1,
        text = text,
    )

    @Test
    fun `isLiveWork is the last work of a running turn, until the agent speaks under it`() {
        val feed = itemsOf(started("c1"), wrote("b1", "Next."), thought("t1", "hm"), started("c2"))

        assertTrue(isLiveWork(feed, 2, true))
        assertFalse(isLiveWork(feed, 0, true))
        assertFalse(isLiveWork(feed, 2, false))
    }

    @Test
    fun `closesReply is the last text of a reply, not a remark between stretches of work`() {
        val feed = itemsOf(wrote("b1", "Looking."), started("c1"), wrote("b2", "Done."), said("more"))

        assertFalse(closesReply(feed, 0, false))
        assertTrue(closesReply(feed, 2, false))
    }

    @Test
    fun `closesReply waits for the turn to end`() {
        val feed = itemsOf(said("hi"), wrote("b1", "Half a sen"))

        assertFalse(closesReply(feed, 1, true))
        assertTrue(closesReply(feed, 1, false))
    }

    @Test
    fun `closesReply is not closed by text a shown file follows`() {
        val feed = itemsOf(wrote("b1", "Here it is:"), shownFile("draft"))

        assertFalse(closesReply(feed, 0, false))
    }

    @Test
    fun `closesReply holds for an earlier reply while the next turn runs`() {
        val feed = itemsOf(wrote("b1", "Done."), said("more"), started("c1"))

        assertTrue(closesReply(feed, 0, true))
    }

    @Test
    fun `replyText gathers every piece of the model's reply, without the work between them`() {
        val feed = itemsOf(
            said("do it"),
            wrote("b1", "Looking."),
            started("c1"),
            wrote("b2", "Found it."),
            started("c2"),
            wrote("b3", "Done."),
        )

        assertEquals("Looking.\n\nFound it.\n\nDone.", replyText(feed, feed.size - 1))
    }

    @Test
    fun `replyText stops at the human message that began the reply`() {
        val feed = itemsOf(wrote("b0", "Earlier answer."), said("next"), wrote("b1", "Now."))

        assertEquals("Now.", replyText(feed, feed.size - 1))
    }

    @Test
    fun `replyText leaves out a file the agent showed`() {
        val feed = itemsOf(
            said("show"),
            wrote("b1", "Here it is:"),
            shownFile("draft"),
            wrote("b2", "That is all."),
        )

        assertEquals("Here it is:\n\nThat is all.", replyText(feed, feed.size - 1))
    }
}
