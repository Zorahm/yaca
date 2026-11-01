package com.yaca.core.feed

/** The action rows of a work item, for its summary line. */
fun workRows(steps: List<FeedItem.WorkStep>): List<ActionRow> =
    steps.filterIsInstance<FeedItem.WorkStep.Action>().map { it.row }

/**
 * Whether the work item at [index] is the one the running turn is adding to.
 *
 * It is while nothing the agent said stands under it: text closes the stretch, and whatever comes
 * next is a new one. The todo list and a request waiting for the person do not close it.
 */
fun isLiveWork(items: List<FeedItem>, index: Int, busy: Boolean): Boolean {
    if (!busy) return false
    for (i in index + 1 until items.size) {
        val item = items[i]
        val transparent = item is FeedBlock.Todos ||
            item is FeedBlock.Permission ||
            item is FeedBlock.Question ||
            item is FeedBlock.Elicitation
        if (!transparent) return false
    }
    return true
}

/**
 * Whether the text at [index] is the last thing the agent said in its reply.
 *
 * A remark between two stretches of work is not the answer, and neither is text of a turn that is
 * still running: it may be mid-sentence, or more may follow it.
 */
fun closesReply(items: List<FeedItem>, index: Int, busy: Boolean): Boolean {
    for (i in index + 1 until items.size) {
        when (items[i]) {
            is FeedBlock.Human -> return true
            // What the agent still adds to a reply after a piece of its text.
            is FeedBlock.Agent,
            is FeedItem.Work,
            is FeedItem.Agents,
            is FeedBlock.Plan,
            is FeedBlock.Shown,
            -> return false
            else -> Unit
        }
    }
    return !busy
}

/**
 * Everything the model said in the reply that the text at [index] closes: its own words from the
 * human's message up to here, in order, joined by a blank line. A reply is cut into pieces by the
 * work between them, and the copy button sits under the last piece, so it has to gather the others.
 *
 * Only the model's text. Tool rows, reasoning, plans and files the agent showed ([FeedBlock.Shown])
 * are not what the model wrote to the human, and are left out.
 */
fun replyText(items: List<FeedItem>, index: Int): String {
    val parts = mutableListOf<String>()
    for (i in index downTo 0) {
        val item = items[i]
        if (item is FeedBlock.Human) break
        if (item is FeedBlock.Agent && item.text.trim().isNotEmpty()) parts.add(item.text.trim())
    }
    return parts.reversed().joinToString("\n\n")
}
