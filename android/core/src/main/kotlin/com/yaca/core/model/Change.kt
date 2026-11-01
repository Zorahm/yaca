@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/** What a file action did to the file. */
@Serializable
enum class ChangeKind {
    @SerialName("created") CREATED,
    @SerialName("modified") MODIFIED,
    @SerialName("deleted") DELETED,
    @SerialName("moved") MOVED,
}

/** A fact about a file action; the interface writes the action's caption from it. */
@Serializable
data class FileFact(
    val path: String,
    val kind: ChangeKind,
    val movedFrom: String? = null,
    val added: Int,
    val removed: Int,
)

/**
 * A file's full diff against its baseline, with the unchanged lines between the
 * hunks. A file the session only read has the same shape with no hunks.
 */
@Serializable
data class FileView(
    /** Null when the session has not changed the file. */
    val fact: FileFact? = null,
    val segments: List<ViewSegment>,
    /** The view stops before the rest of the file; hunks past the cut are not shown. */
    val truncated: Boolean,
)

/** One piece of a file's view, in order. Unchanged text is never split into hunks. */
@Serializable
@JsonClassDiscriminator("kind")
sealed interface ViewSegment

@Serializable
@SerialName("unchanged")
data class ViewSegmentUnchanged(
    val lines: List<DiffLine>,
) : ViewSegment

@Serializable
@SerialName("hunk")
data class ViewSegmentHunk(
    val hunk: Hunk,
) : ViewSegment
