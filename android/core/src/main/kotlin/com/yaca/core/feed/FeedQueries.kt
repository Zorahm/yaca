package com.yaca.core.feed

import com.yaca.core.model.FileDiff
import com.yaca.core.model.FileFact

/** What is visible in the feed: closed permission requests leave it. */
fun visible(feed: Feed): List<FeedBlock> =
    feed.blocks.filter { block -> block !is FeedBlock.Permission || block.open }

/**
 * Files the session has touched, in order of appearance.
 *
 * `⌥←` and `⌥→` page through the file panel by this list. It used to come from the set of
 * edits; now it is derived from the feed, which already knows all of this.
 */
fun touchedFiles(feed: Feed): List<String> {
    val out = mutableListOf<String>()
    for (block in feed.blocks) {
        if (block !is FeedBlock.Actions) continue
        for (row in flatRows(block.rows)) {
            val path = row.fact?.path ?: continue
            if (path !in out) out.add(path)
        }
    }
    return out
}

/**
 * The latest thing the core said about a file: the kind of change and the counters.
 *
 * It is looked up where the diff is, for the same reason: the set of edits that would know this
 * separately no longer exists.
 */
fun factOf(feed: Feed, path: String): FileFact? {
    for (block in feed.blocks.asReversed()) {
        if (block !is FeedBlock.Actions) continue
        val fact = flatRows(block.rows).asReversed().firstNotNullOfOrNull { row ->
            row.fact?.takeIf { it.path == path }
        }
        if (fact != null) return fact
    }
    return null
}

/**
 * The diff of a file for the file panel.
 *
 * The staging index comes first: it holds the latest thing the core sent for the path. A panel
 * opened from an old action row after applying falls back to the diff of that row itself: the
 * set is already empty, but there is no reason to hide what the feed has.
 */
fun diffOf(feed: Feed, path: String): FileDiff? {
    feed.diffs[path]?.let { return it }
    for (block in feed.blocks.asReversed()) {
        if (block !is FeedBlock.Actions) continue
        val diff = flatRows(block.rows).asReversed().firstNotNullOfOrNull { row ->
            if (row.fact?.path == path) row.diff else null
        }
        if (diff != null) return diff
    }
    return null
}
