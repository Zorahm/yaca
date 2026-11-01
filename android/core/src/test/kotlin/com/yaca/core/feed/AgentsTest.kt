package com.yaca.core.feed

import com.yaca.core.model.AgentEvent
import com.yaca.core.model.AgentEventSubagentUsage
import com.yaca.core.model.AgentEventToolCallFailed
import com.yaca.core.model.AgentEventToolCallFinished
import com.yaca.core.model.AgentEventToolCallStarted
import com.yaca.core.model.CallId
import com.yaca.core.model.TokenCounts
import com.yaca.core.model.ToolMeta
import com.yaca.core.model.TurnId
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

/** Ports of `src/core/agents.test.ts`. */
class AgentsTest {
    private fun sent(callId: String, prompt: JsonElement = JsonPrimitive("look around")): AgentEvent =
        AgentEventToolCallStarted(
            turnId = TurnId("t1"),
            callId = CallId(callId),
            tool = "Task",
            description = null,
            target = null,
            input = promptOf(prompt),
            parent = null,
        )

    private fun read(callId: String, parent: String?): AgentEvent = AgentEventToolCallStarted(
        turnId = TurnId("t1"),
        callId = CallId(callId),
        tool = "Read",
        description = null,
        target = "src/a.rs",
        input = JsonObject(emptyMap()),
        parent = parent?.let { CallId(it) },
    )

    private fun spent(callId: String, input: Int, output: Int): AgentEvent =
        AgentEventSubagentUsage(CallId(callId), TokenCounts(input = input, output = output, cacheWrite = 0, cacheRead = 0))

    private fun finished(callId: String): AgentEvent =
        AgentEventToolCallFinished(CallId(callId), ToolMeta(lines = 1, items = null, truncated = false, durationMs = 5))

    @Test
    fun `lists the Task rows in the order they were sent, and nothing else`() {
        val feed = runFeed(read("c1", null), sent("a1"), read("a1c1", "a1"), sent("a2"))
        assertEquals(listOf("a1", "a2"), agentsOf(feed).map { it.callId.value })
    }

    @Test
    fun `agentPrompt is the prompt the call carried`() {
        val row = agentsOf(runFeed(sent("a1", JsonPrimitive("  find the dates\n"))))[0]
        assertEquals("find the dates", agentPrompt(row))
    }

    @Test
    fun `agentPrompt is empty when the call carried none`() {
        val row = agentsOf(runFeed(sent("a1", JsonPrimitive(42))))[0]
        assertEquals("", agentPrompt(row))
    }

    @Test
    fun `tokens shows the newest figure, which is already the total`() {
        val feed = runFeed(sent("a1"), spent("a1", 100, 30), spent("a1", 200, 50))
        assertEquals(250L, agentTokens(agentsOf(feed)[0]))
    }

    @Test
    fun `tokens is unknown, not zero, until a figure arrives`() {
        val feed = runFeed(sent("a1"))
        assertNull(agentTokens(agentsOf(feed)[0]))
        assertNull(tally(agentsOf(feed)).tokens)
    }

    @Test
    fun `tally counts who is at work, who ended badly and what all of them spent`() {
        val feed = runFeed(
            sent("a1"),
            sent("a2"),
            sent("a3"),
            spent("a1", 100, 30),
            spent("a2", 400, 20),
            finished("a1"),
            AgentEventToolCallFailed(CallId("a3"), "no report"),
        )
        assertEquals(AgentsTally(count = 3, running = 1, failed = 1, tokens = 550L), tally(agentsOf(feed)))
    }
}
