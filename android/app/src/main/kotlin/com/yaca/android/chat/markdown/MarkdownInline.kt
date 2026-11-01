package com.yaca.android.chat.markdown

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle

/** The inline pieces a line of text is made of. */
sealed interface MdInline {
    data class Plain(val text: String) : MdInline

    data class Code(val text: String) : MdInline

    data class Bold(val children: List<MdInline>) : MdInline

    data class Italic(val children: List<MdInline>) : MdInline

    /** Only web addresses become links; anything else stays text, so a line cannot open an odd scheme. */
    data class Link(val children: List<MdInline>, val url: String) : MdInline
}

/**
 * Reads the inline marks of one block: code spans, bold, italic and links. A mark without a closing
 * partner is text, so a stray asterisk or an unfinished bracket never swallows the rest of the line.
 */
fun parseInline(text: String): List<MdInline> {
    val out = ArrayList<MdInline>()
    val plain = StringBuilder()

    fun flush() {
        if (plain.isNotEmpty()) {
            out.add(MdInline.Plain(plain.toString()))
            plain.clear()
        }
    }

    var i = 0
    while (i < text.length) {
        val c = text[i]
        if (c == '`') {
            val close = text.indexOf('`', i + 1)
            if (close > i) {
                flush()
                out.add(MdInline.Code(text.substring(i + 1, close)))
                i = close + 1
                continue
            }
        }
        if (text.startsWith("**", i) || text.startsWith("__", i)) {
            val delimiter = text.substring(i, i + 2)
            val close = text.indexOf(delimiter, i + 2)
            if (close > i + 2) {
                flush()
                out.add(MdInline.Bold(parseInline(text.substring(i + 2, close))))
                i = close + 2
                continue
            }
        }
        if (c == '*' || c == '_') {
            val close = italicClose(text, i)
            if (close > i) {
                flush()
                out.add(MdInline.Italic(parseInline(text.substring(i + 1, close))))
                i = close + 1
                continue
            }
        }
        if (c == '[') {
            val labelEnd = text.indexOf("](", i + 1)
            val urlEnd = if (labelEnd > i) text.indexOf(')', labelEnd + 2) else -1
            val label = if (labelEnd > i) text.substring(i + 1, labelEnd) else ""
            if (urlEnd > labelEnd + 2 && '\n' !in label && '[' !in label) {
                val url = text.substring(labelEnd + 2, urlEnd).trim()
                if (url.startsWith("https://") || url.startsWith("http://")) {
                    flush()
                    out.add(MdInline.Link(parseInline(label), url))
                    i = urlEnd + 1
                    continue
                }
            }
        }
        plain.append(c)
        i += 1
    }
    flush()
    return out
}

/**
 * The index of the character that closes an italic run opened at [open], or -1. An underscore
 * inside a word (`snake_case`) is not an opener, so identifiers stay as they are.
 */
private fun italicClose(text: String, open: Int): Int {
    val mark = text[open]
    if (open + 1 >= text.length || text[open + 1].isWhitespace()) return -1
    if (mark == '_' && open > 0 && text[open - 1].isLetterOrDigit()) return -1
    var j = open + 1
    while (j < text.length) {
        val closes = text[j] == mark &&
            text[j - 1] != ' ' &&
            (j + 1 >= text.length || text[j + 1] != mark)
        if (closes && (mark != '_' || j + 1 >= text.length || !text[j + 1].isLetterOrDigit())) return j
        j += 1
    }
    return -1
}

/** The inline marks of [source] as styled text. [codeWell] is the background of code spans; [link] colours links. */
fun inlineAnnotated(source: String, codeWell: Color, link: Color): AnnotatedString =
    buildAnnotatedString { appendInline(parseInline(source), codeWell, link) }

private fun AnnotatedString.Builder.appendInline(items: List<MdInline>, codeWell: Color, link: Color) {
    for (item in items) {
        when (item) {
            is MdInline.Plain -> append(item.text)
            is MdInline.Code -> withStyle(SpanStyle(fontFamily = FontFamily.Monospace, background = codeWell)) {
                append(item.text)
            }
            is MdInline.Bold -> withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) {
                appendInline(item.children, codeWell, link)
            }
            is MdInline.Italic -> withStyle(SpanStyle(fontStyle = FontStyle.Italic)) {
                appendInline(item.children, codeWell, link)
            }
            is MdInline.Link -> withLink(
                LinkAnnotation.Url(
                    url = item.url,
                    styles = TextLinkStyles(style = SpanStyle(color = link, textDecoration = TextDecoration.Underline)),
                ),
            ) {
                appendInline(item.children, codeWell, link)
            }
        }
    }
}
