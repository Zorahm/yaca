package com.yaca.android.chat.panels

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.ui.theme.LocalCodeStyle
import com.yaca.core.model.ChangeKind
import com.yaca.core.model.CheckpointChange

/**
 * The confirmation of a rollback: every file it will touch and what happens to it. [files] is
 * `null` while the core is still working out the list; confirming is possible only once it has
 * answered, so a rollback is never confirmed blind.
 */
@Composable
internal fun CheckpointRestoreDialog(
    time: String,
    files: List<CheckpointChange>?,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    ConfirmDialog(
        title = stringResource(R.string.ckpt_restore_title, time),
        confirmLabel = stringResource(R.string.ckpt_restore),
        destructive = true,
        confirmEnabled = files != null,
        onConfirm = onConfirm,
        onDismiss = onDismiss,
    ) {
        Column {
            Text(
                text = stringResource(R.string.ckpt_restore_body),
                style = MaterialTheme.typography.bodyMedium,
            )
            Spacer(Modifier.height(12.dp))
            when {
                files == null -> Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(12.dp))
                    Text(
                        text = stringResource(R.string.ckpt_restore_loading),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                files.isEmpty() -> Text(
                    text = stringResource(R.string.ckpt_restore_nothing),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                else -> Column(
                    Modifier.fillMaxWidth().heightIn(max = 280.dp).verticalScroll(rememberScrollState()),
                ) {
                    for (file in files) ChangeRow(file)
                }
            }
        }
    }
}

@Composable
private fun ChangeRow(file: CheckpointChange) {
    val what = when (file.kind) {
        ChangeKind.CREATED -> R.string.ckpt_will_restore
        ChangeKind.DELETED -> R.string.ckpt_will_delete
        ChangeKind.MODIFIED, ChangeKind.MOVED -> R.string.ckpt_will_revert
    }
    Column(Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
        Text(
            text = file.path,
            style = LocalCodeStyle.current,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            text = stringResource(R.string.panel_diff_counts, file.added, file.removed) +
                " · " + stringResource(what),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
