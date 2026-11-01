package com.yaca.android.host

import android.content.Context
import com.yaca.android.R
import com.yaca.core.client.ConnectionState
import com.yaca.core.model.SessionMeta

sealed interface HostUiState {
    /** The store has not answered yet. */
    data object Loading : HostUiState

    /** The computer is no longer paired: the route goes back. */
    data object Gone : HostUiState

    data class Ready(
        val name: String,
        val connection: ConnectionState,
        /** The chats running on the host right now, as its last `ChatsChanged` listed them. */
        val running: List<RunningChat>,
        val projects: ProjectsState,
        /** Projects whose chats are open in the list, by project path. */
        val expanded: Map<String, SessionsState>,
        val refreshing: Boolean,
        /** A chat is being opened: the list does not start a second one meanwhile. */
        val opening: Boolean,
        val newChatOpen: Boolean,
        val forgetting: Boolean,
    ) : HostUiState
}

/** A chat running on the host, shaped for the row. [title] is null for a chat not written down yet. */
data class RunningChat(
    val session: String,
    val title: String?,
    val project: String,
    val busy: Boolean,
    val waiting: Boolean,
)

sealed interface ProjectsState {
    data object Loading : ProjectsState
    data class Loaded(val projects: List<String>) : ProjectsState

    /** The host could not list its projects (offline, or it answered with an error). */
    data object Unavailable : ProjectsState
}

sealed interface SessionsState {
    data object Loading : SessionsState
    data class Loaded(val sessions: List<SessionMeta>) : SessionsState
    data object Unavailable : SessionsState
}

/** What a failed call tells the human: a snackbar, in the host's words where it has them. */
sealed interface HostError {
    data object NotConnected : HostError
    data class Remote(val message: String) : HostError
}

fun HostError.text(context: Context): String = when (this) {
    HostError.NotConnected -> context.getString(R.string.error_not_connected)
    is HostError.Remote -> message
}
