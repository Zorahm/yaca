package com.yaca.core.client

import com.yaca.core.protocol.ClientMessage
import com.yaca.core.protocol.ServerMessage
import com.yaca.core.protocol.YacaJson
import java.io.IOException
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.withTimeoutOrNull
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener

/** How long a host has to answer the first message of a connection. */
internal val HANDSHAKE_TIMEOUT = 10.seconds

private const val NORMAL_CLOSURE = 1000

/** What a socket's listener pushes. The reader coroutine is the only consumer. */
internal sealed interface SocketEvent {
    data object Opened : SocketEvent

    class Text(val text: String) : SocketEvent

    class Closed(val code: Int) : SocketEvent

    class Failed(val cause: Throwable) : SocketEvent
}

/**
 * OkHttp's callbacks, kept thin: each one only queues an event. Everything that acts on a message
 * happens in the reader coroutine, so a message is never handled on OkHttp's thread.
 */
internal class SocketListener(private val events: Channel<SocketEvent>) : WebSocketListener() {
    override fun onOpen(webSocket: WebSocket, response: Response) {
        events.trySend(SocketEvent.Opened)
    }

    override fun onMessage(webSocket: WebSocket, text: String) {
        events.trySend(SocketEvent.Text(text))
    }

    override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
        events.trySend(SocketEvent.Closed(code))
        // Answering the close frame lets OkHttp finish the closing handshake. The code is not echoed:
        // the peer may have sent one (1005) that a close frame cannot carry.
        webSocket.close(NORMAL_CLOSURE, null)
    }

    override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
        events.trySend(SocketEvent.Closed(code))
    }

    override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
        events.trySend(SocketEvent.Failed(t))
    }
}

/** One WebSocket to a host. Built by [open], which returns once the socket is up. */
internal class HostSocket private constructor(
    private val webSocket: WebSocket,
    private val events: Channel<SocketEvent>,
) {
    /** The next event. A listener always ends with [SocketEvent.Closed] or [SocketEvent.Failed]. */
    suspend fun receive(): SocketEvent = events.receive()

    /** The answer to the hello or the pair, which must arrive within [HANDSHAKE_TIMEOUT]. */
    suspend fun handshakeAnswer(): ServerMessage {
        val event = withTimeoutOrNull(HANDSHAKE_TIMEOUT) { events.receive() }
            ?: throw IOException("the host did not answer the handshake")
        return when (event) {
            is SocketEvent.Text -> Incoming.decode(event.text)
                ?: throw IOException("the host answered the handshake with a message this client does not know")
            is SocketEvent.Failed -> throw event.cause
            is SocketEvent.Closed -> throw IOException("the host closed the connection")
            SocketEvent.Opened -> throw IOException("the socket opened twice")
        }
    }

    /** False when the socket is already closing; the caller treats that as a lost connection. */
    fun send(message: ClientMessage): Boolean =
        webSocket.send(YacaJson.encodeToString(ClientMessage.serializer(), message))

    /** A clean close, for a socket that has done its job. */
    fun close() {
        webSocket.close(NORMAL_CLOSURE, null)
    }

    /** Drops the socket at once: a loser of the connection race, or one that has failed. */
    fun cancel() {
        webSocket.cancel()
    }

    companion object {
        /**
         * Opens a socket to [url] and returns once the upgrade is done. A failure (TLS, refused, a
         * certificate that is not the pinned one) is thrown. Cancelling the caller cancels the socket.
         */
        suspend fun open(http: OkHttpClient, url: String): HostSocket {
            val events = Channel<SocketEvent>(Channel.UNLIMITED)
            val webSocket = http.newWebSocket(Request.Builder().url(url).build(), SocketListener(events))
            val first = try {
                events.receive()
            } catch (e: Throwable) {
                webSocket.cancel()
                throw e
            }
            return when (first) {
                is SocketEvent.Opened -> HostSocket(webSocket, events)
                is SocketEvent.Failed -> {
                    webSocket.cancel()
                    throw first.cause
                }
                else -> {
                    webSocket.cancel()
                    throw IOException("the socket did not open")
                }
            }
        }
    }
}

/** The WebSocket URL of one address: `wss://<host>:<port>/`, with IPv6 in brackets. */
internal fun hostUrl(address: String, port: Int): String {
    val host = if (':' in address && !address.startsWith("[")) "[$address]" else address
    return "wss://$host:$port/"
}
