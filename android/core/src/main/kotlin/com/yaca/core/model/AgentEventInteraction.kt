@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Variants of AgentEvent that a chat raises when it needs the human, and the
// queue of messages the human sent during a turn.

@Serializable
@SerialName("permissionRequested")
data class AgentEventPermissionRequested(
    val request: PermissionRequest,
) : AgentEvent

@Serializable
@SerialName("permissionResolved")
data class AgentEventPermissionResolved(
    val requestId: RequestId,
) : AgentEvent

@Serializable
@SerialName("questionAsked")
data class AgentEventQuestionAsked(
    val request: QuestionRequest,
) : AgentEvent

/** [reply] is null when the turn stopped before anyone answered. */
@Serializable
@SerialName("questionResolved")
data class AgentEventQuestionResolved(
    val requestId: RequestId,
    val reply: QuestionReply? = null,
) : AgentEvent

@Serializable
@SerialName("elicitationRequested")
data class AgentEventElicitationRequested(
    val request: ElicitationRequest,
) : AgentEvent

/** [reply] is null when the server stopped waiting first. */
@Serializable
@SerialName("elicitationResolved")
data class AgentEventElicitationResolved(
    val requestId: RequestId,
    val reply: ElicitationReply? = null,
) : AgentEvent

@Serializable
@SerialName("queueUpdated")
data class AgentEventQueueUpdated(
    val queued: List<QueuedMessage>,
) : AgentEvent
