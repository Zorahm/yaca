@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator
import kotlinx.serialization.json.JsonElement

/** What the human did with a form, discriminated by `type`. */
@Serializable
@JsonClassDiscriminator("type")
sealed interface ElicitationReply

@Serializable
@SerialName("accept")
data class ElicitationReplyAccept(
    /** One value per field that was filled, keyed by the field's name. */
    val values: Map<String, JsonElement>,
) : ElicitationReply

@Serializable
@SerialName("decline")
data object ElicitationReplyDecline : ElicitationReply

@Serializable
@SerialName("cancel")
data object ElicitationReplyCancel : ElicitationReply
