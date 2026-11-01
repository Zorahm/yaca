@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.protocol

import com.yaca.core.model.AgentEvent
import com.yaca.core.model.ModelInfo
import com.yaca.core.model.SessionId
import com.yaca.core.model.SessionMeta
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/** The successful answer to a [Call], discriminated by `kind`. */
@Serializable
@JsonClassDiscriminator("kind")
sealed interface Reply

@Serializable
@SerialName("projects")
data class ReplyProjects(
    val projects: List<String>,
) : Reply

@Serializable
@SerialName("sessions")
data class ReplySessions(
    val sessions: List<SessionMeta>,
) : Reply

@Serializable
@SerialName("models")
data class ReplyModels(
    val models: List<ModelInfo>,
) : Reply

@Serializable
@SerialName("chats")
data class ReplyChats(
    val chats: List<ChatSummary>,
) : Reply

@Serializable
@SerialName("feed")
data class ReplyFeed(
    val events: List<AgentEvent>,
) : Reply

/**
 * The chat that was opened. The host boxes it because a summary is several times the
 * size of every other reply; the client has no such concern.
 */
@Serializable
@SerialName("opened")
data class ReplyOpened(
    val chat: ChatSummary,
) : Reply

/** The whole feed so far of an attached chat; frames of [epoch] follow. */
@Serializable
@SerialName("attached")
data class ReplyAttached(
    val session: SessionId,
    val epoch: Int,
    val events: List<AgentEvent>,
) : Reply

@Serializable
@SerialName("done")
data object ReplyDone : Reply
