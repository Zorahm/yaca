@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.chat
import androidx.compose.material3.ExperimentalMaterial3Api

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.yaca.android.R
import com.yaca.android.chat.panels.CheckpointsPanel
import com.yaca.android.chat.panels.DiffPanel
import com.yaca.android.chat.panels.FilePanel
import com.yaca.android.chat.panels.MentionMenu
import com.yaca.android.chat.panels.ProcessesPanel
import com.yaca.android.chat.sheets.ElicitationSheet
import com.yaca.android.chat.sheets.PermissionSheet
import com.yaca.android.chat.sheets.QuestionSheet
import com.yaca.core.feed.Feed
import com.yaca.core.model.CheckpointId
import com.yaca.core.model.ElicitationReply
import com.yaca.core.model.PermissionAnswer
import com.yaca.core.model.ProcessId
import com.yaca.core.model.QuestionReply
import com.yaca.core.model.RequestId

/**
 * The open request as a bottom sheet. The sheet is answered or dismissed here; the answer goes up
 * through the callbacks, and the view model closes the sheet once the host has taken it.
 */
@Composable
fun RequestSheetHost(
    sheet: RequestSheet,
    feed: Feed,
    onAnswerPermission: (RequestId, PermissionAnswer) -> Unit,
    onAnswerQuestion: (RequestId, QuestionReply) -> Unit,
    onAnswerElicitation: (RequestId, ElicitationReply) -> Unit,
    onDismiss: () -> Unit,
) {
    when (sheet) {
        is RequestSheet.Permission -> PermissionSheet(
            request = sheet.request,
            reason = findRow(feed, sheet.request.callId)?.description,
            onAnswer = { onAnswerPermission(sheet.request.id, it) },
            onDismiss = onDismiss,
        )
        is RequestSheet.Question -> QuestionSheet(
            request = sheet.request,
            onAnswer = { onAnswerQuestion(sheet.request.id, it) },
            onDismiss = onDismiss,
        )
        is RequestSheet.Elicitation -> ElicitationSheet(
            request = sheet.request,
            onReply = { onAnswerElicitation(sheet.request.id, it) },
            onDismiss = onDismiss,
        )
    }
}

/**
 * The file diff, the file view, the checkpoints and the processes: each takes the whole window.
 * A dialog with the platform width off is the full-screen layer; the system Back closes it.
 */
@Composable
fun PanelHost(
    panel: ChatPanel,
    feed: Feed,
    onOpenCheckpoints: () -> Unit,
    onWatch: (ProcessId?) -> Unit,
    onStop: (ProcessId) -> Unit,
    onPreview: (CheckpointId) -> Unit,
    onRestore: (CheckpointId) -> Unit,
    onReadFile: (String) -> Unit,
    onClose: () -> Unit,
) {
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            when (panel) {
                is ChatPanel.Diff -> DiffPanel(
                    feed = feed,
                    path = panel.path,
                    onOpenCheckpoints = onOpenCheckpoints,
                    onClose = onClose,
                )
                is ChatPanel.File -> FilePanel(
                    feed = feed,
                    path = panel.path,
                    onRequest = onReadFile,
                    onClose = onClose,
                )
                ChatPanel.Checkpoints -> CheckpointsPanel(
                    feed = feed,
                    onPreview = onPreview,
                    onRestore = onRestore,
                    onClose = onClose,
                )
                ChatPanel.Processes -> ProcessesPanel(
                    feed = feed,
                    onWatch = onWatch,
                    onStop = onStop,
                    onClose = onClose,
                )
            }
        }
    }
}

/**
 * The `@` menu as a bottom sheet. The query lives here: the menu asks the core for matches as it
 * is typed, and the core's answer comes back in the feed.
 */
@Composable
fun MentionHost(
    feed: Feed,
    onSearch: (String) -> Unit,
    onPick: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var query by rememberSaveable { mutableStateOf("") }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        MentionMenu(
            feed = feed,
            query = query,
            onSearch = {
                query = it
                onSearch(it)
            },
            onPick = onPick,
            onDismiss = onDismiss,
        )
    }
}

/** A dialog before an action that cannot be taken back. The confirm button is the error role when the action destroys. */
@Composable
fun ChatConfirmDialog(
    confirm: ChatConfirm,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    val text = confirmText(confirm)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(text.title) },
        text = { Text(text.body) },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(
                    text = text.action,
                    color = if (text.destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_cancel)) }
        },
    )
}

private class ConfirmText(
    val title: String,
    val body: String,
    val action: String,
    val destructive: Boolean,
)

@Composable
private fun confirmText(confirm: ChatConfirm): ConfirmText = when (confirm) {
    ChatConfirm.CloseChat -> ConfirmText(
        title = stringResource(R.string.chat_close_title),
        body = stringResource(R.string.chat_close_body),
        action = stringResource(R.string.chat_close_confirm),
        destructive = true,
    )
    ChatConfirm.Compact -> ConfirmText(
        title = stringResource(R.string.chat_compact_title),
        body = stringResource(R.string.chat_compact_body),
        action = stringResource(R.string.chat_compact_confirm),
        destructive = false,
    )
}
