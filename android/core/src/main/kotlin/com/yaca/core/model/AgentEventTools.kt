@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

// Variants of AgentEvent about tool calls, subagents and hooks.

@Serializable
@SerialName("toolCallStarted")
data class AgentEventToolCallStarted(
    val turnId: TurnId,
    val callId: CallId,
    /** Tool name. Not shown in the feed, but needed for the icon and for debugging. */
    val tool: String,
    /** The label the model wrote; null for a tool the interface labels itself. */
    val description: String? = null,
    /** First line of the expanded row: the full path or the full command. */
    val target: String? = null,
    /** Call arguments; the permission request's detail is built from them. */
    val input: JsonElement,
    /** The `Task` row this call belongs under; null for the agent's own calls. */
    val parent: CallId? = null,
) : AgentEvent

@Serializable
@SerialName("toolCallProgress")
data class AgentEventToolCallProgress(
    val callId: CallId,
    val chunk: String,
) : AgentEvent

@Serializable
@SerialName("toolCallFinished")
data class AgentEventToolCallFinished(
    val callId: CallId,
    val meta: ToolMeta,
) : AgentEvent

@Serializable
@SerialName("toolCallFailed")
data class AgentEventToolCallFailed(
    val callId: CallId,
    val message: String,
) : AgentEvent

@Serializable
@SerialName("toolCallRejected")
data class AgentEventToolCallRejected(
    val callId: CallId,
    val reason: String,
) : AgentEvent

/** [stopped] is set when the human stopped a running call, rather than refusing it before it started. */
@Serializable
@SerialName("toolCallDenied")
data class AgentEventToolCallDenied(
    val callId: CallId,
    val reason: String? = null,
    val stopped: Boolean,
) : AgentEvent

@Serializable
@SerialName("subagentUsage")
data class AgentEventSubagentUsage(
    val callId: CallId,
    val tokens: TokenCounts,
) : AgentEvent

@Serializable
@SerialName("todosUpdated")
data class AgentEventTodosUpdated(
    val items: List<TodoItem>,
) : AgentEvent

@Serializable
@SerialName("hookReported")
data class AgentEventHookReported(
    val plugin: String,
    val event: String,
    val text: String,
    val blocked: Boolean,
) : AgentEvent
