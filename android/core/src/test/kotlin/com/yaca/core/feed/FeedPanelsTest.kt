package com.yaca.core.feed

import com.yaca.core.model.AgentEventProcessOutput
import com.yaca.core.model.AgentEventProcessOutputReset
import com.yaca.core.model.ProcessId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull

/** Ports of the `panels` group of `src/core/feed.test.ts`. */
class FeedPanelsTest {
    @Test
    fun `an edit feeds the file panel's index`() {
        val feed = runFeed(started("c1", "Edit"), changed("c1", "a.rs"))

        assertNotNull(feed.diffs["a.rs"])
        assertEquals(feed.diffs["a.rs"], diffOf(feed, "a.rs"))
    }

    @Test
    fun `diffOf falls back to the action row's diff`() {
        val feed = runFeed(started("c3", "Edit"), changed("c3", "src/a.rs"))

        // The index and the row hold one event; this checks the fallback itself, by dropping the index.
        val withoutIndex = feed.copy(diffs = emptyMap())
        assertNotNull(diffOf(withoutIndex, "src/a.rs"))
        assertEquals(null, diffOf(withoutIndex, "src/other.rs"))
    }

    @Test
    fun `the last edit of a path wins`() {
        // The file panel shows the file as it became, not its first edit.
        val feed = runFeed(
            started("c1", "Edit"),
            changed("c1", "a.rs", added = 2),
            started("c2", "Edit"),
            changed("c2", "a.rs", added = 5),
        )

        assertEquals(5, feed.diffs["a.rs"]?.added)
        assertEquals(5, factOf(feed, "a.rs")?.added)
        assertEquals(listOf("a.rs"), touchedFiles(feed))
    }

    @Test
    fun `process output - a reset puts the buffer, chunks are appended to their own id`() {
        val feed = runFeed(
            AgentEventProcessOutputReset(ProcessId(3), "ready\n", 0L),
            AgentEventProcessOutput(ProcessId(3), "compiled\n"),
        )

        assertEquals(ProcessOutputBuffer(ProcessId(3), "ready\ncompiled\n", 0L), feed.processOutput)
    }

    @Test
    fun `a chunk from another process does not replace the selected buffer`() {
        // A selection is announced by a reset; a chunk without a reset is not the one being looked at.
        val feed = runFeed(
            AgentEventProcessOutputReset(ProcessId(3), "ready\n", 0L),
            AgentEventProcessOutput(ProcessId(7), "other\n"),
        )

        assertEquals(ProcessOutputBuffer(ProcessId(3), "ready\n", 0L), feed.processOutput)
    }

    @Test
    fun `a new selection resets the whole buffer`() {
        val feed = runFeed(
            AgentEventProcessOutputReset(ProcessId(3), "ready\n", 0L),
            AgentEventProcessOutput(ProcessId(3), "compiled\n"),
            AgentEventProcessOutputReset(ProcessId(4), "listening\n", 12L),
        )

        assertEquals(ProcessOutputBuffer(ProcessId(4), "listening\n", 12L), feed.processOutput)
    }
}
