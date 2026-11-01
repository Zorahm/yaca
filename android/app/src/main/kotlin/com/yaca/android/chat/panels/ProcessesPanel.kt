package com.yaca.android.chat.panels

import android.text.format.DateUtils
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Error
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.ui.common.BusyDot
import com.yaca.android.ui.common.SectionLabel
import com.yaca.android.ui.theme.LocalCodeStyle
import com.yaca.core.feed.Feed
import com.yaca.core.model.ProcessId
import com.yaca.core.model.ProcessState
import com.yaca.core.model.ProcessStateExited
import com.yaca.core.model.ProcessStateRunning
import com.yaca.core.model.ProcessStateStopped
import com.yaca.core.model.ProcessSummary
import kotlinx.coroutines.delay

/**
 * The background commands of this chat. A row opens its output; leaving the output (back, or the
 * panel closing) releases it, so the host stops streaming what nobody reads. Stopping a running
 * command asks first, in [ProcessOutputScreen].
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProcessesPanel(
    feed: Feed,
    onWatch: (ProcessId?) -> Unit,
    onStop: (ProcessId) -> Unit,
    onClose: () -> Unit,
) {
    // The row open for its output, by process number. Saved across rotation, so the output comes
    // back where it was left.
    var watched by rememberSaveable { mutableStateOf<Int?>(null) }
    val watchLatest by rememberUpdatedState(onWatch)
    DisposableEffect(watched) {
        watchLatest(watched?.let { ProcessId(it) })
        onDispose { watchLatest(null) }
    }
    BackHandler(enabled = watched != null) { watched = null }

    // The run time of the live commands counts up once a second; a finished one keeps its end.
    val running = feed.processes.any { it.state is ProcessStateRunning }
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(running) {
        while (running) {
            now = System.currentTimeMillis()
            delay(1_000L)
        }
    }

    val open = watched
    Scaffold(
        topBar = {
            if (open == null) {
                PanelTopBar(
                    title = stringResource(R.string.proc_title),
                    subtitle = null,
                    onClose = onClose,
                )
            } else {
                val process = feed.processes.firstOrNull { it.id.value == open }
                PanelTopBar(
                    title = process?.command ?: "#$open",
                    subtitle = process?.let { processStatus(it, now) },
                    onClose = { watched = null },
                )
            }
        },
    ) { padding ->
        Box(Modifier.padding(padding).fillMaxSize()) {
            if (open == null) {
                ProcessList(
                    processes = feed.processes,
                    now = now,
                    onOpen = { watched = it.value },
                )
            } else {
                ProcessOutputScreen(
                    process = feed.processes.firstOrNull { it.id.value == open },
                    id = ProcessId(open),
                    output = feed.processOutput?.takeIf { it.id.value == open },
                    onStop = onStop,
                )
            }
        }
    }
}

@Composable
private fun ProcessList(processes: List<ProcessSummary>, now: Long, onOpen: (ProcessId) -> Unit) {
    if (processes.isEmpty()) {
        PanelMessage(stringResource(R.string.proc_empty))
        return
    }
    val live = processes.filter { it.state is ProcessStateRunning }
    val ended = processes.filterNot { it.state is ProcessStateRunning }
    LazyColumn(contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)) {
        if (live.isNotEmpty()) {
            item { SectionLabel(stringResource(R.string.proc_section_running)) }
            items(live, key = { it.id.value }) { process -> ProcessRow(process, now, onOpen) }
        }
        if (ended.isNotEmpty()) {
            item { SectionLabel(stringResource(R.string.proc_section_finished)) }
            items(ended, key = { it.id.value }) { process -> ProcessRow(process, now, onOpen) }
        }
    }
}

@Composable
private fun ProcessRow(process: ProcessSummary, now: Long, onOpen: (ProcessId) -> Unit) {
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier
                .fillMaxWidth()
                .clickable { onOpen(process.id) }
                .padding(vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ProcessStatusMark(process.state)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    text = process.command,
                    style = LocalCodeStyle.current,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = processStatus(process, now),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        HorizontalDivider()
    }
}

/**
 * What the state is drawn as: a pulsing dot while running, a tick for exit 0, an error mark for
 * any other exit, a plain stop for a stopped command. Each mark comes with its words below, so the
 * colour is never the only carrier.
 */
@Composable
private fun ProcessStatusMark(state: ProcessState) {
    val colors = MaterialTheme.colorScheme
    Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
        when (state) {
            is ProcessStateRunning -> BusyDot(busy = true)
            is ProcessStateExited -> if (state.code == 0) {
                Icon(Icons.Filled.Check, contentDescription = null, tint = colors.tertiary)
            } else {
                Icon(Icons.Filled.Error, contentDescription = null, tint = colors.error)
            }
            is ProcessStateStopped -> Icon(Icons.Filled.Stop, contentDescription = null, tint = colors.onSurfaceVariant)
        }
    }
}

/** "running · 04:12", "exit code 1 · 00:03": the state and the run time in one line. */
@Composable
private fun processStatus(process: ProcessSummary, now: Long): String {
    val state = when (val current = process.state) {
        is ProcessStateRunning -> stringResource(R.string.proc_state_running)
        is ProcessStateStopped -> stringResource(R.string.proc_state_stopped)
        is ProcessStateExited -> current.code?.let { stringResource(R.string.proc_state_exited, it) }
            ?: stringResource(R.string.proc_state_exited_none)
    }
    val end = process.finishedAt?.value ?: now
    val seconds = ((end - process.startedAt.value) / 1_000L).coerceAtLeast(0L)
    return state + " · " + DateUtils.formatElapsedTime(seconds)
}
