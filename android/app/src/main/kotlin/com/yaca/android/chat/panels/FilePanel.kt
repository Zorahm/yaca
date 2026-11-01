package com.yaca.android.chat.panels

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.ui.theme.LocalCodeStyle
import com.yaca.core.feed.Feed
import com.yaca.core.model.ChangeKind
import com.yaca.core.model.DiffLine
import com.yaca.core.model.FileView
import com.yaca.core.model.LineKind
import com.yaca.core.model.ViewSegmentHunk
import com.yaca.core.model.ViewSegmentUnchanged

/**
 * The file as it lies now, whole, with line numbers: a viewer, not a diff. Removed lines are
 * dropped and nothing is marked, so what is shown is the file the agent left behind.
 *
 * The file is asked for when the panel opens on it, and again whenever the agent writes any file:
 * the view has to follow the file, and a write is the only thing that changes it. Until the answer
 * for this path arrives the panel says it is reading; a view the core could not give is said to be
 * unavailable. The feed's file slot carries no error of its own, so there is no error text to show.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FilePanel(feed: Feed, path: String, onRequest: (String) -> Unit, onClose: () -> Unit) {
    val requestLatest by rememberUpdatedState(onRequest)
    LaunchedEffect(path, feed.writes) { requestLatest(path) }

    val slot = feed.fileView?.takeIf { it.path == path }
    val view = slot?.view
    val lines = remember(view) { view?.let { currentLines(it) }.orEmpty() }

    Scaffold(topBar = { PanelTopBar(title = path, subtitle = null, onClose = onClose) }) { padding ->
        Box(Modifier.padding(padding).fillMaxSize()) {
            when {
                slot == null -> PanelMessage(stringResource(R.string.file_loading))
                view == null -> PanelMessage(stringResource(R.string.file_unavailable))
                lines.isEmpty() -> PanelMessage(
                    stringResource(
                        if (view.fact?.kind == ChangeKind.DELETED) R.string.panel_file_deleted else R.string.file_empty,
                    ),
                )
                else -> FileBody(lines, truncated = view.truncated)
            }
        }
    }
}

/** The file as it is now: every line of the new side, none of them marked; removed lines are gone. */
private fun currentLines(view: FileView): List<DiffLine> =
    view.segments
        .flatMap { segment ->
            when (segment) {
                is ViewSegmentHunk -> segment.hunk.lines
                is ViewSegmentUnchanged -> segment.lines
            }
        }
        .filter { it.kind != LineKind.DEL }

@Composable
private fun FileBody(lines: List<DiffLine>, truncated: Boolean) {
    val digits = remember(lines) {
        maxOf(1, (lines.maxOfOrNull { it.newLine ?: 0 } ?: 0).toString().length)
    }
    val longest = remember(lines) { lines.maxOf { it.text.length } }
    val numberWidth = codeWidth(digits)
    val contentWidth = numberWidth + 6.dp + codeWidth(longest) + 16.dp

    CodeList(
        contentWidth = contentWidth,
        itemCount = lines.size,
        note = if (truncated) stringResource(R.string.file_truncated) else null,
    ) { index ->
        val line = lines[index]
        FileLine(number = line.newLine ?: (index + 1), text = line.text, numberWidth = numberWidth)
    }
}

@Composable
private fun FileLine(number: Int, text: String, numberWidth: Dp) {
    Row(Modifier.fillMaxWidth()) {
        LineNumber(number, numberWidth)
        Text(
            text = text,
            style = LocalCodeStyle.current,
            color = MaterialTheme.colorScheme.onSurface,
            softWrap = false,
            modifier = Modifier.padding(start = 4.dp),
        )
    }
}
