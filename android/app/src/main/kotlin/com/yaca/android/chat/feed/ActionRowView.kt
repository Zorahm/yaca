package com.yaca.android.chat.feed

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.yaca.android.chat.FeedActions
import com.yaca.core.feed.ActionRow
import com.yaca.core.feed.ActionState

/**
 * One action of a group. A running row opens by itself and shows an indeterminate bar under it, so
 * the command is visible while it runs; a finished row folds, and a person who opens it keeps it open
 * until its state changes.
 */
@Composable
fun ActionRowView(
    row: ActionRow,
    actions: FeedActions,
    modifier: Modifier = Modifier,
) {
    val running = row.state == ActionState.RUNNING
    var open by rememberSaveable(row.callId.value, running) { mutableStateOf(running) }
    val counts = rowCounts(row)
    Column(modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth().defaultMinSize(minHeight = 40.dp).clickable { open = !open },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ActionSign(row = row)
            Spacer(Modifier.width(8.dp))
            Text(
                text = rowCaption(row),
                style = MaterialTheme.typography.bodyMedium,
                color = captionColor(row.state),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            if (counts != null) {
                DiffCounts(added = counts.first, removed = counts.second)
            }
            Icon(
                imageVector = if (open) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(18.dp),
            )
        }
        if (running) {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
        }
        if (open) {
            ActionBody(row = row, actions = actions)
        }
    }
}

@Composable
private fun captionColor(state: ActionState): Color = when (state) {
    ActionState.FAILED -> MaterialTheme.colorScheme.error
    ActionState.REJECTED, ActionState.DENIED, ActionState.STOPPED -> MaterialTheme.colorScheme.onSurfaceVariant
    ActionState.RUNNING, ActionState.DONE -> MaterialTheme.colorScheme.onSurface
}
