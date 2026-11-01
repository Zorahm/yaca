package com.yaca.android.chat.panels

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Width of a monospace run of [chars] characters at the code style's size. The monospace advance
 * is about 0.6 em; 8 sp a character leaves a little room, so a fallback font cannot clip a line.
 */
@Composable
internal fun codeWidth(chars: Int): Dp {
    val density = LocalDensity.current
    return with(density) { (chars * CODE_CHAR_SP).sp.toDp() }
}

private const val CODE_CHAR_SP = 8f

/**
 * A list of code rows that scrolls sideways as one block. Rows are not wrapped: a diff or a file
 * is read by line, and a wrapped line would take two numbers. The list is as wide as the longest
 * line needs, or the screen, whichever is wider, so no row is clipped and the short ones are not
 * stretched past their text.
 */
@Composable
internal fun CodeList(
    contentWidth: Dp,
    itemCount: Int,
    note: String?,
    row: @Composable (Int) -> Unit,
) {
    val scroll = rememberScrollState()
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val width = maxOf(contentWidth, maxWidth)
        Box(Modifier.fillMaxSize().horizontalScroll(scroll)) {
            LazyColumn(
                modifier = Modifier.width(width).fillMaxHeight(),
                contentPadding = PaddingValues(bottom = 24.dp),
            ) {
                items(itemCount) { index -> row(index) }
                if (note != null) {
                    item { PanelNote(note) }
                }
            }
        }
    }
}
