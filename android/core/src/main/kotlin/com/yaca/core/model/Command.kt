@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/**
 * A core command for a running chat, discriminated by `type`. Its effect arrives
 * as events of the attached chat, not as a reply. The variants are split over the
 * `Command*` files, named after this type and their own `type` value.
 */
@Serializable
@JsonClassDiscriminator("type")
sealed interface Command

@Serializable
@SerialName("sendMessage")
data class CommandSendMessage(
    val text: String,
    /** Files attached with `@`. They count as read: the human attached them, so the agent need not read them again. */
    val attachments: List<String> = emptyList(),
) : Command

/** `Esc`: unfinished tools end as cancelled. */
@Serializable
@SerialName("cancel")
data object CommandCancel : Command

@Serializable
@SerialName("setMode")
data class CommandSetMode(
    val mode: Mode,
) : Command

/** What the chat is for, switched the way the mode is. */
@Serializable
@SerialName("setProfile")
data class CommandSetProfile(
    val profile: Profile,
) : Command

@Serializable
@SerialName("setModel")
data class CommandSetModel(
    val model: String,
) : Command

@Serializable
@SerialName("setEffort")
data class CommandSetEffort(
    val effort: Effort,
) : Command

@Serializable
@SerialName("setBypass")
data class CommandSetBypass(
    val on: Boolean,
) : Command

@Serializable
@SerialName("compact")
data object CommandCompact : Command

@Serializable
@SerialName("editSummary")
data class CommandEditSummary(
    val text: String,
) : Command
