package com.yaca.core.protocol

import com.yaca.core.model.AgentEvent
import com.yaca.core.model.Command
import java.io.File
import kotlin.test.Test
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.test.fail
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * Every line of the fixtures the Rust side writes must decode into its Kotlin type
 * and encode back to the same JSON, null-valued fields aside. The fixtures are read
 * from the source tree (see `build.gradle.kts`), so a stale copy cannot pass.
 */
class WireFixturesTest {
    private val fixtures: File = File(
        checkNotNull(System.getProperty("yaca.fixtures")) { "the Gradle test task sets yaca.fixtures" },
    )

    @Test
    fun agentEventsRoundTrip() = roundTrip("agent_events.jsonl", AgentEvent.serializer())

    @Test
    fun commandsRoundTrip() = roundTrip("commands.jsonl", Command.serializer())

    @Test
    fun clientMessagesRoundTrip() = roundTrip("client_messages.jsonl", ClientMessage.serializer())

    @Test
    fun serverMessagesRoundTrip() = roundTrip("server_messages.jsonl", ServerMessage.serializer())

    @Test
    fun everyAgentEventFixtureIsKnownToTheClient() {
        for (line in fixtureLines("agent_events.jsonl")) {
            val element = YacaJson.parseToJsonElement(line.text)
            assertNotNull(decodeAgentEventOrNull(element), "${location("agent_events.jsonl", line)} has a type this client does not know")
        }
    }

    @Test
    fun unknownAgentEventTypeIsSkippedNotThrown() {
        val future = YacaJson.parseToJsonElement("""{"type":"someFutureEvent","x":1}""")
        assertNull(decodeAgentEventOrNull(future))
    }

    private fun <T> roundTrip(name: String, serializer: KSerializer<T>) {
        for (line in fixtureLines(name)) {
            val where = location(name, line)
            val original = try {
                YacaJson.parseToJsonElement(line.text)
            } catch (e: SerializationException) {
                fail("$where is not JSON: ${e.message}")
            }
            val decoded = try {
                YacaJson.decodeFromJsonElement(serializer, original)
            } catch (e: SerializationException) {
                fail("$where does not decode: ${e.message}\n${line.text}")
            }
            val written = YacaJson.encodeToJsonElement(serializer, decoded)
            val before = original.withoutNulls()
            val after = written.withoutNulls()
            if (!sameJson(before, after)) {
                fail("$where encodes back to other JSON\nread:    $before\nwritten: $after")
            }
        }
    }

    /** The non-blank lines of a fixture file, numbered as the file numbers them. */
    private fun fixtureLines(name: String): List<FixtureLine> {
        val file = File(fixtures, name)
        assertTrue(file.isFile, "fixture file is missing: ${file.path}; the Rust test that writes it has not run")
        val lines = file.readLines().mapIndexedNotNull { index, text ->
            if (text.isBlank()) null else FixtureLine(index + 1, text)
        }
        assertTrue(lines.isNotEmpty(), "fixture file has no lines: ${file.path}")
        return lines
    }

    private fun location(name: String, line: FixtureLine): String = "${File(fixtures, name).path}:${line.number}"

    private class FixtureLine(val number: Int, val text: String)
}

/** Drops null-valued object fields, recursively: the client omits them on the wire, the Rust side writes them. */
private fun JsonElement.withoutNulls(): JsonElement = when (this) {
    is JsonObject -> JsonObject(filterValues { it !is JsonNull }.mapValues { (_, value) -> value.withoutNulls() })
    is JsonArray -> JsonArray(map { it.withoutNulls() })
    else -> this
}

private fun sameJson(a: JsonElement, b: JsonElement): Boolean = when {
    a is JsonObject && b is JsonObject ->
        a.keys == b.keys && a.keys.all { key -> sameJson(a.getValue(key), b.getValue(key)) }
    a is JsonArray && b is JsonArray ->
        a.size == b.size && a.indices.all { index -> sameJson(a[index], b[index]) }
    a is JsonPrimitive && b is JsonPrimitive -> samePrimitive(a, b)
    else -> a == b
}

/**
 * Numbers are compared as numbers: Rust and Kotlin print one value differently
 * (`1e-7` against `1.0E-7`), and that is not a difference in the wire.
 */
private fun samePrimitive(a: JsonPrimitive, b: JsonPrimitive): Boolean {
    if (a.isString || b.isString) return a == b
    val x = a.content.toBigDecimalOrNull() ?: return a == b
    val y = b.content.toBigDecimalOrNull() ?: return false
    return x.compareTo(y) == 0
}
