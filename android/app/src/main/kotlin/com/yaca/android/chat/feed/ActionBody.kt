package com.yaca.android.chat.feed

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.chat.FeedActions
import com.yaca.android.chat.formatDuration
import com.yaca.android.chat.markdown.CodeBlock
import com.yaca.android.chat.pluralCount
import com.yaca.core.feed.ActionRow
import com.yaca.core.feed.ActionState
import com.yaca.android.ui.theme.LocalCodeStyle

/** Output longer than this is cut in the feed; the rest is read in the file or the diff panel. */
private const val OUTPUT_LINES = 60

/**
 * What an opened row shows, indented under its caption: the target (the command in a box, the path
 * for a file), the nested rows of a rename or a subagent, the diff or the output, the reason it
 * failed, and the actions that lead further (the diff, the file).
 */
@Composable
fun ActionBody(
    row: ActionRow,
    actions: FeedActions,
    modifier: Modifier = Modifier,
) {
    val meta = row.meta
    Column(
        modifier.fillMaxWidth().padding(start = 28.dp, end = 4.dp, bottom = 8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        val target = row.target
        if (target != null) {
            if (isFileRow(row)) {
                Text(
                    text = target,
                    style = LocalCodeStyle.current,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                // A command is shown as the prompt it ran at: the box is what was run, not what it printed.
                CodeBlock(text = if (row.tool == "Bash") "\$ $target" else target)
            }
        }
        if (row.children.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                row.children.forEach { child -> ActionRowView(row = child, actions = actions) }
            }
        }

        val diff = row.diff
        val output = row.output.trimEnd()
        when {
            diff != null -> DiffView(diff = diff)
            output.isNotBlank() -> OutputBlock(output = output, failed = row.state == ActionState.FAILED)
            row.state == ActionState.DONE -> Text(
                text = stringResource(R.string.row_no_output),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (meta?.truncated == true) {
            Text(
                text = stringResource(R.string.row_output_cut),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (row.state == ActionState.FAILED) {
            row.message?.let { reason ->
                Text(text = reason, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
            }
        }
        val pictures = meta?.pictures.orEmpty()
        if (pictures.isNotEmpty()) {
            // Images stay on the computer: the phone lists their paths, so the feed does not hide them.
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    text = stringResource(R.string.row_pictures),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                pictures.forEach { path ->
                    Text(text = path, style = LocalCodeStyle.current, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            val fact = row.fact
            if (fact != null) {
                TextButton(onClick = { actions.onOpenDiff(fact.path) }) {
                    Text(stringResource(R.string.row_open_diff))
                }
            } else if (row.tool == "Read" && row.state == ActionState.DONE && target != null) {
                TextButton(onClick = { actions.onOpenFile(target) }) {
                    Text(stringResource(R.string.row_open_file))
                }
            }
            if (meta != null) {
                Text(
                    text = formatDuration(meta.durationMs.toLong()),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 8.dp),
                )
            }
        }
    }
}

@Composable
private fun OutputBlock(output: String, failed: Boolean) {
    val lines = output.lines()
    val shown = if (lines.size > OUTPUT_LINES) lines.take(OUTPUT_LINES).joinToString("\n") else output
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        CodeBlock(text = shown, error = failed)
        if (lines.size > OUTPUT_LINES) {
            Text(
                text = pluralCount(R.plurals.lines_more, lines.size - OUTPUT_LINES),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
