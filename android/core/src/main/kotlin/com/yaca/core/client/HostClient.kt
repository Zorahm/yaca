package com.yaca.core.client

import com.yaca.core.feed.Feed
import com.yaca.core.model.AgentEvent
import com.yaca.core.model.Command
import com.yaca.core.model.ModelInfo
import com.yaca.core.model.SessionId
import com.yaca.core.model.SessionMeta
import com.yaca.core.protocol.Attention
import com.yaca.core.protocol.ChatSummary
import com.yaca.core.protocol.ErrorCode
import com.yaca.core.protocol.HostInfo
import com.yaca.core.protocol.Refusal
import com.yaca.core.protocol.Startup
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable

/**
 * One paired host, as the phone keeps it. Everything needed to reach it again:
 * where to look, which certificate to trust, and the device's credentials.
 *
 * `token` is a secret: the app stores this record encrypted, never in plain
 * preferences or logs.
 */
@Serializable
data class HostRecord(
    val name: String,
    /** In the order the pairing link gave them; the client tries them all at once. */
    val addresses: List<String>,
    val port: Int,
    /** SHA-256 of the host's certificate, lowercase hex: the only certificate trusted. */
    val fingerprint: String,
    val deviceId: String,
    val token: String,
)

/** Where the connection to one host stands. The UI draws exactly this. */
sealed interface ConnectionState {
    /** Not asked to connect. */
    data object Idle : ConnectionState

    /** Trying the host's addresses. `attempt` counts from 1 since the last success. */
    data class Connecting(val attempt: Int) : ConnectionState

    data class Connected(val host: HostInfo, val address: String) : ConnectionState

    /**
     * The host said no. Retrying will not help until the human acts (pair again,
     * update the app), so the client stops here instead of looping.
     */
    data class Refused(val reason: Refusal) : ConnectionState

    /** The connection dropped or could not be made; the client retries on its own. */
    data class Lost(val message: String, val retryInMillis: Long) : ConnectionState
}

/** A running chat needs the human, or finished its turn. What the phone notifies about. */
data class AttentionEvent(
    val session: SessionId,
    val project: String,
    val title: String?,
    val reason: Attention,
)

/** A call the host answered with an error. `message` is English, shown as it is. */
class RemoteCallException(val code: ErrorCode, message: String) : Exception(message)

/** The connection is not up, so a call cannot be made right now. */
class NotConnectedException : Exception("not connected to the host")

/**
 * The phone's view of one host: the connection, the live chats, and a feed per chat.
 *
 * State the host owns is mirrored, never accumulated: the chat list is the last
 * `ChatsChanged`, a feed is the fold of the attach snapshot and the frames after
 * it. After a reconnect or a `Resync` the feed is rebuilt from a new snapshot.
 */
interface HostClient {
    val record: HostRecord
    val state: StateFlow<ConnectionState>

    /** The chats running on the host, as its last `ChatsChanged` listed them. */
    val chats: StateFlow<List<ChatSummary>>

    /** Every attention push, for every live chat, while connected. */
    val attention: SharedFlow<AttentionEvent>

    /** Starts connecting and keeps the connection up (reconnecting with backoff) until [disconnect]. */
    fun connect()

    fun disconnect()

    suspend fun projects(): List<String>

    suspend fun sessions(project: String): List<SessionMeta>

    suspend fun models(): List<ModelInfo>

    /** The feed of a recorded chat that is not running. */
    suspend fun recordedFeed(project: String, session: SessionId): List<AgentEvent>

    /** Opens a chat (a new one, or `resume`); a chat that is already live comes back as it is. */
    suspend fun open(project: String, resume: SessionId? = null, startup: Startup? = null): ChatSummary

    /** Closes the chat on the host, for everyone. */
    suspend fun close(session: SessionId)

    suspend fun send(session: SessionId, command: Command)

    /**
     * The chat's feed. The chat is attached while at least one collector is
     * active and detached a few seconds after the last one leaves, so turning the
     * screen off and on does not refetch the whole feed. Reattaches by itself after
     * a reconnect or a `Resync`.
     */
    fun feed(session: SessionId): StateFlow<Feed>
}
