package com.yaca.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class SentFile(
    val id: DeliveryId,
    val name: String,
    val title: String,
    val caption: String? = null,
    val mime: String,
    val size: Long,
    val sha256: String,
    val renderType: String,
)

@Serializable
@SerialName("fileSent")
data class AgentEventFileSent(val file: SentFile) : AgentEvent
