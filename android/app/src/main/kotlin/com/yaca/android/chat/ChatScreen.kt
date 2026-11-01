package com.yaca.android.chat

import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.yaca.android.appViewModelFactory
import com.yaca.android.host.text
import kotlinx.coroutines.launch

/**
 * One chat, as the navigation opens it. The view model is keyed by the host and the session, so a
 * chat keeps its draft and its open sheet while the screen is in the back stack.
 *
 * The feed is collected through [collectAsStateWithLifecycle]: it is attached while the screen is
 * started and released when the screen goes off.
 */
@Composable
fun ChatScreen(fingerprint: String, session: String, onBack: () -> Unit) {
    val vm: ChatViewModel = viewModel(
        key = "$fingerprint/$session",
        factory = appViewModelFactory { ChatViewModel(it, fingerprint, session) },
    )
    val ui by vm.ui.collectAsStateWithLifecycle()
    val draft by vm.draft.collectAsStateWithLifecycle()
    val sheet by vm.sheet.collectAsStateWithLifecycle()
    val panel by vm.panel.collectAsStateWithLifecycle()
    val confirm by vm.confirm.collectAsStateWithLifecycle()

    val snackbar = remember { SnackbarHostState() }
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val openRequest = remember(ui.feed) { openRequestOf(ui.feed) }

    LaunchedEffect(vm) {
        vm.errors.collect { snackbar.showSnackbar(it.text(context)) }
    }
    // A request that is open in the feed opens its sheet; one that is no longer open closes it.
    LaunchedEffect(openRequest?.id) { vm.syncOpenRequest(openRequest) }
    LaunchedEffect(ui.gone) {
        if (ui.gone) onBack()
    }

    ChatContent(
        ui = ui,
        draft = draft,
        listState = listState,
        snackbar = snackbar,
        onBack = onBack,
        feedActions = FeedActions(
            onOpenDiff = { vm.openPanel(ChatPanel.Diff(it)) },
            onOpenFile = { vm.openPanel(ChatPanel.File(it)) },
            onOpenRequest = vm::openSheet,
            onOpenCheckpoints = { vm.openPanel(ChatPanel.Checkpoints) },
        ),
        composer = ComposerActions(
            onTextChange = vm::setDraftText,
            onSend = {
                vm.sendMessage()
                // The sent message is the newest item: bring it into view.
                scope.launch { listState.animateScrollToItem(0) }
            },
            onStop = vm::cancelTurn,
            onMention = { vm.setMentionOpen(true) },
            onRemoveAttachment = vm::removeAttachment,
            onMode = vm::setMode,
            onModel = vm::setModel,
            onEffort = vm::setEffort,
            onCancelQueued = vm::cancelQueued,
        ),
        menu = ChatMenuActions(
            onProcesses = { vm.openPanel(ChatPanel.Processes) },
            onCheckpoints = { vm.openPanel(ChatPanel.Checkpoints) },
            onCompact = { vm.ask(ChatConfirm.Compact) },
            onClose = { vm.ask(ChatConfirm.CloseChat) },
        ),
    )

    sheet?.let { current ->
        RequestSheetHost(
            sheet = current,
            feed = ui.feed,
            onAnswerPermission = vm::answerPermission,
            onAnswerQuestion = vm::answerQuestion,
            onAnswerElicitation = vm::answerElicitation,
            onDismiss = vm::dismissSheet,
        )
    }

    panel?.let { current ->
        PanelHost(
            panel = current,
            feed = ui.feed,
            onOpenCheckpoints = { vm.openPanel(ChatPanel.Checkpoints) },
            onWatch = vm::watchProcess,
            onStop = vm::stopProcess,
            onPreview = vm::previewCheckpoint,
            onRestore = vm::restoreCheckpoint,
            onReadFile = vm::readFileView,
            onClose = vm::closePanel,
        )
    }

    if (draft.mentionOpen) {
        MentionHost(
            feed = ui.feed,
            onSearch = vm::searchFiles,
            onPick = {
                vm.addAttachment(it)
                vm.setMentionOpen(false)
            },
            onDismiss = { vm.setMentionOpen(false) },
        )
    }

    confirm?.let { current ->
        ChatConfirmDialog(
            confirm = current,
            onConfirm = {
                vm.dismissConfirm()
                when (current) {
                    ChatConfirm.CloseChat -> vm.closeChat(onBack)
                    ChatConfirm.Compact -> vm.compact()
                }
            },
            onDismiss = vm::dismissConfirm,
        )
    }
}
