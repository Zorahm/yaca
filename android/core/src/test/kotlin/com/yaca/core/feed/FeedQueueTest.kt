package com.yaca.core.feed

import com.yaca.core.model.AgentEventFilesFound
import com.yaca.core.model.AgentEventQueueUpdated
import com.yaca.core.model.QueuedId
import com.yaca.core.model.QueuedMessage
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Ports of the `queue and file search (§7.4, §7.2)` group. */
class FeedQueueTest {
    private fun waiting(id: String, text: String): QueuedMessage =
        QueuedMessage(QueuedId(id), text, emptyList())

    @Test
    fun `shows exactly the list the core last sent, in its order`() {
        val feed = runFeed(
            AgentEventQueueUpdated(listOf(waiting("q1", "first"), waiting("q2", "second"))),
        )
        assertEquals(listOf(QueuedId("q1"), QueuedId("q2")), feed.queued.map { it.id })
    }

    @Test
    fun `drops a message the core no longer lists, and the empty list clears all`() {
        val after = runFeed(
            AgentEventQueueUpdated(listOf(waiting("q1", "a"), waiting("q2", "b"))),
            AgentEventQueueUpdated(listOf(waiting("q2", "b"))),
        )
        assertEquals(listOf(QueuedId("q2")), after.queued.map { it.id })

        val empty = reduceAll(after, listOf(AgentEventQueueUpdated(emptyList())))
        assertEquals(emptyList(), empty.queued)
    }

    @Test
    fun `takes the message into the conversation without touching the queue itself`() {
        val feed = runFeed(
            AgentEventQueueUpdated(listOf(waiting("q1", "later"))),
            AgentEventQueueUpdated(emptyList()),
            said("later"),
        )
        assertEquals(emptyList(), feed.queued)
        assertTrue(visible(feed).any { it is FeedBlock.Human && it.text == "later" })
    }

    @Test
    fun `keeps the answer to a search with the query it answers`() {
        val feed = runFeed(AgentEventFilesFound("app", listOf("src/app.rs")))
        assertEquals(FileSearchResult("app", listOf("src/app.rs")), feed.found)
    }

    @Test
    fun `the last answer wins, whichever query it is for`() {
        val feed = runFeed(
            AgentEventFilesFound("a", listOf("a.txt")),
            AgentEventFilesFound("ab", listOf("ab.txt")),
        )
        assertEquals("ab", feed.found?.query)
    }
}
