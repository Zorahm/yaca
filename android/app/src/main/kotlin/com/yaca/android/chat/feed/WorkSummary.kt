package com.yaca.android.chat.feed

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.yaca.android.R
import com.yaca.android.chat.pluralCount
import com.yaca.core.feed.ActionRow
import com.yaca.core.feed.ActionState
import com.yaca.core.feed.MULTI_FILE_TOOLS
import com.yaca.core.feed.flatRows
import com.yaca.core.model.ChangeKind

/** What a group of actions did, in the words of its header. */
enum class TallyKind { READ, CREATED, MODIFIED, DELETED, COMMANDS, TOOLS }

data class Tally(val kind: TallyKind, val count: Int)

/**
 * The counts of a group. [failed] counts the rows that ended badly (failed, denied, stopped);
 * [added] and [removed] are the line counters of the files it touched, subagents' rows included.
 */
data class GroupSummary(
    val tallies: List<Tally>,
    val failed: Int,
    val added: Int,
    val removed: Int,
)

/**
 * Counts what a group of rows did, the way the desktop's group header does: created, changed,
 * deleted, commands, reads and other tools. A rename counts the files under it, not the call.
 */
fun summarizeRows(rows: List<ActionRow>): GroupSummary {
    val counts = HashMap<TallyKind, Int>()
    var failed = 0
    var added = 0
    var removed = 0
    for (row in flatRows(rows)) {
        val bad = row.state == ActionState.FAILED ||
            row.state == ActionState.DENIED ||
            row.state == ActionState.STOPPED
        if (!bad && row.tool in MULTI_FILE_TOOLS && row.children.isNotEmpty()) continue
        if (bad) failed += 1
        val kind = tallyKindOf(row)
        counts[kind] = (counts[kind] ?: 0) + filesOf(row)
        row.fact?.let { fact ->
            added += fact.added
            removed += fact.removed
        }
    }
    val tallies = TallyKind.entries.mapNotNull { kind ->
        counts[kind]?.takeIf { it > 0 }?.let { Tally(kind, it) }
    }
    return GroupSummary(tallies = tallies, failed = failed, added = added, removed = removed)
}

/** A deleted folder counts the files it took with it; every other row is one thing. */
private fun filesOf(row: ActionRow): Int =
    if (row.tool == "DeleteDir" && row.state == ActionState.DONE) (row.meta?.items ?: 1) else 1

private fun tallyKindOf(row: ActionRow): TallyKind {
    val fact = row.fact
    if (fact != null) {
        return when (fact.kind) {
            ChangeKind.CREATED -> TallyKind.CREATED
            ChangeKind.DELETED -> TallyKind.DELETED
            ChangeKind.MODIFIED, ChangeKind.MOVED -> TallyKind.MODIFIED
        }
    }
    return when {
        row.tool == "Read" -> TallyKind.READ
        row.tool == "Write" -> TallyKind.CREATED
        row.tool == "Delete" || row.tool == "DeleteDir" -> TallyKind.DELETED
        row.tool == "Bash" -> TallyKind.COMMANDS
        row.tool in MULTI_FILE_TOOLS || row.tool == "Edit" || row.tool == "NotebookEdit" || row.tool == "Move" ->
            TallyKind.MODIFIED
        else -> TallyKind.TOOLS
    }
}

private fun pluralFor(kind: TallyKind): Int = when (kind) {
    TallyKind.READ -> R.plurals.work_read
    TallyKind.CREATED -> R.plurals.work_created
    TallyKind.MODIFIED -> R.plurals.work_modified
    TallyKind.DELETED -> R.plurals.work_deleted
    TallyKind.COMMANDS -> R.plurals.work_commands
    TallyKind.TOOLS -> R.plurals.work_tools
}

/**
 * The header line of a group: "Read 3 files, changed 1 file (1 failed)". A group with no action
 * in it (only reasoning) is headed by the word for reasoning.
 */
@Composable
fun groupHeadline(summary: GroupSummary): String {
    if (summary.tallies.isEmpty()) return stringResource(R.string.chat_reasoning)
    val separator = stringResource(R.string.chat_list_separator)
    val text = summary.tallies
        .map { tally -> pluralCount(pluralFor(tally.kind), tally.count) }
        .joinToString(separator)
    return if (summary.failed > 0) {
        stringResource(R.string.chat_work_with_failed, text, pluralCount(R.plurals.work_failed, summary.failed))
    } else {
        text
    }
}
