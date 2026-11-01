package com.yaca.core.client

import com.yaca.core.model.AgentEvent
import com.yaca.core.model.SessionId
import com.yaca.core.protocol.CallError
import com.yaca.core.protocol.CallResult
import com.yaca.core.protocol.CallResultError
import com.yaca.core.protocol.CallResultOk
import com.yaca.core.protocol.Reply
import com.yaca.core.protocol.ReplyAttached
import com.yaca.core.protocol.ReplyFeed
import com.yaca.core.protocol.ServerMessage
import com.yaca.core.protocol.ServerMessageFrame
import com.yaca.core.protocol.ServerMessageResponse
import com.yaca.core.protocol.YacaJson
import com.yaca.core.protocol.decodeAgentEventOrNull
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long

/**
 * Reads one text message of the socket. Frames and attach or feed replies are decoded event by
 * event, so an event of a kind this client does not know is skipped instead of failing the whole
 * message; the rest of the message is still read. A message of a type this client does not know is
 * skipped as a whole. A known message that cannot be read is an error: the stream has a hole, and
 * the caller drops the connection rather than fold it.
 */
internal object Incoming {
    private val knownTypes = setOf(
        "paired", "welcome", "refused", "response", "frame", "resync", "chatsChanged", "attention",
    )

    /** The message, or null for a type this client does not know. Throws for a known one it cannot read. */
    fun decode(text: String): ServerMessage? {
        val root = YacaJson.parseToJsonElement(text).jsonObject
        val type = root["type"]?.jsonPrimitive?.contentOrNull ?: return null
        if (type !in knownTypes) return null
        return when (type) {
            "frame" -> ServerMessageFrame(
                session = SessionId(root.text("session")),
                epoch = root.number("epoch"),
                events = events(root),
            )
            "response" -> response(root)
            else -> YacaJson.decodeFromJsonElement(ServerMessage.serializer(), root)
        }
    }

    private fun response(root: JsonObject): ServerMessageResponse {
        val result = root.getValue("result").jsonObject
        val outcome: CallResult = when (result.text("status")) {
            "ok" -> CallResultOk(reply(result.getValue("reply").jsonObject))
            "error" -> CallResultError(YacaJson.decodeFromJsonElement(CallError.serializer(), result.getValue("error")))
            else -> throw SerializationException("a call result has an unknown status")
        }
        return ServerMessageResponse(root.getValue("id").jsonPrimitive.long, outcome)
    }

    private fun reply(reply: JsonObject): Reply = when (reply.text("kind")) {
        "attached" -> ReplyAttached(SessionId(reply.text("session")), reply.number("epoch"), events(reply))
        "feed" -> ReplyFeed(events(reply))
        else -> YacaJson.decodeFromJsonElement(Reply.serializer(), reply)
    }

    private fun events(message: JsonObject): List<AgentEvent> =
        message.getValue("events").jsonArray.mapNotNull { decodeAgentEventOrNull(it) }

    private fun JsonObject.text(key: String): String = getValue(key).jsonPrimitive.content

    private fun JsonObject.number(key: String): Int = getValue(key).jsonPrimitive.int
}
