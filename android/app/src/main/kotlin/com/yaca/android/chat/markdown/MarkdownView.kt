package com.yaca.android.chat.markdown

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/** The colours a Markdown message is drawn in, all read from the theme's roles. */
private class MarkdownPalette(
    val text: Color,
    val muted: Color,
    val codeWell: Color,
    val link: Color,
    val rule: Color,
)

/**
 * An agent's message as Markdown: paragraphs, headings, lists, quotes, rules, fenced code and inline
 * marks. Nothing is fetched or run; a link opens in the browser when it is tapped.
 */
@Composable
fun Markdown(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.onSurface,
) {
    val blocks = remember(text) { parseMarkdown(text) }
    val palette = MarkdownPalette(
        text = color,
        muted = MaterialTheme.colorScheme.onSurfaceVariant,
        codeWell = MaterialTheme.colorScheme.surfaceVariant,
        link = MaterialTheme.colorScheme.primary,
        rule = MaterialTheme.colorScheme.outlineVariant,
    )
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        blocks.forEach { block -> MarkdownBlockView(block = block, palette = palette) }
    }
}

@Composable
private fun MarkdownBlockView(block: MdBlock, palette: MarkdownPalette) {
    when (block) {
        is MdBlock.Paragraph -> Text(
            text = inlineAnnotated(block.text, palette.codeWell, palette.link),
            style = MaterialTheme.typography.bodyMedium,
            color = palette.text,
        )
        is MdBlock.Heading -> Text(
            text = inlineAnnotated(block.text, palette.codeWell, palette.link),
            style = if (block.level <= 2) MaterialTheme.typography.titleMedium else MaterialTheme.typography.titleSmall,
            color = palette.text,
        )
        is MdBlock.Item -> Row(
            Modifier.fillMaxWidth().padding(start = (block.depth * 16).dp),
        ) {
            Text(
                text = if (block.ordered) "${block.number}." else "•",
                style = MaterialTheme.typography.bodyMedium,
                color = palette.muted,
                modifier = Modifier.width(22.dp),
            )
            Text(
                text = inlineAnnotated(block.text, palette.codeWell, palette.link),
                style = MaterialTheme.typography.bodyMedium,
                color = palette.text,
                modifier = Modifier.weight(1f),
            )
        }
        is MdBlock.Code -> CodeBlock(text = block.text)
        is MdBlock.Quote -> Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min)) {
            Box(Modifier.width(3.dp).fillMaxHeight().background(palette.rule))
            Spacer(Modifier.width(10.dp))
            Text(
                text = inlineAnnotated(block.text, palette.codeWell, palette.link),
                style = MaterialTheme.typography.bodyMedium,
                color = palette.muted,
                modifier = Modifier.weight(1f),
            )
        }
        MdBlock.Rule -> HorizontalDivider(color = palette.rule)
    }
}
