package com.yaca.android.chat.panels

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.History
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.core.feed.Feed
import com.yaca.core.feed.diffOf
import com.yaca.core.feed.factOf
import com.yaca.core.model.ChangeKind
import com.yaca.core.model.FileDiff
import com.yaca.core.model.FileFact

/**
 * The file's diff in this chat: the hunks the core computed, with both line numbers and the
 * word-level changes. The top bar's history button goes to the checkpoints, where the same file
 * can be taken back to an earlier state.
 *
 * The panel has no "whole file" tab. Showing the file as it now stands needs a request to the core
 * for it, and this panel has no callback to make that request; [FilePanel] does it, and the
 * host opens it for the file.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DiffPanel(feed: Feed, path: String, onOpenCheckpoints: () -> Unit, onClose: () -> Unit) {
    val diff = diffOf(feed, path)
    val fact = factOf(feed, path)
    val added = diff?.added ?: fact?.added ?: 0
    val removed = diff?.removed ?: fact?.removed ?: 0
    val subtitle = diffSubtitle(fact, added, removed)

    Scaffold(
        topBar = {
            PanelTopBar(
                title = path,
                subtitle = subtitle,
                onClose = onClose,
                actions = {
                    IconButton(onClick = onOpenCheckpoints) {
                        Icon(Icons.Filled.History, contentDescription = stringResource(R.string.ckpt_title))
                    }
                },
            )
        },
    ) { padding ->
        Box(Modifier.padding(padding).fillMaxSize()) {
            when {
                diff == null -> PanelMessage(stringResource(R.string.diff_none))
                diff.hunks.isEmpty() -> PanelMessage(stringResource(R.string.diff_no_lines))
                else -> DiffBody(diff)
            }
        }
    }
}

/** What kind of change it was, and how many lines it added and removed: the panel's second line. */
@Composable
private fun diffSubtitle(fact: FileFact?, added: Int, removed: Int): String {
    val kind = when (fact?.kind) {
        null -> null
        ChangeKind.CREATED -> stringResource(R.string.change_created)
        ChangeKind.MODIFIED -> stringResource(R.string.change_modified)
        ChangeKind.DELETED -> stringResource(R.string.change_deleted)
        ChangeKind.MOVED -> stringResource(R.string.change_moved, fact?.movedFrom.orEmpty())
    }
    val counts = stringResource(R.string.panel_diff_counts, added, removed)
    return listOfNotNull(kind, counts).joinToString(" · ")
}

@Composable
private fun DiffBody(diff: FileDiff) {
    val items = remember(diff) { diffItems(diff) }
    val digits = remember(diff) { digitsOf(diff) }
    val longest = remember(items) { items.maxOf { it.chars } }
    // The gutter is as wide as the longest number and no wider: a column sized for five digits
    // would push a ten-line diff's text away from its edge.
    val numberWidth = codeWidth(digits)
    val gutter = numberWidth + 6.dp
    val contentWidth = gutter + gutter + codeWidth(longest + 2) + 16.dp

    CodeList(
        contentWidth = contentWidth,
        itemCount = items.size,
        note = if (diff.truncated) stringResource(R.string.diff_truncated) else null,
    ) { index ->
        when (val item = items[index]) {
            is DiffItem.Header -> HunkHeaderRow(item.hunk)
            is DiffItem.Code -> DiffLineRow(item.line, numberWidth)
        }
    }
}
