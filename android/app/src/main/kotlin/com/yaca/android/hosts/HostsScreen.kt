@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.hosts

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.yaca.android.R
import com.yaca.android.appViewModelFactory
import com.yaca.android.ui.common.CenteredProgress
import com.yaca.android.ui.common.ForgetComputerDialog
import com.yaca.android.ui.common.SectionLabel
import com.yaca.android.ui.theme.YacaTheme
import com.yaca.core.client.ConnectionState
import com.yaca.core.client.HostRecord
import com.yaca.core.protocol.HostInfo
import com.yaca.core.protocol.RefusalUnauthorized

/** The route: the list of paired computers, from the view model. */
@Composable
fun HostsRoute(onPair: () -> Unit, onOpenHost: (String) -> Unit, onOpenSettings: () -> Unit) {
    val vm: HostsViewModel = viewModel(factory = appViewModelFactory { HostsViewModel(it) })
    val state by vm.state.collectAsStateWithLifecycle()
    HostsScreen(
        state = state,
        onPair = onPair,
        onOpenHost = onOpenHost,
        onOpenSettings = onOpenSettings,
        onAskForget = vm::askForget,
        onDismissForget = vm::dismissForget,
        onConfirmForget = vm::confirmForget,
    )
}

/** Stateless: the list, the empty state and the forget dialog, drawn from [state]. */
@Composable
fun HostsScreen(
    state: HostsUiState,
    onPair: () -> Unit,
    onOpenHost: (String) -> Unit,
    onOpenSettings: () -> Unit,
    onAskForget: (String) -> Unit,
    onDismissForget: () -> Unit,
    onConfirmForget: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Scaffold(
        modifier = modifier,
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.hosts_title)) },
                actions = {
                    IconButton(onClick = onPair) {
                        Icon(
                            imageVector = Icons.Filled.QrCodeScanner,
                            contentDescription = stringResource(R.string.hosts_pair_action),
                        )
                    }
                    HostsOverflow(onOpenSettings = onOpenSettings)
                },
            )
        },
    ) { padding ->
        when (state) {
            HostsUiState.Loading -> CenteredProgress(Modifier.padding(padding))
            is HostsUiState.Loaded -> if (state.rows.isEmpty()) {
                EmptyHosts(onPair = onPair, modifier = Modifier.padding(padding))
            } else {
                HostsList(
                    rows = state.rows,
                    onPair = onPair,
                    onOpenHost = onOpenHost,
                    onAskForget = onAskForget,
                    modifier = Modifier.padding(padding),
                )
            }
        }
    }

    (state as? HostsUiState.Loaded)?.forgetting?.let { record ->
        ForgetComputerDialog(
            name = record.name,
            onConfirm = onConfirmForget,
            onDismiss = onDismissForget,
        )
    }
}

/** The top bar's overflow: the app-wide items that do not belong to one computer. */
@Composable
private fun HostsOverflow(onOpenSettings: () -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.action_more))
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.settings_title)) },
                onClick = {
                    open = false
                    onOpenSettings()
                },
            )
        }
    }
}

@Composable
private fun HostsList(
    rows: List<HostRow>,
    onPair: () -> Unit,
    onOpenHost: (String) -> Unit,
    onAskForget: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item { SectionLabel(stringResource(R.string.hosts_section_paired)) }
        items(rows, key = { it.record.fingerprint }) { row ->
            HostCard(
                row = row,
                onOpen = { onOpenHost(row.record.fingerprint) },
                onForget = { onAskForget(row.record.fingerprint) },
                onPairAgain = onPair,
            )
        }
        item {
            Text(
                text = stringResource(R.string.hosts_note),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        item {
            OutlinedButton(onClick = onPair, modifier = Modifier.fillMaxWidth()) {
                Text(stringResource(R.string.hosts_pair_another))
            }
        }
    }
}

@Composable
private fun EmptyHosts(onPair: () -> Unit, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = stringResource(R.string.hosts_empty),
            style = MaterialTheme.typography.bodyLarge,
            textAlign = TextAlign.Center,
        )
        Button(onClick = onPair, modifier = Modifier.padding(top = 16.dp)) {
            Text(stringResource(R.string.hosts_pair_action))
        }
    }
}

private val sampleHost = HostRecord(
    name = "ZORAH-PC",
    addresses = listOf("192.168.0.10"),
    port = 7443,
    fingerprint = "3f9a0c",
    deviceId = "device-1",
    token = "not-a-real-token",
)

@Preview(showBackground = true)
@Composable
private fun HostsPreview() {
    YacaTheme {
        HostsScreen(
            state = HostsUiState.Loaded(
                rows = listOf(
                    HostRow(
                        record = sampleHost,
                        connection = ConnectionState.Connected(
                            host = HostInfo(name = "ZORAH-PC", version = "1.0.0-beta", protocol = 1),
                            address = "192.168.0.10",
                        ),
                    ),
                    HostRow(
                        record = sampleHost.copy(name = "NOTES-PC", fingerprint = "77de21"),
                        connection = ConnectionState.Refused(RefusalUnauthorized),
                    ),
                ),
            ),
            onPair = {},
            onOpenHost = {},
            onOpenSettings = {},
            onAskForget = {},
            onDismissForget = {},
            onConfirmForget = {},
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun HostsEmptyPreview() {
    YacaTheme {
        HostsScreen(
            state = HostsUiState.Loaded(rows = emptyList()),
            onPair = {},
            onOpenHost = {},
            onOpenSettings = {},
            onAskForget = {},
            onDismissForget = {},
            onConfirmForget = {},
        )
    }
}
