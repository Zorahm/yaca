package com.yaca.android.chat.feed

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.yaca.android.chat.markdown.Markdown

/** The agent's own words, as Markdown, at full width: the reply is the reading, not a bubble. */
@Composable
fun TextBlock(text: String, modifier: Modifier = Modifier) {
    Markdown(text = text, modifier = modifier)
}
