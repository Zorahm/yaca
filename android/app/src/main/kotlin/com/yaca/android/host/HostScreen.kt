@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.host

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.yaca.android.R
import com.yaca.android.appViewModelFactory
import com.yaca.android.ui.common.CenteredProgress
import com.yaca.android.ui.common.ForgetComputerDialog
import com.yaca.android.ui.common.ForgetMenu
import com.yaca.android.ui.common.SectionLabel
import com.yaca.android.ui.common.connectionColor
import com.yaca.android.ui.common.connectionLabel
import com.yaca.core.client.ConnectionState

/** The route: one computer's state from its view model; a snackbar for failed calls; back when it is forgotten. */
@Composable
fun HostRoute(
    fingerprint: String,
    onBack: () -> Unit,
    onPairAgain: () -> Unit,
    onOpenChat: (String) -> Unit,
) {
    val vm: HostViewModel = viewModel(
        key = fingerprint,
        factory = appViewModelFactory { HostViewModel(it, fingerprint) },
    )
    val state by vm.state.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    val context = LocalContext.current

    LaunchedEffect(vm) {
        vm.openedSessions.collect { onOpenChat(it) }
    }
    LaunchedEffect(vm) {
        vm.errors.collect { snackbar.showSnackbar(it.text(context)) }
    }
    LaunchedEffect(state) {
        if (state == HostUiState.Gone) onBack()
    }

    HostScreen(
        state = state,
        snackbarHostState = snackbar,
        onBack = onBack,
        onRefresh = vm::refresh,
        onPairAgain = onPairAgain,
        onOpenRunning = onOpenChat,
        onToggleProject = vm::toggleProject,
        onOpenRecorded = vm::openRecorded,
        onShowNewChat = { vm.setNewChatOpen(true) },
        onDismissNewChat = { vm.setNewChatOpen(false) },
        onOpenNewChat = vm::openNew,
        onAskForget = { vm.setForgetting(true) },
        onDismissForget = { vm.setForgetting(false) },
        onConfirmForget = vm::confirmForget,
    )
}

/**
 * Stateless: the top bar with connection and overflow, the running chats, the projects, the
 * new-chat sheet and the forget dialog, all drawn from [state].
 */
@Composable
fun HostScreen(
    state: HostUiState,
    snackbarHostState: SnackbarHostState,
    onBack: () -> Unit,
    onRefresh: () -> Unit,
    onPairAgain: () -> Unit,
    onOpenRunning: (String) -> Unit,
    onToggleProject: (String) -> Unit,
    onOpenRecorded: (String, String) -> Unit,
    onShowNewChat: () -> Unit,
    onDismissNewChat: () -> Unit,
    onOpenNewChat: (String) -> Unit,
    onAskForget: () -> Unit,
    onDismissForget: () -> Unit,
    onConfirmForget: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val ready = state as? HostUiState.Ready
    Scaffold(
        modifier = modifier,
        topBar = {
            HostTopBar(
                name = ready?.name.orEmpty(),
                connection = ready?.connection,
                onBack = onBack,
                onRefresh = onRefresh,
                onAskForget = onAskForget,
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
        floatingActionButton = {
            // Only offered while connected: a chat cannot be started on an unreachable computer.
            if (ready != null && ready.connection is ConnectionState.Connected) {
                ExtendedFloatingActionButton(
                    onClick = onShowNewChat,
                    icon = { Icon(Icons.Filled.Add, contentDescription = null) },
                    text = { Text(stringResource(R.string.host_new_chat)) },
                )
            }
        },
    ) { padding ->
        when (state) {
            HostUiState.Loading -> CenteredProgress(Modifier.padding(padding))
            HostUiState.Gone -> Unit
            is HostUiState.Ready -> HostBody(
                ready = state,
                padding = padding,
                onRefresh = onRefresh,
                onPairAgain = onPairAgain,
                onOpenRunning = onOpenRunning,
                onToggleProject = onToggleProject,
                onOpenRecorded = onOpenRecorded,
            )
        }
    }

    if (ready != null && ready.forgetting) {
        ForgetComputerDialog(name = ready.name, onConfirm = onConfirmForget, onDismiss = onDismissForget)
    }
    if (ready != null && ready.newChatOpen) {
        NewChatSheet(
            projects = ready.projects,
            opening = ready.opening,
            onPick = onOpenNewChat,
            onDismiss = onDismissNewChat,
        )
    }
}

@Composable
private fun HostTopBar(
    name: String,
    connection: ConnectionState?,
    onBack: () -> Unit,
    onRefresh: () -> Unit,
    onAskForget: () -> Unit,
) {
    TopAppBar(
        title = {
            Column {
                Text(
                    text = name,
                    style = MaterialTheme.typography.titleLarge,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (connection != null) {
                    Text(
                        text = connectionLabel(connection),
                        style = MaterialTheme.typography.bodySmall,
                        color = connectionColor(connection),
                    )
                }
            }
        },
        navigationIcon = {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.action_back))
            }
        },
        actions = {
            IconButton(onClick = onRefresh) {
                Icon(Icons.Filled.Refresh, contentDescription = stringResource(R.string.action_refresh))
            }
            ForgetMenu(onForget = onAskForget)
        },
    )
}

@Composable
private fun HostBody(
    ready: HostUiState.Ready,
    padding: PaddingValues,
    onRefresh: () -> Unit,
    onPairAgain: () -> Unit,
    onOpenRunning: (String) -> Unit,
    onToggleProject: (String) -> Unit,
    onOpenRecorded: (String, String) -> Unit,
) {
    PullToRefreshBox(
        isRefreshing = ready.refreshing,
        onRefresh = onRefresh,
        modifier = Modifier.padding(padding).fillMaxSize(),
    ) {
        LazyColumn(
            contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            val connection = ready.connection
            if (connection is ConnectionState.Refused) {
                item { RefusedBanner(reason = connection.reason, onPairAgain = onPairAgain) }
            }
            if (ready.running.isNotEmpty()) {
                item { SectionLabel(stringResource(R.string.host_section_running)) }
                items(ready.running, key = { it.session }) { chat ->
                    RunningChatRow(chat = chat, onClick = { onOpenRunning(chat.session) })
                }
            }
            item { SectionLabel(stringResource(R.string.host_section_projects)) }
            projectItems(ready, onToggleProject, onOpenRecorded)
        }
    }
}

