package com.yaca.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The status of a todo item. The core also accepts `in_progress` from journals of
 * older builds, but it writes `inProgress` back out, so the wire never carries
 * the old spelling and the client does not accept it.
 */
@Serializable
enum class TodoStatus {
    @SerialName("pending") PENDING,
    @SerialName("inProgress") IN_PROGRESS,
    @SerialName("completed") COMPLETED,
}

@Serializable
data class TodoItem(
    val content: String,
    val status: TodoStatus,
)
