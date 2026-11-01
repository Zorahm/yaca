package com.yaca.android.chat.feed

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.yaca.android.R
import com.yaca.android.chat.fileNameOf
import com.yaca.core.feed.ActionRow
import com.yaca.core.feed.ActionState
import com.yaca.core.feed.MULTI_FILE_TOOLS
import com.yaca.core.model.ChangeKind

/** Tools whose row is about one file: their counters and name sit where a file row lays them out. */
private val FILE_TOOLS = setOf("Read", "Write", "Edit", "NotebookEdit", "Delete", "DeleteDir", "Move")

/** Whether the row is about one file. */
fun isFileRow(row: ActionRow): Boolean =
    row.fact != null || (row.tool in FILE_TOOLS && row.target != null)

/** The file a finished read or a change points at, for the file and diff panels; null otherwise. */
fun openablePath(row: ActionRow): String? =
    row.fact?.path ?: row.target.takeIf { row.tool == "Read" && row.state == ActionState.DONE }

/** The line counters of a row: its own, or the sum over the files under a rename. Null when it touched no file. */
fun rowCounts(row: ActionRow): Pair<Int, Int>? {
    row.fact?.let { return it.added to it.removed }
    if (row.tool !in MULTI_FILE_TOOLS || row.children.isEmpty()) return null
    return row.children.fold(0 to 0) { sum, child ->
        (sum.first + (child.fact?.added ?: 0)) to (sum.second + (child.fact?.removed ?: 0))
    }
}

private fun nameOf(row: ActionRow): String? {
    row.fact?.let { return fileNameOf(it.path) }
    val target = row.target ?: return null
    return if (row.tool in FILE_TOOLS) fileNameOf(target) else null
}

/**
 * The one-line caption of a row, worded for its state. A file is named by its file name; a
 * command or a tool with no caption of its own shows its target, and a failure says why.
 */
@Composable
fun rowCaption(row: ActionRow): String {
    val name = nameOf(row)
    return when (row.state) {
        ActionState.RUNNING -> runningCaption(row, name)
        ActionState.DONE -> doneCaption(row)
        ActionState.FAILED -> when {
            // A cancelled turn ends its tools without a reason; that is a stop, not a failure.
            row.message == null -> stringResource(R.string.row_cancelled)
            name != null -> stringResource(R.string.row_edit_failed, name)
            else -> stringResource(R.string.row_failed)
        }
        ActionState.REJECTED -> stringResource(R.string.row_rejected, row.message.orEmpty())
        ActionState.DENIED -> row.message?.let { stringResource(R.string.row_denied_because, it) }
            ?: stringResource(R.string.row_denied)
        ActionState.STOPPED -> stringResource(R.string.row_stopped)
    }
}

@Composable
private fun runningCaption(row: ActionRow, name: String?): String {
    if (name != null) {
        val text = when (row.tool) {
            "Read" -> stringResource(R.string.file_reading, name)
            "Write" -> stringResource(R.string.file_creating, name)
            "Delete", "DeleteDir" -> stringResource(R.string.file_deleting, name)
            else -> stringResource(R.string.file_changing, name)
        }
        return text
    }
    // The model's own caption is in the past tense for a finished call; while running it is shown as written.
    return row.description ?: row.target ?: row.tool
}

@Composable
private fun doneCaption(row: ActionRow): String {
    val fact = row.fact
    if (fact != null) {
        val file = fileNameOf(fact.path)
        return when (fact.kind) {
            ChangeKind.CREATED -> stringResource(R.string.file_created, file)
            ChangeKind.DELETED -> stringResource(R.string.file_deleted, file)
            ChangeKind.MODIFIED -> stringResource(R.string.file_modified, file)
            ChangeKind.MOVED -> {
                val from = fact.movedFrom?.let { fileNameOf(it) }
                if (from != null) {
                    stringResource(R.string.file_moved, from, file)
                } else {
                    stringResource(R.string.file_modified, file)
                }
            }
        }
    }
    val target = row.target
    if (target != null && row.tool == "DeleteDir") {
        return stringResource(R.string.file_dir_deleted, fileNameOf(target))
    }
    if (target != null && row.tool == "Read") {
        return stringResource(R.string.file_read, fileNameOf(target))
    }
    return row.description ?: target ?: row.tool
}
