package com.yaca.core.client

import com.yaca.core.protocol.CallResult
import kotlinx.coroutines.CompletableDeferred

/**
 * The calls waiting for a response on one connection, by request id. Guarded by one lock so a
 * registration and a close cannot interleave: once closed, nothing new can wait forever.
 */
internal class PendingCalls {
    private val lock = Any()
    private val waiting = HashMap<Long, CompletableDeferred<CallResult>>()
    private var closed = false

    /** The answer for [id], or null when the connection is already closed. */
    fun register(id: Long): CompletableDeferred<CallResult>? = synchronized(lock) {
        if (closed) {
            null
        } else {
            CompletableDeferred<CallResult>().also { waiting[id] = it }
        }
    }

    fun forget(id: Long) {
        synchronized(lock) { waiting.remove(id) }
    }

    fun complete(id: Long, result: CallResult) {
        synchronized(lock) { waiting.remove(id)?.complete(result) }
    }

    /** Fails every waiting call: the connection is gone and no answer will come. */
    fun close() {
        synchronized(lock) {
            closed = true
            waiting.values.forEach { it.completeExceptionally(NotConnectedException()) }
            waiting.clear()
        }
    }
}
