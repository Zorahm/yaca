package com.yaca.core.feed

import com.yaca.core.model.AgentEventFileViewLoaded
import com.yaca.core.model.ChangeKind
import com.yaca.core.model.FileFact
import com.yaca.core.model.FileView
import kotlin.test.Test
import kotlin.test.assertEquals

/** Ports of the `file panel (§6.4)` group. */
class FeedFilePanelTest {
    private val pendingFact = FileFact(path = "a.txt", kind = ChangeKind.MODIFIED, movedFrom = null, added = 1, removed = 0)

    @Test
    fun `each file write moves the counter the panel rereads its view by`() {
        val feed = runFeed(changed("c1", "a.txt", added = 1, removed = 0), changed("c2", "a.txt", added = 1, removed = 0))
        assertEquals(2, feed.writes)
    }

    @Test
    fun `the file view holds the core's latest answer for the path`() {
        val view = FileView(fact = pendingFact, segments = emptyList(), truncated = false)
        val feed = runFeed(AgentEventFileViewLoaded("a.txt", view))

        assertEquals(FileViewSlot("a.txt", view), feed.fileView)
    }

    @Test
    fun `'nothing to decide' arrives as null, not as a missing answer`() {
        val feed = runFeed(AgentEventFileViewLoaded("a.txt", null))

        assertEquals(FileViewSlot("a.txt", null), feed.fileView)
    }

    @Test
    fun `a new view replaces the previous one, even for another file`() {
        val view = FileView(
            fact = pendingFact.copy(path = "b.txt"),
            segments = emptyList(),
            truncated = false,
        )
        val feed = runFeed(
            AgentEventFileViewLoaded("a.txt", null),
            AgentEventFileViewLoaded("b.txt", view),
        )

        assertEquals("b.txt", feed.fileView?.path)
    }
}
