@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/**
 * One event of a chat's feed, discriminated by `type`.
 *
 * The variants are top-level classes named after this type and their own `type`
 * value, spread over the `AgentEvent*` files: nested classes would not fit one
 * file under the size limit. Turn-level variants are in this file.
 */
@Serializable
@JsonClassDiscriminator("type")
sealed interface AgentEvent

@Serializable
@SerialName("userMessage")
data class AgentEventUserMessage(
    val turnId: TurnId,
    val text: String,
    /** Files attached with `@`, shown as chips above the text. */
    val attachments: List<String>,
) : AgentEvent

@Serializable
@SerialName("turnStarted")
data class AgentEventTurnStarted(
    val turnId: TurnId,
    val at: TsMillis,
) : AgentEvent

@Serializable
@SerialName("textDelta")
data class AgentEventTextDelta(
    val turnId: TurnId,
    val blockId: BlockId,
    val text: String,
) : AgentEvent

@Serializable
@SerialName("thinkingDelta")
data class AgentEventThinkingDelta(
    val turnId: TurnId,
    val blockId: BlockId,
    val text: String,
) : AgentEvent

/** [at] is zero in a journal written before the field existed: "unknown", not 1970. */
@Serializable
@SerialName("turnFinished")
data class AgentEventTurnFinished(
    val turnId: TurnId,
    val stopReason: StopReason,
    val at: TsMillis = TsMillis(0),
) : AgentEvent

/** Unfinished tools of the turn end as cancelled. */
@Serializable
@SerialName("turnCancelled")
data class AgentEventTurnCancelled(
    val turnId: TurnId,
    val at: TsMillis = TsMillis(0),
) : AgentEvent

/** Error block with the code and the "Retry" and "Switch model" actions. */
@Serializable
@SerialName("error")
data class AgentEventError(
    val code: String,
    val message: String,
    val retryable: Boolean,
) : AgentEvent

@Serializable
@SerialName("retryScheduled")
data class AgentEventRetryScheduled(
    val attempt: Int,
    val of: Int,
    val delayMs: Int,
    val reason: String,
) : AgentEvent

@Serializable
@SerialName("stateChanged")
data class AgentEventStateChanged(
    val state: SessionState,
) : AgentEvent

@Serializable
@SerialName("planProposed")
data class AgentEventPlanProposed(
    val plan: String,
) : AgentEvent

@Serializable
@SerialName("titleSet")
data class AgentEventTitleSet(
    val title: String,
) : AgentEvent
