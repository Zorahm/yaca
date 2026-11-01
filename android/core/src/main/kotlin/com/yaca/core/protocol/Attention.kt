@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.protocol

import com.yaca.core.model.RequestId
import com.yaca.core.model.StopReason
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/** Why a chat wants the human, discriminated by `kind`. */
@Serializable
@JsonClassDiscriminator("kind")
sealed interface Attention

/**
 * The agent asks for permission; [target] is the request's short target, and
 * [request] lets a notification answer it without opening the chat.
 */
@Serializable
@SerialName("permission")
data class AttentionPermission(
    val request: RequestId,
    val target: String,
) : Attention

/** The agent asked a question. */
@Serializable
@SerialName("question")
data class AttentionQuestion(
    val text: String,
) : Attention

/** An MCP server asks the human to fill in a form. */
@Serializable
@SerialName("elicitation")
data object AttentionElicitation : Attention

/** The turn ended, not cancelled: the agent is done and waits. */
@Serializable
@SerialName("turnFinished")
data class AttentionTurnFinished(
    val stopReason: StopReason,
) : Attention
