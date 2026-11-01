package com.yaca.android.attention

import com.yaca.core.client.AttentionEvent
import com.yaca.core.client.HostClient
import com.yaca.core.model.SessionId
import com.yaca.core.protocol.AttentionElicitation
import com.yaca.core.protocol.AttentionPermission
import com.yaca.core.protocol.AttentionQuestion
import com.yaca.core.protocol.AttentionTurnFinished
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** One attention push, with the fingerprint of the host it came from. */
data class HostAttention(val fingerprint: String, val event: AttentionEvent)

/**
 * Which chats wait for the human, per host. A permission, a question or a form adds the
 * chat; a finished turn or opening the chat removes it. UI-free: screens and notifications read
 * [waiting] and [pushed], nothing here knows how it is shown.
 */
class AttentionTracker(private val scope: CoroutineScope) {
    private val waitingState = MutableStateFlow<Map<String, Set<SessionId>>>(emptyMap())

    /** Fingerprint of the host to the chats on it that wait for the human. */
    val waiting: StateFlow<Map<String, Set<SessionId>>> = waitingState.asStateFlow()

    private val pushes = MutableSharedFlow<HostAttention>(extraBufferCapacity = 64)

    /**
     * Every push of every host, as it arrives. The notifications follow this flow: one place
     * collects the pushes of all the clients, instead of each notification keeping its own watcher.
     */
    val pushed: SharedFlow<HostAttention> = pushes.asSharedFlow()

    /** One collector per host, guarded by itself. */
    private val watchers = HashMap<String, Job>()

    /** Follows [client]'s attention pushes. A client that replaces an old one for the same host takes over its watcher. */
    fun watch(client: HostClient) {
        val fingerprint = client.record.fingerprint
        synchronized(watchers) {
            watchers.remove(fingerprint)?.cancel()
            watchers[fingerprint] = scope.launch {
                client.attention.collect { event -> applyEvent(fingerprint, event) }
            }
        }
    }

    /** The host is gone from the phone: stop following it and forget what waited on it. */
    fun forget(fingerprint: String) {
        synchronized(watchers) { watchers.remove(fingerprint) }?.cancel()
        waitingState.update { it - fingerprint }
    }

    /** The human opened the chat: it no longer waits for them, until the agent asks again. */
    fun markSeen(fingerprint: String, session: SessionId) {
        remove(fingerprint, session)
    }

    private fun applyEvent(fingerprint: String, event: AttentionEvent) {
        pushes.tryEmit(HostAttention(fingerprint, event))
        when (event.reason) {
            is AttentionPermission, is AttentionQuestion, is AttentionElicitation -> add(fingerprint, event.session)
            is AttentionTurnFinished -> remove(fingerprint, event.session)
        }
    }

    private fun add(fingerprint: String, session: SessionId) {
        waitingState.update { all -> all + (fingerprint to all[fingerprint].orEmpty() + session) }
    }

    private fun remove(fingerprint: String, session: SessionId) {
        waitingState.update { all ->
            val rest = all[fingerprint].orEmpty() - session
            if (rest.isEmpty()) all - fingerprint else all + (fingerprint to rest)
        }
    }
}
