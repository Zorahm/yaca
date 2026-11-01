package com.yaca.android.chat.markdown

/**
 * One block of an agent's Markdown. A list is one block per item: the view spaces them, and an item
 * knows its depth and its number. Tables and HTML are not parsed; they stay as the paragraph text
 * the agent wrote, so nothing is hidden.
 */
sealed interface MdBlock {
    data class Paragraph(val text: String) : MdBlock

    data class Heading(val level: Int, val text: String) : MdBlock

    data class Item(val ordered: Boolean, val number: Int, val depth: Int, val text: String) : MdBlock

    data class Code(val language: String, val text: String) : MdBlock

    data class Quote(val text: String) : MdBlock

    data object Rule : MdBlock
}

private val HEADING = Regex("""^(#{1,6})\s+(.*)$""")
private val ITEM = Regex("""^(\s*)([-*+]|(\d{1,9})[.)])\s+(.*)$""")
private val RULE = Regex("""^([-*_])(\s*\1){2,}$""")
private val QUOTE = Regex("""^>\s?(.*)$""")

/**
 * Splits a message into blocks. It reads what the agent streams: a fence still open at the end of
 * the text runs to the end, a list item that wraps continues under itself, and a blank line ends a
 * paragraph. Nothing here can fail; any text is some blocks.
 */
fun parseMarkdown(source: String): List<MdBlock> {
    val lines = source.replace("\r\n", "\n").replace('\r', '\n').split('\n')
    val out = ArrayList<MdBlock>()
    val paragraph = ArrayList<String>()
    var inItem = false
    var index = 0

    fun flush() {
        if (paragraph.isNotEmpty()) {
            out.add(MdBlock.Paragraph(paragraph.joinToString("\n").trimEnd()))
            paragraph.clear()
        }
    }

    while (index < lines.size) {
        val line = lines[index]
        val trimmed = line.trim()
        val heading = HEADING.matchEntire(trimmed)
        val item = ITEM.matchEntire(line)
        val quote = QUOTE.matchEntire(trimmed)
        when {
            trimmed.startsWith("```") -> {
                flush()
                inItem = false
                val language = trimmed.removePrefix("```").trim()
                val code = ArrayList<String>()
                index += 1
                while (index < lines.size && !lines[index].trim().startsWith("```")) {
                    code.add(lines[index])
                    index += 1
                }
                out.add(MdBlock.Code(language, code.joinToString("\n")))
                // The closing fence; past the end when the text stops inside the block.
                index += 1
            }
            trimmed.isEmpty() -> {
                flush()
                inItem = false
                index += 1
            }
            RULE.matches(trimmed) -> {
                flush()
                inItem = false
                out.add(MdBlock.Rule)
                index += 1
            }
            heading != null -> {
                flush()
                inItem = false
                out.add(MdBlock.Heading(level = heading.groupValues[1].length, text = heading.groupValues[2].trim()))
                index += 1
            }
            item != null -> {
                flush()
                val indent = item.groupValues[1].length
                val marker = item.groups[3]?.value
                out.add(
                    MdBlock.Item(
                        ordered = marker != null,
                        number = marker?.toIntOrNull() ?: 0,
                        depth = (indent / 2).coerceAtMost(4),
                        text = item.groupValues[4].trim(),
                    ),
                )
                inItem = true
                index += 1
            }
            quote != null -> {
                flush()
                inItem = false
                val quoted = ArrayList<String>()
                while (index < lines.size) {
                    val match = QUOTE.matchEntire(lines[index].trim()) ?: break
                    quoted.add(match.groupValues[1])
                    index += 1
                }
                out.add(MdBlock.Quote(quoted.joinToString("\n").trimEnd()))
            }
            inItem && paragraph.isEmpty() && line.startsWith(" ") -> {
                // A wrapped line of the item above: it belongs to that item, not to a new paragraph.
                val last = out.lastOrNull() as? MdBlock.Item
                if (last != null) out[out.lastIndex] = last.copy(text = last.text + "\n" + trimmed)
                index += 1
            }
            else -> {
                inItem = false
                paragraph.add(line.trimEnd())
                index += 1
            }
        }
    }
    flush()
    return out
}
