package com.yaca.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** Session mode: the only thing the human switches. Declared from cautious to trusting. */
@Serializable
enum class Mode {
    @SerialName("plan") PLAN,
    @SerialName("manual") MANUAL,
    @SerialName("auto") AUTO,
    @SerialName("edits") EDITS,
}

/** What a chat is for: code, or the person's documents. Not a level of trust. */
@Serializable
enum class Profile {
    @SerialName("code") CODE,
    @SerialName("chat") CHAT,
}

/** Reasoning effort, the bottom-right corner of the composer. */
@Serializable
enum class Effort {
    @SerialName("low") LOW,
    @SerialName("medium") MEDIUM,
    @SerialName("high") HIGH,
    @SerialName("extra") EXTRA,
    @SerialName("max") MAX,
}

/** Where a chat works, as the interface shows it. */
@Serializable
data class Isolation(
    /** The chat's branch. Merging it is how the work comes back. */
    val branch: String,
    /** The directory the chat's tools run in: the worktree, or the project's folder inside it. */
    val path: String,
)

/** Snapshot of the session settings for the composer's bottom row. */
@Serializable
data class SessionState(
    val projectRoot: String,
    /** Absent from journals written before the field existed; decodes to empty. */
    val provider: String = "",
    val model: String,
    val mode: Mode,
    /**
     * What the chat is for. The phone's composer does not offer it yet — it is switched from the
     * desktop — but the field decodes, so a chat started here shows the truth.
     */
    val profile: Profile = Profile.CODE,
    val effort: Effort,
    val busy: Boolean,
    /** Permission bypass: a flag on top of the mode, not a fifth mode. */
    val bypass: Boolean,
    /** Absent for a chat in the project's own tree. */
    val isolation: Isolation? = null,
)

/**
 * A chat as the host lists it. Every field marked with a default is absent from
 * journals written before the field existed, and decodes to that default.
 *
 * [titleByModel] and [additionalRoots] are not in the TypeScript binding (ts-rs
 * skips them), but the Rust side serializes them, so they are modelled here.
 */
@Serializable
data class SessionMeta(
    val id: SessionId,
    /** The model's name for the session, else cut from the first message, unless renamed. */
    val title: String,
    /** Renamed by hand: the first message no longer decides the title. */
    val titleCustom: Boolean = false,
    /** The model has used its one chance to name the session. */
    val titleByModel: Boolean = false,
    val createdAt: TsMillis,
    val updatedAt: TsMillis,
    val provider: String = "",
    val model: String,
    /** Messages the human sent. */
    val messages: Int = 0,
    val costUsd: Double = 0.0,
    val tokens: TokenCounts = TokenCounts(),
    /** Paths the session changed, relative to the project root. */
    val files: List<String> = emptyList(),
    /** The session this one was duplicated from. */
    val forkedFrom: SessionId? = null,
    /** The additional roots the session was started with, as absolute paths. */
    val additionalRoots: List<String> = emptyList(),
)
