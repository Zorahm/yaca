package com.yaca.core.client

import com.yaca.core.feed.Feed
import com.yaca.core.feed.FeedBlock
import com.yaca.core.model.SessionId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/*
 * The feed's blocks carry keys from a process-wide sequence, so two feeds reduced from the same
 * events are never equal. The tests therefore compare what the feed says: the agent text.
 */
class RemoteHostClientFeedTest {
    private val session = SessionId("s1")

    private fun Feed.agentText(): String = blocks.filterIsInstance<FeedBlock.Agent>().joinToString("") { it.text }

    /** A client connected to [host] through a welcome the test has sent, and the host's end of the connection. */
    private suspend fun connectedClient(
        host: FakeHost,
        scope: CoroutineScope,
        detachDelay: Duration = 50.milliseconds,
    ): Pair<RemoteHostClient, ServerSide> {
        val server = host.allowConnection()
        val client = RemoteHostClient(host.record(), scope, detachDelay = detachDelay, initialBackoff = 10.milliseconds)
        client.connect()
        server.receive()
        server.send(Msg.welcome())
        client.state.first { it is ConnectionState.Connected }
        return client to server
    }

    @Test
    fun attachesWhileCollectedAndFoldsFramesOfTheCurrentEpochOnly() = clientTest { host, scope ->
        val (client, server) = connectedClient(host, scope)
        val feed = client.feed(session)
        val collecting = launch { feed.collect { } }

        val attach = server.receiveRequest("attach")
        server.send(Msg.attached(attach, "s1", 1, listOf(Msg.textDelta("Hi"))))
        assertEquals("Hi", feed.first { it.agentText() == "Hi" }.agentText())

        server.send(Msg.frame("s1", 1, listOf(Msg.textDelta(" there"))))
        assertEquals("Hi there", feed.first { it.agentText() == "Hi there" }.agentText())

        // A frame of an epoch that is not current is dropped; were it folded, the text would not match.
        server.send(Msg.frame("s1", 0, listOf(Msg.textDelta("STALE"))))
        server.send(Msg.frame("s1", 1, listOf(Msg.textDelta("!"))))
        assertEquals("Hi there!", feed.first { it.agentText() == "Hi there!" }.agentText())

        collecting.cancel()
        client.disconnect()
    }

    @Test
    fun aResyncAttachesAgainAndRebuildsTheFeed() = clientTest { host, scope ->
        val (client, server) = connectedClient(host, scope)
        val feed = client.feed(session)
        val collecting = launch { feed.collect { } }

        val firstAttach = server.receiveRequest("attach")
        server.send(Msg.attached(firstAttach, "s1", 1, listOf(Msg.textDelta("old"))))
        assertEquals("old", feed.first { it.agentText() == "old" }.agentText())
        server.send(Msg.resync("s1"))

        val secondAttach = server.receiveRequest("attach")
        assertNotEquals(firstAttach, secondAttach)
        server.send(Msg.attached(secondAttach, "s1", 2, listOf(Msg.textDelta("fresh"))))
        assertEquals("fresh", feed.first { it.agentText() == "fresh" }.agentText())

        // The replaced epoch's frame is dropped; a frame of the new epoch is folded.
        server.send(Msg.frame("s1", 1, listOf(Msg.textDelta("late"))))
        server.send(Msg.frame("s1", 2, listOf(Msg.textDelta(" new"))))
        assertEquals("fresh new", feed.first { it.agentText() == "fresh new" }.agentText())

        collecting.cancel()
        client.disconnect()
    }

    @Test
    fun detachesAfterTheLastCollectorLeaves() = clientTest { host, scope ->
        val (client, server) = connectedClient(host, scope, detachDelay = 50.milliseconds)
        val collecting = launch { client.feed(session).collect { } }

        val attach = server.receiveRequest("attach")
        server.send(Msg.attached(attach, "s1", 1, listOf(Msg.turnStarted)))
        collecting.cancel()

        server.receiveRequest("detach")
        client.disconnect()
    }

    @Test
    fun aReconnectAttachesAgainTheFeedsThatAreStillCollected() = clientTest { host, scope ->
        val (client, first) = connectedClient(host, scope)
        val feed = client.feed(session)
        val collecting = launch { feed.collect { } }
        first.send(Msg.attached(first.receiveRequest("attach"), "s1", 1, listOf(Msg.textDelta("one"))))
        feed.first { it.agentText() == "one" }

        val second = host.allowConnection()
        first.close()
        second.receive()
        second.send(Msg.welcome())

        val reattach = second.receiveRequest("attach")
        second.send(Msg.attached(reattach, "s1", 1, listOf(Msg.textDelta("again"))))
        assertEquals("again", feed.first { it.agentText() == "again" }.agentText())

        collecting.cancel()
        client.disconnect()
    }
}
