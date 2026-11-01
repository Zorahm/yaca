package com.yaca.core.client

import com.yaca.core.feed.Feed
import com.yaca.core.feed.reduceAll
import com.yaca.core.model.AgentEvent
import com.yaca.core.model.Command
import com.yaca.core.model.ModelInfo
import com.yaca.core.model.SessionId
import com.yaca.core.model.SessionMeta
import com.yaca.core.protocol.Call
import com.yaca.core.protocol.CallAttach
import com.yaca.core.protocol.CallCloseChat
import com.yaca.core.protocol.CallDetach
import com.yaca.core.protocol.CallListModels
import com.yaca.core.protocol.CallListProjects
import com.yaca.core.protocol.CallListSessions
import com.yaca.core.protocol.CallOpenChat
import com.yaca.core.protocol.CallResultOk
import com.yaca.core.protocol.CallSend
import com.yaca.core.protocol.CallSessionFeed
import com.yaca.core.protocol.ChatSummary
import com.yaca.core.protocol.ClientMessageHello
import com.yaca.core.protocol.DeviceId
import com.yaca.core.protocol.ErrorCode
import com.yaca.core.protocol.PROTOCOL_VERSION
import com.yaca.core.protocol.Refusal
import com.yaca.core.protocol.Reply
import com.yaca.core.protocol.ReplyAttached
import com.yaca.core.protocol.ReplyDone
import com.yaca.core.protocol.ReplyFeed
import com.yaca.core.protocol.ReplyModels
import com.yaca.core.protocol.ReplyOpened
import com.yaca.core.protocol.ReplyProjects
import com.yaca.core.protocol.ReplySessions
import com.yaca.core.protocol.ServerMessage
import com.yaca.core.protocol.ServerMessageAttention
import com.yaca.core.protocol.ServerMessageChatsChanged
import com.yaca.core.protocol.ServerMessageFrame
import com.yaca.core.protocol.ServerMessageRefused
import com.yaca.core.protocol.ServerMessageResponse
import com.yaca.core.protocol.ServerMessageResync
import com.yaca.core.protocol.ServerMessageWelcome
import com.yaca.core.protocol.Startup
import java.util.concurrent.atomic.AtomicLong
import kotlin.random.Random
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.job
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

/**
 * The phone's [HostClient] for one paired host, over the WebSocket of `crates/yaca-remote`.
 *
 * Every message of a connection is read by one coroutine, in order. That is what keeps a feed
 * right: an attachment's snapshot is applied before the frames that follow it on the socket.
 * The bookkeeping fields are guarded by [lock]; the feeds are written only by the reader.
 */
class RemoteHostClient(
    override val record: HostRecord,
    private val scope: CoroutineScope,
    private val http: OkHttpClient = pinnedHttpClient(record.fingerprint),
    /** A call the host has not answered by then fails with [RemoteCallException] (code HOST). */
    private val callTimeout: Duration = 30.seconds,
    /** How long a feed stays attached after its last collector leaves. */
    private val detachDelay: Duration = 5.seconds,
    private val initialBackoff: Duration = 1.seconds,
    private val maxBackoff: Duration = 30.seconds,
    private val random: Random = Random.Default,
) : HostClient {
    private val lock = Any()
    private val nextId = AtomicLong(0)
    private val feeds = HashMap<SessionId, SessionFeed>() // guarded by lock
    private var loop: Job? = null // guarded by lock
    private var current: Connection? = null // guarded by lock

    private val connectionState = MutableStateFlow<ConnectionState>(ConnectionState.Idle)
    override val state: StateFlow<ConnectionState> = connectionState.asStateFlow()

    private val liveChats = MutableStateFlow<List<ChatSummary>>(emptyList())
    override val chats: StateFlow<List<ChatSummary>> = liveChats.asStateFlow()

    private val pushedAttention = MutableSharedFlow<AttentionEvent>(extraBufferCapacity = 64)
    override val attention: SharedFlow<AttentionEvent> = pushedAttention.asSharedFlow()

    override fun connect() {
        synchronized(lock) {
            if (loop?.isActive == true) return
            loop = scope.launch { keepConnected() }
        }
    }

    override fun disconnect() {
        val job = synchronized(lock) { loop.also { loop = null } }
        job?.cancel()
        connectionState.value = ConnectionState.Idle
    }

    override suspend fun projects(): List<String> = typed<ReplyProjects>(CallListProjects).projects

    override suspend fun sessions(project: String): List<SessionMeta> =
        typed<ReplySessions>(CallListSessions(project)).sessions

    override suspend fun models(): List<ModelInfo> = typed<ReplyModels>(CallListModels).models

    override suspend fun recordedFeed(project: String, session: SessionId): List<AgentEvent> =
        typed<ReplyFeed>(CallSessionFeed(project, session)).events

    override suspend fun open(project: String, resume: SessionId?, startup: Startup?): ChatSummary =
        typed<ReplyOpened>(CallOpenChat(project, resume, startup)).chat

    override suspend fun close(session: SessionId) {
        typed<ReplyDone>(CallCloseChat(session))
    }

    override suspend fun send(session: SessionId, command: Command) {
        typed<ReplyDone>(CallSend(session, command))
    }

    override fun feed(session: SessionId): StateFlow<Feed> = synchronized(lock) {
        feeds.getOrPut(session) {
            SessionFeed(session, scope, detachDelay) { sessionFeed, wanted -> onWanted(sessionFeed, wanted) }
        }.view
    }

    /** Keeps the connection up until the host refuses it or [disconnect] stops the loop. */
    private suspend fun keepConnected() {
        val owner = currentCoroutineContext().job
        val backoff = Backoff(initialBackoff.inWholeMilliseconds, maxBackoff.inWholeMilliseconds, random)
        while (true) {
            publish(owner, ConnectionState.Connecting(backoff.attempt))
            val end = try {
                connectOnce(owner, backoff)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                SessionEnd.Lost(e.message ?: "the connection failed")
            }
            when (end) {
                is SessionEnd.Refused -> {
                    // A refusal will not change by itself: the human has to pair again or update the app.
                    publish(owner, ConnectionState.Refused(end.reason))
                    return
                }
                is SessionEnd.Lost -> {
                    val wait = backoff.nextWaitMillis()
                    publish(owner, ConnectionState.Lost(end.message, wait))
                    delay(wait)
                }
            }
        }
    }

    private suspend fun connectOnce(owner: Job, backoff: Backoff): SessionEnd {
        if (record.addresses.isEmpty()) return SessionEnd.Lost("the host has no address to reach it at")
        val (address, socket) = firstSuccess(
            attempts = record.addresses.map { candidate ->
                suspend { candidate to HostSocket.open(http, hostUrl(candidate, record.port)) }
            },
            discard = { (_, loser) -> loser.cancel() },
        )
        val answer = try {
            socket.send(ClientMessageHello(DeviceId(record.deviceId), record.token, PROTOCOL_VERSION))
            socket.handshakeAnswer()
        } catch (e: Throwable) {
            socket.cancel()
            throw e
        }
        val welcome = when (answer) {
            is ServerMessageWelcome -> answer
            is ServerMessageRefused -> {
                socket.cancel()
                return SessionEnd.Refused(answer.reason)
            }
            else -> {
                socket.cancel()
                return SessionEnd.Lost("the host answered the hello with an unexpected message")
            }
        }

        backoff.reset()
        val connection = Connection(socket, callTimeout)
        val wanted = synchronized(lock) {
            current = connection
            feeds.values.filter { it.wanted }
        }
        publish(owner, ConnectionState.Connected(welcome.host, address))
        for (feed in wanted) {
            scope.launch { attachTo(connection, feed) }
        }
        val reason = try {
            connection.readUntilClosed { message -> dispatch(connection, message) }
        } finally {
            leave(connection)
        }
        return SessionEnd.Lost(reason)
    }

    /** Publishes a state only for the loop that is still current, so a disconnected loop cannot overwrite Idle. */
    private fun publish(owner: Job, value: ConnectionState) {
        synchronized(lock) {
            if (loop === owner) connectionState.value = value
        }
    }

    private fun leave(connection: Connection) {
        synchronized(lock) {
            if (current !== connection) return
            current = null
            for (feed in feeds.values) {
                feed.epoch = null
                feed.attachId = null
            }
        }
    }

    private fun dispatch(connection: Connection, message: ServerMessage) {
        when (message) {
            is ServerMessageResponse -> applyAttached(message)
            is ServerMessageFrame -> applyFrame(message)
            is ServerMessageResync -> resync(connection, message.session)
            is ServerMessageChatsChanged -> liveChats.value = message.chats
            is ServerMessageAttention -> pushedAttention.tryEmit(
                AttentionEvent(message.session, message.project, message.title, message.reason),
            )
            // The handshake answers arrive once, before this reader starts; nothing else is pushed.
            else -> Unit
        }
    }

    /**
     * Takes the snapshot of the latest attach. A reply to an older attach is stale: the host has
     * replaced that attachment, and its epoch will not come back.
     */
    private fun applyAttached(message: ServerMessageResponse) {
        val reply = (message.result as? CallResultOk)?.reply as? ReplyAttached ?: return
        synchronized(lock) {
            val feed = feeds[reply.session] ?: return
            if (feed.attachId != message.id) return
            feed.epoch = reply.epoch
            feed.state.value = reduceAll(Feed.EMPTY, reply.events)
        }
    }

    /**
     * Folds a frame of the current epoch. Frames of an epoch can only follow that epoch's reply on
     * this socket, and the reader handles messages in order, so the snapshot is always in place
     * first. A frame of any other epoch belongs to an attachment that was replaced or left.
     */
    private fun applyFrame(frame: ServerMessageFrame) {
        synchronized(lock) {
            val feed = feeds[frame.session] ?: return
            if (feed.epoch != frame.epoch) return
            feed.state.value = reduceAll(feed.state.value, frame.events)
        }
    }

    private fun resync(connection: Connection, session: SessionId) {
        val feed = synchronized(lock) {
            feeds[session]?.takeIf { it.wanted }?.also {
                it.epoch = null
                it.attachId = null
            }
        } ?: return
        scope.launch { attachTo(connection, feed) }
    }

    private fun onWanted(feed: SessionFeed, wanted: Boolean) {
        val connection = synchronized(lock) {
            feed.wanted = wanted
            if (!wanted) {
                feed.epoch = null
                feed.attachId = null
            }
            current
        } ?: return
        scope.launch {
            if (wanted) attachTo(connection, feed) else detachFrom(connection, feed)
        }
    }

    /**
     * Attaches [feed] on [connection]. The request is sent under the lock, and so is the check that
     * the feed is still wanted, so an attach and a detach reach the host in the order they were decided.
     */
    private suspend fun attachTo(connection: Connection, feed: SessionFeed) {
        val id = nextId.incrementAndGet()
        val answer = synchronized(lock) {
            if (!feed.wanted || current !== connection) return
            val answer = connection.send(id, CallAttach(feed.session)) ?: return
            // Set under the lock the reader takes, so the reply cannot arrive before the id is known.
            feed.attachId = id
            feed.epoch = null
            answer
        }
        try {
            connection.await(id, answer)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            // The feed keeps what it had. The next connect or Resync asks again.
            synchronized(lock) {
                if (feed.attachId == id) feed.attachId = null
            }
        }
    }

    private suspend fun detachFrom(connection: Connection, feed: SessionFeed) {
        val id = nextId.incrementAndGet()
        val answer = synchronized(lock) {
            // Checked again: the feed may have been wanted again while this coroutine waited to run.
            if (feed.wanted || current !== connection) return
            connection.send(id, CallDetach(feed.session)) ?: return
        }
        try {
            connection.await(id, answer)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            // The connection went with the attachment, so there is nothing left to detach.
        }
    }

    private suspend fun request(call: Call): Reply {
        val connection = synchronized(lock) { current } ?: throw NotConnectedException()
        return connection.request(nextId.incrementAndGet(), call)
    }

    private suspend inline fun <reified T : Reply> typed(call: Call): T =
        request(call) as? T
            ?: throw RemoteCallException(ErrorCode.HOST, "the host answered with a reply this call does not expect")

    /** How one connection ended: the host refused it (stop), or it was lost (retry). */
    private sealed interface SessionEnd {
        class Refused(val reason: Refusal) : SessionEnd

        class Lost(val message: String) : SessionEnd
    }
}
