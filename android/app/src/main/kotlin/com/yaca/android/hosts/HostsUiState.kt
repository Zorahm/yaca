package com.yaca.android.hosts

import com.yaca.core.client.ConnectionState
import com.yaca.core.client.HostRecord

/** One paired computer as the list shows it: what was stored and how its connection stands now. */
data class HostRow(val record: HostRecord, val connection: ConnectionState)

sealed interface HostsUiState {
    /** The store has not answered yet. */
    data object Loading : HostsUiState

    /** [forgetting] is the computer the human is asked to forget, or null when no dialog is open. */
    data class Loaded(
        val rows: List<HostRow>,
        val forgetting: HostRecord? = null,
    ) : HostsUiState
}
