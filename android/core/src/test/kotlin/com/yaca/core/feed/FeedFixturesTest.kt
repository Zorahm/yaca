package com.yaca.core.feed

import com.yaca.core.model.AgentEvent
import com.yaca.core.protocol.YacaJson
import java.io.File
import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * The fixtures the Rust side writes contain every variant of [AgentEvent]: folding them through the
 * reducer is the cheapest check that no variant breaks it. Once as one batch, and once with a batch
 * per event, which is how the interface receives them over a live connection.
 */
class FeedFixturesTest {
    private val fixture: File = File(
        checkNotNull(System.getProperty("yaca.fixtures")) { "the Gradle test task sets yaca.fixtures" },
        "agent_events.jsonl",
    )

    private fun events(): List<AgentEvent> {
        assertTrue(fixture.isFile, "fixture file is missing: ${fixture.path}")
        return fixture.readLines()
            .filter { it.isNotBlank() }
            .map { YacaJson.decodeFromString(AgentEvent.serializer(), it) }
    }

    @Test
    fun `every line of agent_events jsonl folds through reduceAll without throwing`() {
        val events = events()
        assertTrue(events.isNotEmpty(), "fixture file has no lines: ${fixture.path}")

        val feed = reduceAll(Feed.EMPTY, events)

        assertTrue(feed.blocks.isNotEmpty())
        assertTrue(fold(visible(feed)).isNotEmpty())
    }

    @Test
    fun `every line of agent_events jsonl folds one batch at a time without throwing`() {
        val feed = events().fold(Feed.EMPTY) { acc, event -> reduceAll(acc, listOf(event)) }

        assertTrue(feed.blocks.isNotEmpty())
    }
}
