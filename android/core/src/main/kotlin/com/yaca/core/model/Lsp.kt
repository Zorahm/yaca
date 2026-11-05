package com.yaca.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** What state a language server is in. */
@Serializable
enum class LspStatus {
    @SerialName("idle") IDLE,
    @SerialName("starting") STARTING,
    @SerialName("ready") READY,
    @SerialName("failed") FAILED,
    @SerialName("disabled") DISABLED,
    @SerialName("missing") MISSING,
    @SerialName("installing") INSTALLING,
}

/** One language server of the session, as the interface lists it. */
@Serializable
data class LspServerSummary(
    val name: String,
    /** The command line, as declared. */
    val command: String,
    val extensions: List<String>,
    val status: LspStatus,
    /** Why it failed, in the human's language. */
    val reason: String? = null,
    /** How the server introduces itself: `name version`. */
    val label: String? = null,
    val installation: String? = null,
)
