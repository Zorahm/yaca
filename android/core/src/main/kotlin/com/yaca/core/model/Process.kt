@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/** The state column of the process panel, discriminated by `type`. */
@Serializable
@JsonClassDiscriminator("type")
sealed interface ProcessState

@Serializable
@SerialName("running")
data object ProcessStateRunning : ProcessState

@Serializable
@SerialName("exited")
data class ProcessStateExited(
    /** The exit code; null when the process ended without one. */
    val code: Int? = null,
) : ProcessState

@Serializable
@SerialName("stopped")
data object ProcessStateStopped : ProcessState

/** One row of the process panel. The interface computes "uptime" from [startedAt] and [finishedAt]. */
@Serializable
data class ProcessSummary(
    val id: ProcessId,
    val command: String,
    /** Working directory, relative to the project root. */
    val cwd: String,
    val state: ProcessState,
    val startedAt: TsMillis,
    /** Null while running. */
    val finishedAt: TsMillis? = null,
    /** Lines currently in the buffer. */
    val lines: Int,
    /** Lines pushed out of the buffer. */
    val dropped: Long,
)
