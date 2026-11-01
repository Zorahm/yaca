package com.yaca.android.chat.panels

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.RadioButtonChecked
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.core.feed.CheckpointPreview
import com.yaca.core.feed.Feed
import com.yaca.core.model.CheckpointId
import com.yaca.core.model.CheckpointReason
import com.yaca.core.model.CheckpointReasonCommand
import com.yaca.core.model.CheckpointReasonMessage
import com.yaca.core.model.CheckpointReasonRollback
import com.yaca.core.model.CheckpointSummary

/**
 * The checkpoints of this chat, newest first, under a line that says what the working tree is now.
 *
 * Restore does not happen on a tap. The first tap asks the core what the rollback would change
 * ([onPreview]) and opens the confirmation; the dialog waits for the preview of exactly this
 * checkpoint, and only the confirm button calls [onRestore]. A rollback cannot be undone, and the
 * dialog says so.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CheckpointsPanel(
    feed: Feed,
    onPreview: (CheckpointId) -> Unit,
    onRestore: (CheckpointId) -> Unit,
    onClose: () -> Unit,
) {
    // The checkpoint whose restore is being confirmed, by its id's text. It is saved: the question
    // is still open after a rotation, and the dialog should come back.
    var asking by rememberSaveable { mutableStateOf<String?>(null) }
    // The preview the feed held when the question went out. A preview is the answer to this
    // question only if it is a new one: an answer left over from an earlier question about the
    // same checkpoint is not.
    var before by remember { mutableStateOf<CheckpointPreview?>(null) }
    val preview = feed.preview?.takeIf { it.id.value == asking && it !== before }
    val target = feed.checkpoints.firstOrNull { it.id.value == asking }

    Scaffold(
        topBar = {
            PanelTopBar(
                title = stringResource(R.string.ckpt_title),
                subtitle = stringResource(R.string.ckpt_subtitle),
                onClose = onClose,
            )
        },
    ) { padding ->
        Box(Modifier.padding(padding).fillMaxSize()) {
            // Newest first: the core keeps them oldest first.
            val points = feed.checkpoints.asReversed()
            if (points.isEmpty()) {
                PanelMessage(stringResource(R.string.ckpt_empty))
            } else {
                LazyColumn(contentPadding = PaddingValues(vertical = 8.dp)) {
                    item { CurrentStateRow() }
                    items(points, key = { it.id.value }) { point ->
                        CheckpointRow(
                            point = point,
                            onAsk = {
                                before = feed.preview
                                asking = point.id.value
                                onPreview(point.id)
                            },
                        )
                    }
                }
            }
        }
    }

    val chosen = target
    if (asking != null && chosen != null) {
        CheckpointRestoreDialog(
            time = formatClock(chosen.at),
            files = preview?.files,
            onConfirm = {
                asking = null
                before = null
                onRestore(chosen.id)
            },
            onDismiss = {
                asking = null
                before = null
            },
        )
    }
}

/** The working tree as it is now: the line the checkpoints are measured against. It has no restore button. */
@Composable
private fun CurrentStateRow() {
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = Icons.Filled.RadioButtonChecked,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
            )
            Spacer(Modifier.width(12.dp))
            Text(
                text = stringResource(R.string.ckpt_now),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f),
            )
            Surface(
                shape = MaterialTheme.shapes.small,
                color = MaterialTheme.colorScheme.primaryContainer,
            ) {
                Text(
                    text = stringResource(R.string.ckpt_now_mark),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
                )
            }
        }
        HorizontalDivider()
    }
}

@Composable
private fun CheckpointRow(point: CheckpointSummary, onAsk: () -> Unit) {
    val context = LocalContext.current
    val tracked = context.resources.getQuantityString(R.plurals.ckpt_tracked, point.tracked, point.tracked)
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    text = reasonText(point.reason),
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = "${formatClock(point.at)} · $tracked",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            OutlinedButton(onClick = onAsk) {
                Text(stringResource(R.string.ckpt_restore))
            }
        }
        HorizontalDivider()
    }
}

@Composable
private fun reasonText(reason: CheckpointReason): String = when (reason) {
    is CheckpointReasonMessage -> stringResource(R.string.ckpt_reason_message)
    is CheckpointReasonCommand -> stringResource(R.string.ckpt_reason_command, reason.command)
    is CheckpointReasonRollback -> stringResource(R.string.ckpt_reason_rollback)
}
