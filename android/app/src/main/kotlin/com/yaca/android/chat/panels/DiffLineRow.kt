package com.yaca.android.chat.panels

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.yaca.android.ui.theme.LocalCodeStyle
import com.yaca.core.model.DiffLine
import com.yaca.core.model.FileDiff
import com.yaca.core.model.Hunk
import com.yaca.core.model.LineKind
import com.yaca.core.model.Span

/**
 * One row of a diff as the panel draws it: a hunk header, or a line. Flat, so that the list can
 * be virtualised by index.
 */
internal sealed interface DiffItem {
    /** Characters the row needs on one line, for the width of the list. */
    val chars: Int

    class Header(val hunk: Hunk) : DiffItem {
        override val chars: Int = hunkHeader(hunk).length
    }

    class Code(val line: DiffLine) : DiffItem {
        override val chars: Int = line.text.length
    }
}

/** The rows of every hunk, in order. A header comes before the lines it introduces. */
internal fun diffItems(diff: FileDiff): List<DiffItem> = buildList {
    for (hunk in diff.hunks) {
        add(DiffItem.Header(hunk))
        for (line in hunk.lines) add(DiffItem.Code(line))
    }
}

/** The largest line number of a diff: its gutter is as wide as this number and no wider. */
internal fun digitsOf(diff: FileDiff): Int {
    val top = diff.hunks.maxOfOrNull { hunk ->
        hunk.lines.maxOfOrNull { maxOf(it.oldLine ?: 0, it.newLine ?: 0) } ?: 0
    } ?: 0
    return maxOf(1, top.toString().length)
}

/**
 * The hunk's range header, as git writes it. It is code, so it is not translated, and it is
 * drawn as the core computed it.
 */
internal fun hunkHeader(hunk: Hunk): String =
    "@@ -${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines} @@"

/**
 * A hunk header row: the range, on the surface-variant wash so it reads as a divider between
 * the hunks.
 */
@Composable
internal fun HunkHeaderRow(hunk: Hunk) {
    Row(Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surfaceVariant)) {
        Text(
            text = hunkHeader(hunk),
            style = LocalCodeStyle.current,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            softWrap = false,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
        )
    }
}

/**
 * One diff line: the old and new numbers (each only on the side the line belongs to), the sign,
 * and the text. Added lines are tinted tertiary-container and removed lines error-container: the
 * sign carries the same meaning, so the tint is never the only carrier.
 */
@Composable
internal fun DiffLineRow(line: DiffLine, numberWidth: Dp) {
    val colors = MaterialTheme.colorScheme
    val (background, ink, sign) = when (line.kind) {
        LineKind.ADD -> Triple(colors.tertiaryContainer, colors.onTertiaryContainer, "+")
        LineKind.DEL -> Triple(colors.errorContainer, colors.onErrorContainer, "−")
        LineKind.CONTEXT -> Triple(Color.Transparent, colors.onSurface, " ")
    }
    val oldNumber = if (line.kind == LineKind.ADD) null else line.oldLine
    val newNumber = if (line.kind == LineKind.DEL) null else line.newLine

    Row(Modifier.fillMaxWidth().background(background)) {
        LineNumber(oldNumber, numberWidth)
        LineNumber(newNumber, numberWidth)
        Text(
            text = sign,
            style = LocalCodeStyle.current,
            color = ink,
            modifier = Modifier.padding(horizontal = 4.dp),
        )
        Text(
            text = wordAnnotated(line.text, line.spans),
            style = LocalCodeStyle.current,
            color = ink,
            softWrap = false,
        )
    }
}

/** A line number in a gutter column, right-aligned; an empty column when the side has no such line. */
@Composable
internal fun LineNumber(number: Int?, width: Dp) {
    Text(
        text = number?.toString().orEmpty(),
        style = LocalCodeStyle.current,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.End,
        softWrap = false,
        modifier = Modifier.width(width).padding(end = 6.dp),
    )
}

/**
 * The line's text with the word-level changes in bold. The core counts a span's bounds in code
 * points, and a Kotlin string indexes UTF-16 units: a surrogate pair would cut a span in two.
 */
internal fun wordAnnotated(text: String, spans: List<Span>): AnnotatedString = buildAnnotatedString {
    append(text)
    for (span in spans) {
        val from = utf16Index(text, span.start)
        val to = utf16Index(text, span.end)
        if (to > from) addStyle(SpanStyle(fontWeight = FontWeight.Bold), from, to)
    }
}

/** The UTF-16 index of the [codePoint]-th code point of [text], clamped to the end of the text. */
internal fun utf16Index(text: String, codePoint: Int): Int {
    val points = text.codePointCount(0, text.length)
    return if (codePoint >= points) text.length else text.offsetByCodePoints(0, codePoint.coerceAtLeast(0))
}
