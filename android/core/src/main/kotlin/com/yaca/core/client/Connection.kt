package com.yaca.core.client

import com.yaca.core.protocol.Call
import com.yaca.core.protocol.CallResult
import com.yaca.core.protocol.CallResultError
import com.yaca.core.protocol.CallResultOk
import com.yaca.core.protocol.ClientMessageRequest
import com.yaca.core.protocol.ErrorCode
import com.yaca.core.protocol.Reply
import com.yaca.core.protocol.ServerMessage
import com.yaca.core.protocol.ServerMessageResponse
import kotlin.time.Duration
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.withTimeoutOrNull

private const val UNREADABLE_REASON = "the host sent a message this client cannot read"

/**
 * One open socket to the host and the calls waiting on it. It lives from the welcome until the
 * socket ends; a call on a dead connection fails with [NotConnectedException].
 */
internal class Connection(
    private val socket: HostSocket,
    private val callTimeout: Duration,
) {
    private val pending = PendingCalls()

    /**
     * Sends [call] under [id] and returns the answer to wait for with [await]. It does not suspend,
     * so a caller can order it against other sends by holding its own lock. Null when the socket is gone.
     */
    fun send(id: Long, call: Call): CompletableDeferred<CallResult>? {
        val answer = pending.register(id) ?: return null
        if (socket.send(ClientMessageRequest(id, call))) return answer
        pending.forget(id)
        return null
    }

    /** Waits for the answer to [id]. A host that does not answer in time is an error, not a hang. */
    suspend fun await(id: Long, answer: CompletableDeferred<CallResult>): Reply {
        try {
            val result = withTimeoutOrNull(callTimeout) { answer.await() }
                ?: throw RemoteCallException(ErrorCode.HOST, "the host did not answer")
            return when (result) {
                is CallResultOk -> result.reply
                is CallResultError -> throw RemoteCallException(result.error.code, result.error.message)
            }
        } finally {
            pending.forget(id)
        }
    }

    suspend fun request(id: Long, call: Call): Reply {
        val answer = send(id, call) ?: throw NotConnectedException()
        return await(id, answer)
    }

    /**
     * Reads the socket in order until it ends, handing each message to [onMessage]. A response is
     * handed over before its call is completed, so a state change it carries (an attachment's
     * snapshot) is applied before the caller goes on. Returns why the connection ended.
     */
    suspend fun readUntilClosed(onMessage: (ServerMessage) -> Unit): String {
        try {
            while (true) {
                when (val event = socket.receive()) {
                    is SocketEvent.Text -> {
                        val message = try {
                            Incoming.decode(event.text)
                        } catch (e: Exception) {
                            return UNREADABLE_REASON
                        }
                        // Null is a message type this client does not know: skipped, as the decoder says.
                        if (message is ServerMessageResponse) {
                            onMessage(message)
                            pending.complete(message.id, message.result)
                        } else if (message != null) {
                            onMessage(message)
                        }
                    }
                    is SocketEvent.Closed -> return "the host closed the connection"
                    is SocketEvent.Failed -> return "the connection was lost"
                    SocketEvent.Opened -> Unit
                }
            }
        } finally {
            pending.close()
            socket.cancel()
        }
    }
}
