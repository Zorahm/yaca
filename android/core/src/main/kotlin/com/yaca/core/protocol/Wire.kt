package com.yaca.core.protocol

import com.yaca.core.model.AgentEvent
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull

/** The protocol number this client speaks. A host with another number refuses the handshake. */
const val PROTOCOL_VERSION: Int = 1

/**
 * The one JSON configuration of the client, shared by every type in this package and
 * `model`. [ignoreUnknownKeys]: a newer host may add fields. [explicitNulls] off: a
 * null is left out on the wire, and a missing nullable field decodes as null.
 * [encodeDefaults] on: the client writes what the host's serde writes, defaults included.
 */
val YacaJson: Json = Json {
    ignoreUnknownKeys = true
    explicitNulls = false
    encodeDefaults = true
}

/**
 * The `type` values of [AgentEvent] that this build knows. A sealed type's serializer
 * lists its subclasses as the elements of its "value" element, named by serial name.
 */
private val knownAgentEventTypes: Set<String> by lazy {
    val value = AgentEvent.serializer().descriptor.getElementDescriptor(1)
    (0 until value.elementsCount).map { value.getElementName(it) }.toSet()
}

/**
 * Decodes one feed event, or returns null when its `type` is not one this client knows.
 * A newer host adds event kinds; dropping only that event keeps the rest of its frame
 * readable. A known kind that fails to decode still throws: that is a broken host, not a
 * newer one.
 */
fun decodeAgentEventOrNull(element: JsonElement): AgentEvent? {
    val type = (element as? JsonObject)?.get("type")?.let { (it as? JsonPrimitive)?.contentOrNull } ?: return null
    if (type !in knownAgentEventTypes) return null
    return YacaJson.decodeFromJsonElement(AgentEvent.serializer(), element)
}
