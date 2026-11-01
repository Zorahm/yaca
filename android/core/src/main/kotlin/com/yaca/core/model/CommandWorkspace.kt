@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Commands about the chat's workspace: checkpoints, background processes, files,
// and the queue of the human's messages.

@Serializable
@SerialName("previewCheckpoint")
data class CommandPreviewCheckpoint(
    val id: CheckpointId,
) : Command

@Serializable
@SerialName("restoreCheckpoint")
data class CommandRestoreCheckpoint(
    val id: CheckpointId,
) : Command

@Serializable
@SerialName("watchProcess")
data class CommandWatchProcess(
    val id: ProcessId? = null,
) : Command

@Serializable
@SerialName("stopProcess")
data class CommandStopProcess(
    val id: ProcessId,
) : Command

@Serializable
@SerialName("readFileView")
data class CommandReadFileView(
    val path: String,
) : Command

@Serializable
@SerialName("cancelQueued")
data class CommandCancelQueued(
    val id: QueuedId,
) : Command

@Serializable
@SerialName("searchFiles")
data class CommandSearchFiles(
    val query: String,
) : Command
