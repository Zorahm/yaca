@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.protocol

import com.yaca.core.model.AgentEvent
import com.yaca.core.model.SessionId
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/**
 * What a host sends, discriminated by `type`: handshake answers, responses matched to
 * requests by [ServerMessageResponse.id], and the pushed messages. The pushed ones
 * arrive for every running chat, attached or not.
 */
@Serializable
@JsonClassDiscriminator("type")
sealed interface ServerMessage

/** The answer to a successful pair. [token] is shown this once: the host keeps only its hash. */
@Serializable
@SerialName("paired")
data class ServerMessagePaired(
    val deviceId: DeviceId,
    val token: String,
    val host: HostInfo,
) : ServerMessage

/** The answer to a successful hello. */
@Serializable
@SerialName("welcome")
data class ServerMessageWelcome(
    val host: HostInfo,
) : ServerMessage

/** The handshake failed. The socket closes after this. */
@Serializable
@SerialName("refused")
data class ServerMessageRefused(
    val reason: Refusal,
) : ServerMessage

/** The answer to the request with the same [id]. */
@Serializable
@SerialName("response")
data class ServerMessageResponse(
    val id: Long,
    val result: CallResult,
) : ServerMessage

/** Events of an attached chat, in order. Frames of an older [epoch] than the client's are dropped. */
@Serializable
@SerialName("frame")
data class ServerMessageFrame(
    val session: SessionId,
    val epoch: Int,
    val events: List<AgentEvent>,
) : ServerMessage

/** The connection fell behind and events were lost; the chat is no longer sent until attached again. */
@Serializable
@SerialName("resync")
data class ServerMessageResync(
    val session: SessionId,
) : ServerMessage

/** The set of running chats changed. */
@Serializable
@SerialName("chatsChanged")
data class ServerMessageChatsChanged(
    val chats: List<ChatSummary>,
) : ServerMessage

/** A running chat needs the human, or finished its turn. Sent for every running chat, which is what the phone notifies about. */
@Serializable
@SerialName("attention")
data class ServerMessageAttention(
    val session: SessionId,
    val project: String,
    val title: String? = null,
    val reason: Attention,
) : ServerMessage
