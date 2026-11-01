package com.yaca.android.hosts

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.yaca.android.AppContainer
import com.yaca.core.client.HostRecord
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/** The paired computers, each with the connection state of its client, and the forget dialog. */
class HostsViewModel(private val container: AppContainer) : ViewModel() {
    /** Fingerprint of the computer the human is asked to forget; null when no dialog is open. */
    private val forgetting = MutableStateFlow<String?>(null)

    val state: StateFlow<HostsUiState> = combine(container.hosts, forgetting) { hosts, pending ->
        hosts to pending
    }
        .flatMapLatest { (hosts, pending) -> rowsFor(hosts, pending) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MS), HostsUiState.Loading)

    private fun rowsFor(hosts: List<HostRecord>?, pending: String?): Flow<HostsUiState> = when {
        hosts == null -> flowOf(HostsUiState.Loading)
        hosts.isEmpty() -> flowOf(HostsUiState.Loaded(rows = emptyList()))
        else -> combine(hosts.map { container.clientFor(it).state }) { states ->
            HostsUiState.Loaded(
                rows = hosts.zip(states.toList()) { record, connection -> HostRow(record, connection) },
                forgetting = hosts.firstOrNull { it.fingerprint == pending },
            )
        }
    }

    fun askForget(fingerprint: String) {
        forgetting.value = fingerprint
    }

    fun dismissForget() {
        forgetting.value = null
    }

    /** Removing the record makes the container drop the host's client, so the connection stops with it. */
    fun confirmForget() {
        val fingerprint = forgetting.value ?: return
        forgetting.value = null
        viewModelScope.launch { container.hostStore.remove(fingerprint) }
    }

    private companion object {
        const val STOP_TIMEOUT_MS = 5_000L
    }
}
