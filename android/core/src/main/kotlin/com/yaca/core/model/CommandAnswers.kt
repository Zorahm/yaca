@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Commands that answer a request the chat raised: a permission, a question, a form.

@Serializable
@SerialName("answerPermission")
data class CommandAnswerPermission(
    val requestId: RequestId,
    val answer: PermissionAnswer,
) : Command

@Serializable
@SerialName("answerQuestion")
data class CommandAnswerQuestion(
    val requestId: RequestId,
    val answer: QuestionReply,
) : Command

@Serializable
@SerialName("answerElicitation")
data class CommandAnswerElicitation(
    val requestId: RequestId,
    val reply: ElicitationReply,
) : Command
