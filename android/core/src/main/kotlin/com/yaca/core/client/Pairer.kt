package com.yaca.core.client

import com.yaca.core.protocol.ClientMessagePair
import com.yaca.core.protocol.PROTOCOL_VERSION
import com.yaca.core.protocol.Refusal
import com.yaca.core.protocol.ServerMessagePaired
import com.yaca.core.protocol.ServerMessageRefused
import java.io.IOException
import okhttp3.OkHttpClient

/** The host refused the pairing: the code is wrong, spent or expired, or the protocol differs. */
class PairingRefusedException(val reason: Refusal) : Exception("the host refused the pairing")

/** Pairs this phone with a host, from the link its QR code carried. */
object Pairer {
    /**
     * Opens a socket to each of the link's hosts at once, keeps the first one that connects and drops
     * the rest, then sends the pairing code and returns the record to keep.
     *
     * A refusal throws [PairingRefusedException]; a network failure throws the transport's exception.
     * Neither message contains the code.
     */
    suspend fun pair(link: PairingLink, deviceName: String, http: OkHttpClient? = null): HostRecord {
        val client = http ?: pinnedHttpClient(link.fingerprint)
        val (_, socket) = firstSuccess(
            attempts = link.hosts.map { host ->
                suspend { host to HostSocket.open(client, hostUrl(host, link.port)) }
            },
            discard = { (_, loser) -> loser.cancel() },
        )
        val record = try {
            pairOn(socket, link, deviceName)
        } catch (e: Throwable) {
            socket.cancel()
            throw e
        }
        // The pairing is over; the connection that follows is a hello on a new socket.
        socket.close()
        return record
    }

    private suspend fun pairOn(socket: HostSocket, link: PairingLink, deviceName: String): HostRecord {
        socket.send(ClientMessagePair(link.code, deviceName, PROTOCOL_VERSION))
        return when (val answer = socket.handshakeAnswer()) {
            is ServerMessagePaired -> HostRecord(
                name = answer.host.name,
                addresses = link.hosts,
                port = link.port,
                fingerprint = link.fingerprint,
                deviceId = answer.deviceId.value,
                token = answer.token,
            )
            is ServerMessageRefused -> throw PairingRefusedException(answer.reason)
            else -> throw IOException("the host answered the pairing with an unexpected message")
        }
    }
}
