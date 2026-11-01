@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/** Why a checkpoint was taken. Named, not worded: the feed writes the line. */
@Serializable
@JsonClassDiscriminator("type")
sealed interface CheckpointReason

@Serializable
@SerialName("message")
data object CheckpointReasonMessage : CheckpointReason

@Serializable
@SerialName("command")
data class CheckpointReasonCommand(
    val command: String,
) : CheckpointReason

@Serializable
@SerialName("rollback")
data object CheckpointReasonRollback : CheckpointReason

/** A point in the checkpoint list, and the divider line in the feed. */
@Serializable
data class CheckpointSummary(
    val id: CheckpointId,
    val at: TsMillis,
    val reason: CheckpointReason,
    /** How many files the snapshot holds. */
    val tracked: Int,
)

/** One line of the list a rollback shows before it is confirmed. */
@Serializable
data class CheckpointChange(
    val path: String,
    /** What the rollback does to the file: `created` brings it back, `deleted` takes it away. */
    val kind: ChangeKind,
    val added: Int,
    val removed: Int,
)
