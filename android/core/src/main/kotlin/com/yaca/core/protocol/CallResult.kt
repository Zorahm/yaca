@file:OptIn(ExperimentalSerializationApi::class)

package com.yaca.core.protocol

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/** The outcome of one call, discriminated by `status`. */
@Serializable
@JsonClassDiscriminator("status")
sealed interface CallResult

@Serializable
@SerialName("ok")
data class CallResultOk(
    val reply: Reply,
) : CallResult

@Serializable
@SerialName("error")
data class CallResultError(
    val error: CallError,
) : CallResult

@Serializable
data class CallError(
    val code: ErrorCode,
    /** English, for the human: the client shows it as it is. */
    val message: String,
)

@Serializable
enum class ErrorCode {
    /** The call names a chat that is not running on the host. */
    @SerialName("chatNotOpen") CHAT_NOT_OPEN,
    /** The request could not be read: an unknown call, a missing field. */
    @SerialName("badRequest") BAD_REQUEST,
    /** The host tried and failed: a folder that is gone, a provider that does not resolve, an unreadable journal. */
    @SerialName("host") HOST,
}
