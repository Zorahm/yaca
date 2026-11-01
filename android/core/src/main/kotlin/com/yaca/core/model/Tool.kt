package com.yaca.core.model

import kotlinx.serialization.Serializable

/**
 * Counters of a finished tool. File deltas travel separately, in the file-changed
 * event: the diff counts them, not the tool.
 */
@Serializable
data class ToolMeta(
    /** How many output lines went into the feed. */
    val lines: Int,
    /** Matches for `grep`, files for `glob`. */
    val items: Int? = null,
    /** The output is cut; the full one is available by a button. */
    val truncated: Boolean,
    /** How long it took, in milliseconds. */
    val durationMs: Int,
    /**
     * Images the call returned, as paths relative to the project. Rust always writes
     * the array, so it is never absent on the wire; an empty list is the common case.
     */
    val pictures: List<String> = emptyList(),
)
