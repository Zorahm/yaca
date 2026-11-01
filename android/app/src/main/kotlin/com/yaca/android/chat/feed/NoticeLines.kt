package com.yaca.android.chat.feed

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.chat.formatDuration
import com.yaca.core.feed.FeedBlock

// The feed's single lines: what the model answered with, a retry, a stop, a plugin's word, and how
// long the turn took. Each is one line of muted text, so the conversation reads at a glance.

/** A retry the core scheduled, with the wait before it. */
@Composable
fun RetryLine(block: FeedBlock.Retry, modifier: Modifier = Modifier) {
    Text(
        text = stringResource(
            R.string.retry_line,
            block.attempt,
            block.of,
            formatDuration(block.delayMs.toLong()),
        ),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier,
    )
}

/** Which model answers the turn. The default route has no name, so it is shown by its provider and model alone. */
@Composable
fun RouteLine(block: FeedBlock.Route, modifier: Modifier = Modifier) {
    val route = block.route
    val text = if (route != null) {
        stringResource(R.string.route_named, route, block.provider, block.model)
    } else {
        stringResource(R.string.route_default, block.provider, block.model)
    }
    Text(
        text = text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier,
    )
}

/** The human stopped the turn: a stop mark and a word, nothing more. */
@Composable
fun StoppedLine(modifier: Modifier = Modifier) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Icon(
            imageVector = Icons.Filled.Stop,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(14.dp),
        )
        Spacer(Modifier.width(6.dp))
        Text(
            text = stringResource(R.string.stopped_line),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** What a plugin hook said. A hook that blocked the message says so in the error role, with its text under it. */
@Composable
fun HookLine(block: FeedBlock.Hook, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(
            text = stringResource(R.string.hook_line, block.plugin, block.event),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (block.blocked) {
            Text(
                text = stringResource(R.string.hook_blocked),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.error,
            )
        }
        if (block.text.isNotBlank()) {
            Text(
                text = block.text,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** How long the turn ran, from the core's own clock. */
@Composable
fun TurnTimeLine(block: FeedBlock.TurnTime, modifier: Modifier = Modifier) {
    Text(
        text = stringResource(R.string.turn_time, formatDuration(block.endedAt.value - block.startedAt.value)),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier,
    )
}
