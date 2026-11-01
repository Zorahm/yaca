package com.yaca.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** Why a turn ended. */
@Serializable
enum class StopReason {
    @SerialName("endTurn") END_TURN,
    @SerialName("maxTokens") MAX_TOKENS,
    @SerialName("stopSequence") STOP_SEQUENCE,
    @SerialName("toolUse") TOOL_USE,
    @SerialName("refusal") REFUSAL,
    @SerialName("pauseTurn") PAUSE_TURN,
    @SerialName("failed") FAILED,
}
