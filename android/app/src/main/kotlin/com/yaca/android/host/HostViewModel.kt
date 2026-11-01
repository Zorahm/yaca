package com.yaca.android.host

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.yaca.android.AppContainer
import com.yaca.android.ui.common.projectShortName
import com.yaca.core.client.ConnectionState
import com.yaca.core.client.HostClient
import com.yaca.core.client.HostRecord
import com.yaca.core.client.NotConnectedException
import com.yaca.core.client.RemoteCallException
import com.yaca.core.model.SessionId
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/**
 * One computer: its connection, running chats, projects and the chats of each project. Reads the
 * host's state from its client; only what the screen itself decides (what is expanded, what is
 * being opened) lives here.
 */
class HostViewModel(
    private val container: AppContainer,
    private val fingerprint: String,
) : ViewModel() {
    private val local = MutableStateFlow(ScreenState())
    private val openedChats = Channel<String>(Channel.BUFFERED)
    private val failures = Channel<HostError>(Channel.BUFFERED)

    /** Session ids of chats this screen just opened: the route navigates to them. */
    val openedSessions: Flow<String> = openedChats.receiveAsFlow()

    /** Failed calls, for the snackbar. */
    val errors: Flow<HostError> = failures.receiveAsFlow()

    val state: StateFlow<HostUiState> = container.hosts
        .flatMapLatest { hosts -> stateFor(hosts) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MS), HostUiState.Loading)

    init {
        // Every time the connection comes up (the first time, or after a drop) the lists are fetched anew.
        viewModelScope.launch {
            container.hosts
                .flatMapLatest { hosts -> connectionOf(hosts) }
                .filterIsInstance<ConnectionState.Connected>()
                .collect { refresh() }
        }
    }

    private fun stateFor(hosts: List<HostRecord>?): Flow<HostUiState> {
        if (hosts == null) return flowOf(HostUiState.Loading)
        val record = hosts.firstOrNull { it.fingerprint == fingerprint } ?: return flowOf(HostUiState.Gone)
        val client = container.clientFor(record)
        return combine(client.state, client.chats, container.attention.waiting, local) { connection, chats, waiting, screen ->
            val waitingHere = waiting[fingerprint].orEmpty()
            HostUiState.Ready(
                name = record.name,
                connection = connection,
                running = chats.map { chat ->
                    RunningChat(
                        session = chat.id.value,
                        title = chat.meta?.title,
                        project = projectShortName(chat.project),
                        busy = chat.state.busy,
                        waiting = chat.id in waitingHere,
                    )
                },
                projects = screen.projects,
                expanded = screen.expanded,
                refreshing = screen.refreshing,
                opening = screen.opening,
                newChatOpen = screen.newChatOpen,
                forgetting = screen.forgetting,
            )
        }
    }

    private fun connectionOf(hosts: List<HostRecord>?): Flow<ConnectionState> {
        val record = hosts?.firstOrNull { it.fingerprint == fingerprint } ?: return emptyFlow()
        return container.clientFor(record).state
    }

    /** Fetches the projects, then the chats of every project that is open in the list. */
    fun refresh() {
        val client = currentClient() ?: return
        viewModelScope.launch {
            local.update { it.copy(refreshing = true) }
            try {
                loadProjects(client)
                for (project in local.value.expanded.keys.toList()) loadSessions(client, project)
            } finally {
                local.update { it.copy(refreshing = false) }
            }
        }
    }

    fun toggleProject(project: String) {
        if (project in local.value.expanded) {
            local.update { it.copy(expanded = it.expanded - project) }
            return
        }
        val client = currentClient() ?: return
        local.update { it.copy(expanded = it.expanded + (project to SessionsState.Loading)) }
        viewModelScope.launch { loadSessions(client, project) }
    }

    /** A chat recorded on disk: opening it resumes it on the host. */
    fun openRecorded(project: String, session: String) {
        open(project, resume = SessionId(session))
    }

    fun openNew(project: String) {
        open(project, resume = null)
    }

    fun setNewChatOpen(open: Boolean) {
        local.update { it.copy(newChatOpen = open) }
    }

    fun setForgetting(open: Boolean) {
        local.update { it.copy(forgetting = open) }
    }

    /** Removing the record makes the container drop this host's client; the state then goes Gone and the route leaves. */
    fun confirmForget() {
        local.update { it.copy(forgetting = false) }
        viewModelScope.launch { container.hostStore.remove(fingerprint) }
    }

    private fun open(project: String, resume: SessionId?) {
        if (local.value.opening) return
        val client = currentClient() ?: return
        local.update { it.copy(opening = true) }
        viewModelScope.launch {
            val chat = call { client.open(project, resume = resume) }
            local.update { it.copy(opening = false, newChatOpen = false) }
            if (chat != null) openedChats.send(chat.id.value)
        }
    }

    private suspend fun loadProjects(client: HostClient) {
        val projects = call { client.projects() }
        local.update {
            it.copy(projects = if (projects == null) ProjectsState.Unavailable else ProjectsState.Loaded(projects))
        }
    }

    private suspend fun loadSessions(client: HostClient, project: String) {
        val sessions = call { client.sessions(project) }
        val next = if (sessions == null) {
            SessionsState.Unavailable
        } else {
            SessionsState.Loaded(sessions.sortedByDescending { it.updatedAt.value })
        }
        // Collapsed while the call was in flight: the answer is dropped, not shown under a closed row.
        local.update { screen ->
            if (project in screen.expanded) screen.copy(expanded = screen.expanded + (project to next)) else screen
        }
    }

    /** Runs a call to the host; a failure becomes a snackbar and a null answer. */
    private suspend fun <T> call(block: suspend () -> T): T? = try {
        block()
    } catch (remote: RemoteCallException) {
        failures.send(HostError.Remote(remote.message.orEmpty()))
        null
    } catch (offline: NotConnectedException) {
        failures.send(HostError.NotConnected)
        null
    }

    private fun currentClient(): HostClient? =
        container.hosts.value?.firstOrNull { it.fingerprint == fingerprint }?.let { container.clientFor(it) }

    /** What the screen decides for itself, combined with the host's own state in [stateFor]. */
    private data class ScreenState(
        val projects: ProjectsState = ProjectsState.Loading,
        val expanded: Map<String, SessionsState> = emptyMap(),
        val refreshing: Boolean = false,
        val opening: Boolean = false,
        val newChatOpen: Boolean = false,
        val forgetting: Boolean = false,
    )

    private companion object {
        const val STOP_TIMEOUT_MS = 5_000L
    }
}
