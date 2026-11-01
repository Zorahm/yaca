package com.yaca.core.client

import kotlin.random.Random

/**
 * Reconnect waits: they double from [initialMillis] up to [maxMillis]. Each wait is drawn from the
 * upper half of its step, so phones that lost the same host together do not retry in lockstep.
 * [reset] after a success.
 */
internal class Backoff(
    private val initialMillis: Long,
    private val maxMillis: Long,
    private val random: Random,
) {
    private var ceilingMillis = initialMillis

    /** The attempt about to be made, counted from 1 since the last success. */
    var attempt: Int = 1
        private set

    /** The wait before the next attempt, in milliseconds. */
    fun nextWaitMillis(): Long {
        val half = ceilingMillis / 2
        val wait = half + random.nextLong(half + 1)
        ceilingMillis = minOf(ceilingMillis * 2, maxMillis)
        attempt++
        return wait
    }

    fun reset() {
        ceilingMillis = initialMillis
        attempt = 1
    }
}
