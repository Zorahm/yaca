package com.yaca.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** What a diff line is: unchanged context, an addition, or a deletion. */
@Serializable
enum class LineKind {
    @SerialName("context") CONTEXT,
    @SerialName("add") ADD,
    @SerialName("del") DEL,
}

/** A character range inside a line, for word-level highlighting. End is exclusive. */
@Serializable
data class Span(
    val start: Int,
    val end: Int,
)

@Serializable
data class DiffLine(
    val kind: LineKind,
    /** 1-based line number before the edit: the left gutter. */
    val oldLine: Int? = null,
    /** 1-based line number after the edit. */
    val newLine: Int? = null,
    val text: String,
    /** What changed inside the line, in characters (the interface slices strings by characters). */
    val spans: List<Span>,
)

@Serializable
data class Hunk(
    val oldStart: Int,
    val oldLines: Int,
    val newStart: Int,
    val newLines: Int,
    val lines: List<DiffLine>,
)

@Serializable
data class FileDiff(
    val hunks: List<Hunk>,
    val added: Int,
    val removed: Int,
    /** The diff is cut off at the host's line limit. */
    val truncated: Boolean,
)
