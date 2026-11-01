package com.yaca.core.client

import com.yaca.core.model.SessionId
import com.yaca.core.protocol.ErrorCode
import com.yaca.core.protocol.RefusalUnauthorized
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.milliseconds
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first

class RemoteHostClientConnectionTest {
    @Test
    fun connectsGreetsAndFollowsTheChatListAndTheCalls() = clientTest { host, scope ->
        val server = host.allowConnection()
        val client = RemoteHostClient(host.record(), scope, initialBackoff = 10.milliseconds)
        assertFailsWith<NotConnectedException> { client.projects() }

        client.connect()
        val hello = server.receiveJson()
        assertEquals("hello", hello.str("type"))
        assertEquals("device-1", hello.str("deviceId"))
        assertEquals("token-1", hello.str("token"))
        server.send(Msg.welcome())

        val connected = assertIs<ConnectionState.Connected>(client.state.first { it is ConnectionState.Connected })
        assertEquals("localhost", connected.address)
        assertEquals("Desk", connected.host.name)

        server.send(Msg.chatsChanged("s1", "s2"))
        assertEquals(listOf("s1", "s2"), client.chats.first { it.isNotEmpty() }.map { it.id.value })

        val projects = async { client.projects() }
        val id = server.receiveRequest("listProjects")
        server.send(Msg.projects(id, "/a", "/b"))
        assertEquals(listOf("/a", "/b"), projects.await())

        val closing = async { runCatching { client.close(SessionId("gone")) }.exceptionOrNull() }
        val closeId = server.receiveRequest("closeChat")
        server.send(Msg.errorResponse(closeId, "chatNotOpen", "the chat is not running"))
        val error = assertIs<RemoteCallException>(closing.await())
        assertEquals(ErrorCode.CHAT_NOT_OPEN, error.code)
        assertEquals("the chat is not running", error.message)

        client.disconnect()
    }

    @Test
    fun aCallTheHostLeavesUnansweredFailsAfterItsTimeout() = clientTest { host, scope ->
        val server = host.allowConnection()
        val client = RemoteHostClient(host.record(), scope, callTimeout = 100.milliseconds, initialBackoff = 10.milliseconds)
        client.connect()
        server.receive()
        server.send(Msg.welcome())
        client.state.first { it is ConnectionState.Connected }

        val failure = assertIs<RemoteCallException>(runCatching { client.projects() }.exceptionOrNull())
        assertEquals(ErrorCode.HOST, failure.code)
        client.disconnect()
    }

    @Test
    fun aRefusedHelloStopsRetrying() = clientTest { host, scope ->
        val server = host.allowConnection()
        val client = RemoteHostClient(host.record(), scope, initialBackoff = 10.milliseconds)
        client.connect()
        server.receive()
        server.send(Msg.refused("unauthorized"))

        val refused = assertIs<ConnectionState.Refused>(client.state.first { it is ConnectionState.Refused })
        assertIs<RefusalUnauthorized>(refused.reason)
        // At a 10 ms backoff a retry would have reached the host long before this wait ends.
        delay(300.milliseconds)
        assertEquals(1, host.requestCount)
        assertIs<ConnectionState.Refused>(client.state.value)
    }

    @Test
    fun aDroppedConnectionIsLostAndReconnects() = clientTest { host, scope ->
        val first = host.allowConnection()
        // The wait is drawn from [400, 800] ms, so the Lost state stays visible to the collector.
        val client = RemoteHostClient(host.record(), scope, initialBackoff = 800.milliseconds)
        client.connect()
        first.receive()
        first.send(Msg.welcome())
        client.state.first { it is ConnectionState.Connected }

        val second = host.allowConnection()
        first.close()
        val lost = assertIs<ConnectionState.Lost>(client.state.first { it is ConnectionState.Lost })
        assertTrue(lost.retryInMillis in 400L..800L, "retry in ${lost.retryInMillis} ms")

        second.receive()
        second.send(Msg.welcome())
        client.state.first { it is ConnectionState.Connected }
        client.disconnect()
    }

    @Test
    fun triesEveryAddressAtOnceAndUsesTheOneThatAnswers() = clientTest { host, scope ->
        val server = host.allowConnection()
        val client = RemoteHostClient(
            host.record(listOf("192.0.2.1", "localhost")),
            scope,
            initialBackoff = 10.milliseconds,
        )
        client.connect()
        server.receive()
        server.send(Msg.welcome())

        val connected = assertIs<ConnectionState.Connected>(client.state.first { it is ConnectionState.Connected })
        assertEquals("localhost", connected.address)
        client.disconnect()
    }
}
