@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Variants of AgentEvent about checkpoints: snapshots of the files, shown as
// dividers in the feed and listed for rollback.

@Serializable
@SerialName("checkpointCreating")
data class AgentEventCheckpointCreating(val id: CheckpointId, val reason: CheckpointReason) : AgentEvent

@Serializable
@SerialName("checkpointCreationFinished")
data class AgentEventCheckpointCreationFinished(val id: CheckpointId) : AgentEvent

@Serializable
@SerialName("checkpointCreated")
data class AgentEventCheckpointCreated(
    val checkpoint: CheckpointSummary,
) : AgentEvent

@Serializable
@SerialName("checkpointsUpdated")
data class AgentEventCheckpointsUpdated(
    val checkpoints: List<CheckpointSummary>,
) : AgentEvent

/** The files a rollback to [id] would change, listed before it is confirmed. */
@Serializable
@SerialName("checkpointPreviewed")
data class AgentEventCheckpointPreviewed(
    val id: CheckpointId,
    val files: List<CheckpointChange>,
) : AgentEvent

@Serializable
@SerialName("checkpointRestored")
data class AgentEventCheckpointRestored(
    val id: CheckpointId,
    val at: TsMillis,
    val files: Int,
) : AgentEvent
