package com.yaca.android.chat.feed

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
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
import com.yaca.android.chat.formatTokens
import com.yaca.android.chat.markdown.Markdown
import com.yaca.android.chat.pluralCount
import com.yaca.core.feed.ActionRow
import com.yaca.core.feed.ActionState
import com.yaca.core.feed.FeedItem
import com.yaca.core.feed.agentPrompt
import com.yaca.core.feed.agentTokens
import com.yaca.core.feed.tally

/**
 * Subagents sent one after another, as a card: how many, how many still run, what they have spent.
 * Each one opens onto its report and the calls it made. The desktop plate stands in the feed the
 * same way; here the list is the card, so a running subagent is visible without a fold.
 */
@Composable
fun AgentsGroup(
    agents: FeedItem.Agents,
    actions: FeedActions,
    modifier: Modifier = Modifier,
) {
    val totals = tally(agents.rows)
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surface,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
    ) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = pluralCount(R.plurals.agents_title, totals.count),
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f),
                )
                if (totals.running > 0) {
                    Text(
                        text = pluralCount(R.plurals.agents_running, totals.running),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
            }
            val spent = totals.tokens
            if (totals.failed > 0 || spent != null) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (totals.failed > 0) {
                        Text(
                            text = pluralCount(R.plurals.work_failed, totals.failed),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                    if (totals.failed > 0 && spent != null) Spacer(Modifier.width(8.dp))
                    if (spent != null) {
                        Text(
                            text = stringResource(R.string.agents_tokens, formatTokens(spent)),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
            agents.rows.forEach { row -> AgentRowView(row = row, actions = actions) }
        }
    }
}

@Composable
private fun AgentRowView(row: ActionRow, actions: FeedActions) {
    var open by rememberSaveable(row.callId.value) { mutableStateOf(false) }
    val prompt = agentPrompt(row).lineSequence().firstOrNull { it.isNotBlank() }
        ?: row.description
        ?: stringResource(R.string.agents_untitled)
    val spent = agentTokens(row)
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth().defaultMinSize(minHeight = 40.dp).clickable { open = !open },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ActionSign(row = row)
            Spacer(Modifier.width(8.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    text = prompt,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                if (spent != null) {
                    Text(
                        text = stringResource(R.string.agents_tokens, formatTokens(spent)),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
        if (open) {
            Column(
                Modifier.fillMaxWidth().padding(start = 28.dp, bottom = 8.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                val report = row.output.trim()
                when {
                    report.isNotEmpty() -> Markdown(text = report)
                    row.state != ActionState.RUNNING -> Text(
                        text = stringResource(R.string.agents_no_report),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                row.message?.let { reason ->
                    Text(text = reason, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
                }
                row.children.forEach { child -> ActionRowView(row = child, actions = actions) }
            }
        }
    }
}
