package com.yaca.android.chat.feed

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.chat.markdown.CodeBlock
import com.yaca.android.ui.theme.LocalCodeStyle
import com.yaca.core.feed.FeedBlock

/**
 * A file's text, or a background command's output, that the agent put in front of the human. The
 * text is the one the event carried; the file may have changed since, and the card does not say
 * otherwise.
 */
@Composable
fun ShownCard(block: FeedBlock.Shown, modifier: Modifier = Modifier) {
    val last = block.firstLine + block.lines - 1
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surface,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
    ) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                text = block.path,
                style = LocalCodeStyle.current,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            val command = block.command
            if (command != null) {
                Text(
                    text = stringResource(R.string.shown_command, command),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Text(
                text = stringResource(R.string.shown_range, block.firstLine, last, block.total),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            CodeBlock(text = block.text)
        }
    }
}
