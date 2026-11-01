package com.yaca.core.model

import kotlinx.serialization.Serializable

/**
 * A message the human sent during a turn that has not yet been taken into it.
 * The chip shows [text] and the names of [attachments]; [id] cancels exactly this one.
 */
@Serializable
data class QueuedMessage(
    val id: QueuedId,
    val text: String,
    val attachments: List<String>,
)
