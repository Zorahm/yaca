@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Variants of AgentEvent about files: what was read, what changed, what was found.

/**
 * A slice of a file, or of a background command's output, that the agent read.
 * [total] tells a whole file from a part of one.
 */
@Serializable
@SerialName("textShown")
data class AgentEventTextShown(
    /** The file, or for a command's output `command <id>`. */
    val path: String,
    /** The command line, when what is shown is a background command's output. */
    val command: String? = null,
    val firstLine: Int,
    val lines: Int,
    val total: Int,
    val text: String,
) : AgentEvent

@Serializable
@SerialName("fileChanged")
data class AgentEventFileChanged(
    val callId: CallId,
    val fact: FileFact,
    val diff: FileDiff,
) : AgentEvent

/** [view] is null for a file the session has not changed; there is no diff to show then. */
@Serializable
@SerialName("fileViewLoaded")
data class AgentEventFileViewLoaded(
    val path: String,
    val view: FileView? = null,
) : AgentEvent

@Serializable
@SerialName("filesFound")
data class AgentEventFilesFound(
    val query: String,
    val paths: List<String>,
) : AgentEvent
