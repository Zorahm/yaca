package com.yaca.android.chat.feed

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.ui.common.BusyDot
import com.yaca.core.model.TodoItem
import com.yaca.core.model.TodoStatus

/**
 * The plan the agent keeps for the turn: a progress bar, then the items. A finished item is struck
 * through and ticked, the one in progress pulses, the rest are hollow circles. Each mark sits in its
 * own fixed column, so a long item never runs under its icon.
 */
@Composable
fun TodosCard(items: List<TodoItem>, modifier: Modifier = Modifier) {
    val done = items.count { it.status == TodoStatus.COMPLETED }
    val fraction = if (items.isEmpty()) 0f else done.toFloat() / items.size
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surface,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
    ) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = stringResource(R.string.plan_title),
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    text = stringResource(R.string.plan_progress, done, items.size),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            LinearProgressIndicator(progress = { fraction }, modifier = Modifier.fillMaxWidth())
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                items.forEach { item -> TodoRow(item) }
            }
        }
    }
}

@Composable
private fun TodoRow(item: TodoItem) {
    val done = item.status == TodoStatus.COMPLETED
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
        Box(Modifier.size(20.dp), contentAlignment = Alignment.Center) {
            TodoMark(item.status)
        }
        Spacer(Modifier.width(8.dp))
        Text(
            text = item.content,
            style = MaterialTheme.typography.bodyMedium,
            color = if (done) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
            textDecoration = if (done) TextDecoration.LineThrough else null,
            modifier = Modifier.weight(1f),
        )
    }
}

@Composable
private fun TodoMark(status: TodoStatus) {
    when (status) {
        TodoStatus.COMPLETED -> Icon(
            imageVector = Icons.Filled.Check,
            contentDescription = stringResource(R.string.plan_item_done),
            tint = MaterialTheme.colorScheme.tertiary,
            modifier = Modifier.size(16.dp),
        )
        TodoStatus.IN_PROGRESS -> {
            val description = stringResource(R.string.plan_item_running)
            BusyDot(
                busy = true,
                modifier = Modifier.semantics { contentDescription = description },
            )
        }
        TodoStatus.PENDING -> {
            val description = stringResource(R.string.plan_item_pending)
            Box(
                Modifier
                    .size(14.dp)
                    .border(1.dp, MaterialTheme.colorScheme.outline, CircleShape)
                    .semantics { contentDescription = description },
            )
        }
    }
}
