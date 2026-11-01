package com.yaca.core.client

import com.yaca.core.protocol.YacaJson
import java.security.MessageDigest
import kotlin.test.assertEquals
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.tls.HandshakeCertificates
import okhttp3.tls.HeldCertificate

/**
 * A host for the tests: a TLS server with a self-signed certificate, as `crates/yaca-remote` runs.
 * The test scripts its side of each connection through the [ServerSide] that [allowConnection] returns.
 */
internal class FakeHost : AutoCloseable {
    private val certificate: HeldCertificate = HeldCertificate.Builder()
        .commonName("yaca.local")
        .addSubjectAlternativeName("localhost")
        .build()

    /** The certificate's SHA-256, the way the pairing link carries it. */
    val fingerprint: String = sha256Hex(certificate.certificate.encoded)

    private val server = MockWebServer()

    init {
        val handshake = HandshakeCertificates.Builder().heldCertificate(certificate).build()
        server.useHttps(handshake.sslSocketFactory(), false)
        server.start()
    }

    val port: Int get() = server.port

    /** Requests that reached the host, upgraded or not. */
    val requestCount: Int get() = server.requestCount

    /** The record a phone would keep after pairing with this host. */
    fun record(addresses: List<String> = listOf("localhost")): HostRecord = HostRecord(
        name = "Desk",
        addresses = addresses,
        port = port,
        fingerprint = fingerprint,
        deviceId = "device-1",
        token = "token-1",
    )

    /** Lets the next connection in as a WebSocket. Its [ServerSide] exists before the client connects. */
    fun allowConnection(): ServerSide {
        val side = ServerSide()
        server.enqueue(MockResponse().withWebSocketUpgrade(side))
        return side
    }

    override fun close() {
        server.shutdown()
    }
}

/** The host's end of one accepted connection: what the client sent, and what the test sends back. */
internal class ServerSide : WebSocketListener() {
    private val incoming = Channel<String>(Channel.UNLIMITED)

    @Volatile
    private var socket: WebSocket? = null

    override fun onOpen(webSocket: WebSocket, response: Response) {
        socket = webSocket
    }

    override fun onMessage(webSocket: WebSocket, text: String) {
        incoming.trySend(text)
    }

    /** Answers the client's close frame, so the closing handshake completes and the server can shut down. */
    override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
        webSocket.close(1000, null)
    }

    /** The next text the client sent, within ten seconds. */
    suspend fun receive(): String = withTimeout(10.seconds) { incoming.receive() }

    suspend fun receiveJson(): JsonObject = YacaJson.parseToJsonElement(receive()).jsonObject

    /** The id of the next request, after checking that it is a call of [kind]. */
    suspend fun receiveRequest(kind: String): Long {
        val json = receiveJson()
        assertEquals("request", json.str("type"))
        assertEquals(kind, json.getValue("call").jsonObject.str("kind"))
        return json.getValue("id").jsonPrimitive.long
    }

    fun send(text: String) {
        checkNotNull(socket).send(text)
    }

    /**
     * Closes the connection from the host's side. OkHttp does not let a server-side socket be
     * cancelled, so a dropped connection is simulated by a close the client did not ask for.
     */
    fun close() {
        checkNotNull(socket).close(1001, "test over")
    }
}

internal fun JsonObject.str(key: String): String = getValue(key).jsonPrimitive.content

internal fun sha256Hex(der: ByteArray): String =
    MessageDigest.getInstance("SHA-256").digest(der).joinToString("") { "%02x".format(it) }

/** Runs a client test against a fresh [FakeHost] and a scope for the client, both torn down afterwards. */
internal fun clientTest(block: suspend CoroutineScope.(FakeHost, CoroutineScope) -> Unit) = runBlocking {
    FakeHost().use { host ->
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            withTimeout(20.seconds) { block(host, scope) }
        } finally {
            scope.cancel()
        }
    }
}
