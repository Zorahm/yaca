package com.yaca.android.chat.feed

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.chat.pluralCount
import com.yaca.android.ui.theme.LocalCodeStyle
import com.yaca.core.model.DiffLine
import com.yaca.core.model.FileDiff
import com.yaca.core.model.LineKind

/** A diff longer than this is cut in the feed; the diff panel shows all of it. */
private const val DIFF_LINES = 120

/**
 * The added and removed counters of a row: "+6" in the tertiary role, "-1" in the error role. The
 * sign is written too, so the figures do not depend on colour to be read.
 */
@Composable
fun DiffCounts(added: Int, removed: Int, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier.padding(start = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = stringResource(R.string.row_added, added),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.tertiary,
        )
        Text(
            text = stringResource(R.string.row_removed, removed),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.error,
        )
    }
}

/**
 * A file's diff in a well: hunk headers and lines, added lines washed in the tertiary container,
 * removed ones in the error container, the changed words set bold. The well scrolls sideways so a
 * long line is never cut; the lines keep their own width.
 */
@Composable
fun DiffView(diff: FileDiff, modifier: Modifier = Modifier) {
    val lines = remember(diff) { diffRowsOf(diff) }
    Column(modifier.fillMaxWidth()) {
        Column(
            Modifier
                .fillMaxWidth()
                .clip(MaterialTheme.shapes.small)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .horizontalScroll(rememberScrollState())
                .padding(vertical = 6.dp),
        ) {
            Column(Modifier.width(IntrinsicSize.Max)) {
                lines.take(DIFF_LINES).forEach { row -> DiffRowView(row) }
            }
        }
        if (lines.size > DIFF_LINES) {
            Text(
                text = pluralCount(R.plurals.lines_more, lines.size - DIFF_LINES),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
    }
}

/** One entry of the well: a hunk header, or a line. Exactly one of the two is set. */
private class DiffRowUi(val header: String?, val line: DiffLine?)

private fun diffRowsOf(diff: FileDiff): List<DiffRowUi> = diff.hunks.flatMap { hunk ->
    val header = "@@ -${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines} @@"
    listOf(DiffRowUi(header = header, line = null)) + hunk.lines.map { DiffRowUi(header = null, line = it) }
}

@Composable
private fun DiffRowView(row: DiffRowUi) {
    val header = row.header
    if (header != null) {
        Text(
            text = header,
            style = LocalCodeStyle.current,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            softWrap = false,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 2.dp),
        )
        return
    }
    val line = row.line ?: return
    val colors = MaterialTheme.colorScheme
    val (background, foreground, marker) = when (line.kind) {
        LineKind.ADD -> Triple(colors.tertiaryContainer, colors.onTertiaryContainer, "+")
        LineKind.DEL -> Triple(colors.errorContainer, colors.onErrorContainer, "-")
        LineKind.CONTEXT -> Triple(Color.Transparent, colors.onSurface, " ")
    }
    val text = changedLine(marker, line)
    Row(Modifier.background(background).padding(horizontal = 10.dp)) {
        Text(
            text = (line.newLine ?: line.oldLine)?.toString().orEmpty(),
            style = LocalCodeStyle.current,
            color = colors.onSurfaceVariant,
            textAlign = TextAlign.End,
            softWrap = false,
            modifier = Modifier.width(36.dp),
        )
        Spacer(Modifier.width(8.dp))
        Text(text = text, style = LocalCodeStyle.current, color = foreground, softWrap = false)
    }
}

/** The line with its marker in front; the changed spans are set bold, shifted past the marker. */
private fun changedLine(marker: String, line: DiffLine): AnnotatedString {
    val offset = marker.length + 1
    return buildAnnotatedString {
        append(marker)
        append(' ')
        append(line.text)
        for (span in line.spans) {
            if (span.start >= 0 && span.start < span.end && span.end <= line.text.length) {
                addStyle(SpanStyle(fontWeight = FontWeight.SemiBold), offset + span.start, offset + span.end)
            }
        }
    }
}
