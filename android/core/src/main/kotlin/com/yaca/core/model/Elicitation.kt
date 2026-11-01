package com.yaca.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

/** What kind of input a form field takes. */
@Serializable
enum class ElicitKind {
    @SerialName("text") TEXT,
    @SerialName("number") NUMBER,
    @SerialName("integer") INTEGER,
    @SerialName("boolean") BOOLEAN,
    @SerialName("choice") CHOICE,
}

/** One field of an MCP server's form. */
@Serializable
data class ElicitField(
    /** The key the server reads the value under. */
    val name: String,
    /** The label the form shows: the schema's `title`, or the name. */
    val title: String,
    val description: String? = null,
    val kind: ElicitKind,
    val required: Boolean,
    /** The values a choice takes, as sent back. */
    val options: List<String>,
    /** How each option is shown; the same length as [options]. */
    val optionLabels: List<String>,
    /** What the field starts with. Rust writes an explicit null when there is none. */
    val default: JsonElement? = null,
)

/** A form on screen, asked by an MCP server. */
@Serializable
data class ElicitationRequest(
    val id: RequestId,
    /** The server that asks, as the session names it. */
    val server: String,
    /** What the server says it needs, in its own words. */
    val message: String,
    val fields: List<ElicitField>,
)
