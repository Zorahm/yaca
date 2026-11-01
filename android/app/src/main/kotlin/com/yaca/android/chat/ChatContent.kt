@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.chat
import androidx.compose.material3.ExperimentalMaterial3Api

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.chat.composer.Composer
import com.yaca.android.chat.feed.FeedList
import com.yaca.android.ui.common.ConnectionDot
import com.yaca.android.ui.common.connectionColor
import com.yaca.android.ui.common.connectionLabel
import com.yaca.core.client.ConnectionState

/** The overflow menu's items, as the screen answers them. */
class ChatMenuActions(
    val onProcesses: () -> Unit,
    val onCheckpoints: () -> Unit,
    val onCompact: () -> Unit,
    val onClose: () -> Unit,
)

/**
 * The chat without its state: the top bar, the feed and the composer, all drawn from [ui]. The
 * screen above it wires the view model to these callbacks.
 */
@Composable
fun ChatContent(
    ui: ChatUi,
    draft: ComposerDraft,
    listState: LazyListState,
    snackbar: SnackbarHostState,
    onBack: () -> Unit,
    feedActions: FeedActions,
    composer: ComposerActions,
    menu: ChatMenuActions,
    modifier: Modifier = Modifier,
) {
    val feed = ui.feed
    Scaffold(
        modifier = modifier,
        topBar = {
            ChatTopBar(ui = ui, waiting = openRequestOf(feed) != null, onBack = onBack, menu = menu)
        },
        bottomBar = {
            Composer(feed = feed, models = ui.models, draft = draft, actions = composer)
        },
        snackbarHost = { SnackbarHost(hostState = snackbar) },
    ) { padding ->
        FeedList(ui = ui, listState = listState, actions = feedActions, modifier = Modifier.padding(padding))
    }
}

/**
 * Back, the title, the project and the state of the chat under it, and the overflow menu. The
 * state line says the connection when the phone is not connected: a chat that is not reachable
 * must not look idle.
 */
@Composable
fun ChatTopBar(ui: ChatUi, waiting: Boolean, onBack: () -> Unit, menu: ChatMenuActions) {
    val connected = ui.connection is ConnectionState.Connected
    val working = ui.feed.state?.busy == true
    val state = when {
        !connected -> connectionLabel(ui.connection)
        waiting -> stringResource(R.string.chat_state_waiting)
        working -> stringResource(R.string.chat_state_working)
        else -> stringResource(R.string.chat_state_idle)
    }
    val line = if (ui.project.isEmpty()) state else stringResource(R.string.chat_subtitle, ui.project, state)
    var menuOpen by rememberSaveable { mutableStateOf(false) }

    TopAppBar(
        title = {
            Column {
                Text(
                    text = ui.title ?: stringResource(R.string.chat_untitled),
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    ConnectionDot(color = connectionColor(ui.connection), modifier = Modifier.padding(end = 6.dp))
                    Text(
                        text = line,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
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
            Box {
                IconButton(onClick = { menuOpen = true }) {
                    Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.action_more))
                }
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.chat_menu_processes)) },
                        onClick = {
                            menuOpen = false
                            menu.onProcesses()
                        },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.chat_menu_checkpoints)) },
                        onClick = {
                            menuOpen = false
                            menu.onCheckpoints()
                        },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.chat_menu_compact)) },
                        onClick = {
                            menuOpen = false
                            menu.onCompact()
                        },
                    )
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.chat_menu_close), color = MaterialTheme.colorScheme.error) },
                        onClick = {
                            menuOpen = false
                            menu.onClose()
                        },
                    )
                }
            }
        },
    )
}
