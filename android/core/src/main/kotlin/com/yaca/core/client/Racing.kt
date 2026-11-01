package com.yaca.core.client

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.joinAll
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Runs every attempt at once and returns the first that succeeds; the rest are cancelled. An attempt
 * that succeeds after the winner was picked goes to [discard], so an open socket is never leaked.
 * Throws the last failure when every attempt has failed.
 */
internal suspend fun <T : Any> firstSuccess(attempts: List<suspend () -> T>, discard: (T) -> Unit): T =
    coroutineScope {
        require(attempts.isNotEmpty()) { "there is nothing to try" }
        val outcomes = Channel<Result<T>>(Channel.UNLIMITED)
        val jobs = attempts.map { attempt ->
            launch {
                val outcome = try {
                    Result.success(attempt())
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    Result.failure(e)
                }
                outcomes.trySend(outcome)
            }
        }
        try {
            var failure: Throwable? = null
            repeat(attempts.size) {
                val outcome = outcomes.receive()
                val value = outcome.getOrNull()
                if (value != null) return@coroutineScope value
                failure = outcome.exceptionOrNull()
            }
            throw failure ?: IllegalStateException("no attempt answered")
        } finally {
            jobs.forEach { it.cancel() }
            // Joined before draining: an attempt that was still finishing must land in the channel first.
            withContext(NonCancellable) { jobs.joinAll() }
            outcomes.close()
            while (true) {
                val late = outcomes.tryReceive().getOrNull() ?: break
                late.getOrNull()?.let(discard)
            }
        }
    }
