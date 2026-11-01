@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/** One ready answer the human can pick. */
@Serializable
data class QuestionOption(
    /** Short; the model reads it back when this option is picked. */
    val label: String,
    val description: String? = null,
)

/** A question on screen. */
@Serializable
data class QuestionRequest(
    val id: RequestId,
    val callId: CallId,
    val question: String,
    val options: List<QuestionOption>,
    /** The human may type an answer of their own, instead of or next to the options. */
    val allowOther: Boolean,
    val multiSelect: Boolean,
)

/** What the human did with a question. */
@Serializable
@JsonClassDiscriminator("type")
sealed interface QuestionReply

@Serializable
@SerialName("answered")
data class QuestionReplyAnswered(
    /** Indexes into [QuestionRequest.options]. */
    val selected: List<Int>,
    val other: String? = null,
) : QuestionReply

@Serializable
@SerialName("dismissed")
data object QuestionReplyDismissed : QuestionReply
