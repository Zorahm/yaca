package com.yaca.android.chat.feed

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.size
import com.yaca.android.R
import com.yaca.android.chat.formatClock
import com.yaca.android.chat.pluralCount
import com.yaca.core.feed.FeedBlock
import com.yaca.core.model.CheckpointReasonCommand
import com.yaca.core.model.CheckpointReasonMessage
import com.yaca.core.model.CheckpointReasonRollback
import com.yaca.core.model.CheckpointSummary

@Composable
fun CheckpointCreatingLine(modifier: Modifier = Modifier) {
    Row(modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
        CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
        Text(stringResource(R.string.checkpoint_creating), modifier = Modifier.padding(start = 8.dp), style = MaterialTheme.typography.labelMedium)
    }
}

/**
 * A checkpoint as a divider line: when it was taken and why. Tapping it opens the checkpoints panel,
 * where a rollback is previewed and asked for; the divider itself changes nothing.
 */
@Composable
fun CheckpointDivider(
    checkpoint: CheckpointSummary,
    onOpen: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val reason = when (val why = checkpoint.reason) {
        is CheckpointReasonMessage -> stringResource(R.string.checkpoint_before_message)
        is CheckpointReasonCommand -> stringResource(R.string.checkpoint_before_command, why.command)
        is CheckpointReasonRollback -> stringResource(R.string.checkpoint_after_restore)
    }
    Row(
        modifier.fillMaxWidth().defaultMinSize(minHeight = 40.dp).clickable(onClick = onOpen),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        HorizontalDivider(modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.outlineVariant)
        Text(
            text = stringResource(R.string.checkpoint_line, formatClock(checkpoint.at.value), "${stringResource(R.string.checkpoint_created)} · $reason"),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 8.dp),
        )
        HorizontalDivider(modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.outlineVariant)
    }
}

/** A rollback that was done: the divider and how many files came back. */
@Composable
fun RestoredDivider(block: FeedBlock.Restored, modifier: Modifier = Modifier) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        HorizontalDivider(modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.outlineVariant)
        Text(
            text = stringResource(R.string.restored_line, pluralCount(R.plurals.files_count, block.files)),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 8.dp),
        )
        HorizontalDivider(modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.outlineVariant)
    }
}
