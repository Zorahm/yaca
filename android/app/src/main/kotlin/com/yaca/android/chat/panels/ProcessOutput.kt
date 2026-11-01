package com.yaca.android.chat.panels

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.ui.common.CenteredProgress
import com.yaca.android.ui.theme.LocalCodeStyle
import com.yaca.core.feed.ProcessOutputBuffer
import com.yaca.core.model.ProcessId
import com.yaca.core.model.ProcessStateRunning
import com.yaca.core.model.ProcessSummary

/** How close to the end the reader must be for new output to carry the view down with it. */
private const val TAIL_SLACK_PX = 48

/**
 * The output of one command: the buffer as the core sent it, followed to its end while the reader
 * stays at the bottom. A running command can be stopped; the stop goes out only after the
 * confirmation, and the dialog says the command gets a stop signal and its output is kept.
 *
 * [output] is `null` until the buffer for this command arrives: a spinner, not an empty output.
 */
@Composable
internal fun ProcessOutputScreen(
    process: ProcessSummary?,
    id: ProcessId,
    output: ProcessOutputBuffer?,
    onStop: (ProcessId) -> Unit,
) {
    var asking by rememberSaveable { mutableStateOf(false) }
    val running = process?.state is ProcessStateRunning
    // The buffer's count is the authority once it has arrived; until then the summary's count is
    // the same fact, so the truncation note never waits for the output.
    val dropped = output?.dropped ?: process?.dropped ?: 0L

    Column(Modifier.fillMaxSize()) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(R.string.proc_output),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            if (dropped > 0L) {
                val context = LocalContext.current
                val count = dropped.toInt()
                Text(
                    text = context.resources.getQuantityString(R.plurals.proc_dropped, count, count),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.width(8.dp))
            }
            if (running) {
                OutlinedButton(
                    onClick = { asking = true },
                    colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error),
                ) {
                    Icon(Icons.Filled.Stop, contentDescription = null, modifier = Modifier.size(16.dp))
                    Spacer(Modifier.width(6.dp))
                    Text(stringResource(R.string.proc_stop))
                }
            }
        }

        if (output == null) {
            CenteredProgress()
        } else {
            OutputText(output.text)
        }
    }

    if (asking && process != null) {
        ConfirmDialog(
            title = stringResource(R.string.proc_stop_title, process.command),
            confirmLabel = stringResource(R.string.proc_stop),
            destructive = true,
            onConfirm = {
                asking = false
                onStop(id)
            },
            onDismiss = { asking = false },
        ) {
            Text(
                text = stringResource(R.string.proc_stop_body),
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

/**
 * The output on a sunk well, in monospace. Line wrapping stays on: a terminal's long line is read
 * on a phone by wrapping it, not by scrolling sideways.
 */
@Composable
private fun OutputText(text: String) {
    val scroll = rememberScrollState()
    // The reader's place is followed by the position, not by the content: a reader who scrolled up
    // keeps the place, and one at the bottom is carried down as the output grows.
    var follow by remember { mutableStateOf(true) }
    LaunchedEffect(scroll) {
        snapshotFlow { scroll.value }.collect { follow = it >= scroll.maxValue - TAIL_SLACK_PX }
    }
    LaunchedEffect(scroll) {
        snapshotFlow { scroll.maxValue }.collect { if (follow) scroll.scrollTo(it) }
    }

    Surface(
        modifier = Modifier.fillMaxSize().padding(start = 16.dp, end = 16.dp, bottom = 16.dp),
        shape = MaterialTheme.shapes.small,
        color = MaterialTheme.colorScheme.surfaceVariant,
    ) {
        Column(Modifier.fillMaxSize().verticalScroll(scroll).padding(12.dp)) {
            if (text.isEmpty()) {
                Text(
                    text = stringResource(R.string.proc_no_output),
                    style = LocalCodeStyle.current,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                Text(
                    text = text,
                    style = LocalCodeStyle.current,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
    }
}
