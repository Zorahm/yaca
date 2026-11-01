package com.yaca.core.protocol

import com.yaca.core.model.Effort
import com.yaca.core.model.Mode
import com.yaca.core.model.SessionId
import com.yaca.core.model.SessionMeta
import com.yaca.core.model.SessionState
import kotlinx.serialization.Serializable

/** A chat running on the host. [meta] is null for a chat that is not written down yet. */
@Serializable
data class ChatSummary(
    val id: SessionId,
    /** The project as the host's list names it. */
    val project: String,
    val state: SessionState,
    val meta: SessionMeta? = null,
)

/**
 * What a new chat starts with, chosen before it exists: the same four choices the
 * desktop's home screen makes.
 */
@Serializable
data class Startup(
    val mode: Mode,
    val provider: String,
    val model: String,
    val effort: Effort,
)

/** The host's identity, sent with every handshake answer. */
@Serializable
data class HostInfo(
    /** The machine's name, for the phone's list of hosts. */
    val name: String,
    /** The Yaca version the host runs. */
    val version: String,
    val protocol: Int,
)
