package com.yaca.android.chat.feed

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
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
import com.yaca.android.chat.FeedActions
import com.yaca.android.ui.common.BusyDot
import com.yaca.core.feed.FeedItem
import com.yaca.core.feed.workRows

/**
 * A stretch of the agent's calls and thoughts. Open while it runs, so the live row is in view;
 * folded once the turn is done, under a single line that says what it did. The fold state is keyed
 * on [live], so the group opens or folds by itself when its state changes, and a person's own tap
 * holds until then.
 */
@Composable
fun WorkGroup(
    work: FeedItem.Work,
    live: Boolean,
    actions: FeedActions,
    modifier: Modifier = Modifier,
) {
    val summary = summarizeRows(workRows(work.steps))
    var expanded by rememberSaveable(work.key, live) { mutableStateOf(live) }
    Column(modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth().defaultMinSize(minHeight = 40.dp).clickable { expanded = !expanded },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            GroupSign(live = live, failed = summary.failed > 0)
            Spacer(Modifier.width(8.dp))
            Text(
                text = groupHeadline(summary),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            if (summary.added > 0 || summary.removed > 0) {
                DiffCounts(added = summary.added, removed = summary.removed)
            }
            Icon(
                imageVector = if (expanded) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(18.dp),
            )
        }
        if (expanded) {
            Column(
                Modifier.fillMaxWidth().padding(start = 28.dp, bottom = 4.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                work.steps.forEach { step -> WorkStepView(step = step, actions = actions) }
            }
        }
    }
}

/** A step of the group, in the order it happened: a thought, a call, or a checkpoint taken mid-turn. */
@Composable
private fun WorkStepView(step: FeedItem.WorkStep, actions: FeedActions) {
    when (step) {
        is FeedItem.WorkStep.Thought -> ReasoningBlock(thought = step)
        is FeedItem.WorkStep.Action -> ActionRowView(row = step.row, actions = actions)
        is FeedItem.WorkStep.Checkpoint -> CheckpointDivider(
            checkpoint = step.block.checkpoint,
            onOpen = actions.onOpenCheckpoints,
        )
    }
}

/** The group's mark: a pulse while the turn runs, a cross when something in it failed, a tick when all is done. */
@Composable
private fun GroupSign(live: Boolean, failed: Boolean) {
    Box(Modifier.size(20.dp), contentAlignment = Alignment.Center) {
        when {
            live -> BusyDot(busy = true)
            failed -> Icon(
                imageVector = Icons.Filled.Close,
                contentDescription = stringResource(R.string.row_status_failed),
                tint = MaterialTheme.colorScheme.error,
                modifier = Modifier.size(16.dp),
            )
            else -> Icon(
                imageVector = Icons.Filled.Check,
                contentDescription = stringResource(R.string.row_status_done),
                tint = MaterialTheme.colorScheme.tertiary,
                modifier = Modifier.size(16.dp),
            )
        }
    }
}
