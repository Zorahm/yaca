package com.yaca.android.chat

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.yaca.android.AppContainer
import com.yaca.android.host.HostError
import com.yaca.android.ui.common.projectShortName
import com.yaca.core.client.ConnectionState
import com.yaca.core.client.HostClient
import com.yaca.core.client.NotConnectedException
import com.yaca.core.client.RemoteCallException
import com.yaca.core.feed.Feed
import com.yaca.core.feed.fold
import com.yaca.core.feed.visible
import com.yaca.core.model.CheckpointId
import com.yaca.core.model.Command
import com.yaca.core.model.CommandAnswerElicitation
import com.yaca.core.model.CommandAnswerPermission
import com.yaca.core.model.CommandAnswerQuestion
import com.yaca.core.model.CommandCancel
import com.yaca.core.model.CommandCancelQueued
import com.yaca.core.model.CommandCompact
import com.yaca.core.model.CommandPreviewCheckpoint
import com.yaca.core.model.CommandReadFileView
import com.yaca.core.model.CommandRestoreCheckpoint
import com.yaca.core.model.CommandSearchFiles
import com.yaca.core.model.CommandSendMessage
import com.yaca.core.model.CommandSetEffort
import com.yaca.core.model.CommandSetMode
import com.yaca.core.model.CommandSetModel
import com.yaca.core.model.CommandStopProcess
import com.yaca.core.model.CommandWatchProcess
import com.yaca.core.model.Effort
import com.yaca.core.model.ElicitationReply
import com.yaca.core.model.Mode
import com.yaca.core.model.ModelInfo
import com.yaca.core.model.PermissionAnswer
import com.yaca.core.model.ProcessId
import com.yaca.core.model.QueuedId
import com.yaca.core.model.QuestionReply
import com.yaca.core.model.RequestId
import com.yaca.core.model.SessionId
import com.yaca.core.protocol.ChatSummary
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/**
 * One chat on one host. The feed is the host's, mirrored: this class folds it for display and
 * sends the human's commands. What only the screen decides (which sheet, which panel, what is
 * typed) lives here too, so it survives a rotation.
 *
 * The feed is attached only while the screen collects [ui]: the view model does not keep a
 * connection to the chat alive when the chat is off screen.
 */
class ChatViewModel(
    private val container: AppContainer,
    private val fingerprint: String,
    session: String,
) : ViewModel() {
    private val sessionId = SessionId(session)

    /** Failed calls, for the snackbar. */
    private val failures = Channel<HostError>(Channel.BUFFERED)
    val errors: Flow<HostError> = failures.receiveAsFlow()

    private val client: Flow<HostClient?> = container.hosts
        .map { records -> records?.firstOrNull { it.fingerprint == fingerprint }?.let { container.clientFor(it) } }
        .distinctUntilChanged()

    private val feed: StateFlow<Feed> = client
        .flatMapLatest { host -> host?.feed(sessionId) ?: flowOf(Feed.EMPTY) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MS), Feed.EMPTY)

    private val chat: Flow<ChatSummary?> = client.flatMapLatest { host ->
        host?.chats?.map { list -> list.firstOrNull { it.id == sessionId } } ?: flowOf(null)
    }

    private val connection: Flow<ConnectionState> = client.flatMapLatest { host ->
        host?.state ?: flowOf(ConnectionState.Idle)
    }

    private val models = MutableStateFlow<List<ModelInfo>>(emptyList())

    /** Everything the screen draws, in one value. */
    val ui: StateFlow<ChatUi> = combine(feed, chat, connection, models, container.hosts) { f, summary, state, list, hosts ->
        ChatUi(
            feed = f,
            items = fold(visible(f)),
            title = summary?.meta?.title,
            project = summary?.let { projectShortName(it.project) }.orEmpty(),
            connection = state,
            models = list,
            gone = hosts != null && hosts.none { it.fingerprint == fingerprint },
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MS), ChatUi())

    // The request sheet. A request that appears in the feed opens its sheet once; the human may
    // close it, and then only the block in the feed reopens it.
    private val sheetState = MutableStateFlow<RequestSheet?>(null)
    val sheet: StateFlow<RequestSheet?> = sheetState.asStateFlow()
    private var autoOpenedFor: String? = null

    private val panelState = MutableStateFlow<ChatPanel?>(null)
    val panel: StateFlow<ChatPanel?> = panelState.asStateFlow()

    private val confirmState = MutableStateFlow<ChatConfirm?>(null)
    val confirm: StateFlow<ChatConfirm?> = confirmState.asStateFlow()

    private val draftState = MutableStateFlow(ComposerDraft())
    val draft: StateFlow<ComposerDraft> = draftState.asStateFlow()

    init {
        // Every time the connection comes up, the models are fetched anew: a host may have gained one.
        viewModelScope.launch {
            connection.filterIsInstance<ConnectionState.Connected>().collect { loadModels() }
        }
    }

    // --- Request sheet ------------------------------------------------------------------------

    /**
     * Called as the open request in the feed changes. A new request opens its sheet; a sheet
     * whose request is no longer open (answered on the computer, or the turn ended) closes.
     */
    fun syncOpenRequest(open: RequestSheet?) {
        if (open != null && open.id != autoOpenedFor) {
            autoOpenedFor = open.id
            sheetState.value = open
        }
        val shown = sheetState.value
        if (shown != null && shown.id != open?.id) sheetState.value = null
    }

    /** Reopens a request from its block in the feed. */
    fun openSheet(sheet: RequestSheet) {
        sheetState.value = sheet
    }

    fun dismissSheet() {
        sheetState.value = null
    }

    fun answerPermission(requestId: RequestId, answer: PermissionAnswer) {
        send(CommandAnswerPermission(requestId, answer)) { closeSheetFor(requestId.value) }
    }

    fun answerQuestion(requestId: RequestId, reply: QuestionReply) {
        send(CommandAnswerQuestion(requestId, reply)) { closeSheetFor(requestId.value) }
    }

    fun answerElicitation(requestId: RequestId, reply: ElicitationReply) {
        send(CommandAnswerElicitation(requestId, reply)) { closeSheetFor(requestId.value) }
    }

    private fun closeSheetFor(id: String) {
        if (sheetState.value?.id == id) sheetState.value = null
    }

    // --- Panels and confirmations -------------------------------------------------------------

    /** Opens a panel; a file's panel asks the core for its full view first. */
    fun openPanel(panel: ChatPanel) {
        panelState.value = panel
        when (panel) {
            is ChatPanel.Diff -> readFileView(panel.path)
            is ChatPanel.File -> readFileView(panel.path)
            ChatPanel.Checkpoints, ChatPanel.Processes -> Unit
        }
    }

    fun closePanel() {
        panelState.value = null
    }

    fun ask(confirm: ChatConfirm) {
        confirmState.value = confirm
    }

    fun dismissConfirm() {
        confirmState.value = null
    }

    // --- Composer -----------------------------------------------------------------------------

    fun setDraftText(text: String) {
        draftState.update { it.copy(text = text) }
    }

    fun addAttachment(path: String) {
        draftState.update { if (path in it.attachments) it else it.copy(attachments = it.attachments + path) }
    }

    fun removeAttachment(path: String) {
        draftState.update { it.copy(attachments = it.attachments - path) }
    }

    fun setMentionOpen(open: Boolean) {
        draftState.update { it.copy(mentionOpen = open) }
    }

    /**
     * Sends what is typed. While a turn runs the core queues the message; the draft clears only
     * when the send succeeded and the text was not changed meanwhile.
     */
    fun sendMessage() {
        val current = draftState.value
        val text = current.text.trim()
        if (text.isEmpty() && current.attachments.isEmpty()) return
        send(CommandSendMessage(text = text, attachments = current.attachments)) {
            draftState.update { now ->
                if (now.text.trim() == text) now.copy(text = "", attachments = emptyList()) else now
            }
        }
    }

    // --- Commands -----------------------------------------------------------------------------

    fun cancelTurn() = send(CommandCancel)

    fun setMode(mode: Mode) = send(CommandSetMode(mode))

    fun setModel(model: String) = send(CommandSetModel(model))

    fun setEffort(effort: Effort) = send(CommandSetEffort(effort))

    fun cancelQueued(id: QueuedId) = send(CommandCancelQueued(id))

    fun compact() = send(CommandCompact)

    fun watchProcess(id: ProcessId?) = send(CommandWatchProcess(id))

    fun stopProcess(id: ProcessId) = send(CommandStopProcess(id))

    fun previewCheckpoint(id: CheckpointId) = send(CommandPreviewCheckpoint(id))

    fun restoreCheckpoint(id: CheckpointId) = send(CommandRestoreCheckpoint(id))

    fun readFileView(path: String) = send(CommandReadFileView(path))

    fun searchFiles(query: String) = send(CommandSearchFiles(query))

    /** Closes the chat on the computer, for everyone. [onClosed] runs once the host has done it. */
    fun closeChat(onClosed: () -> Unit) {
        viewModelScope.launch {
            if (call { it.close(sessionId) } != null) onClosed()
        }
    }

    private fun send(command: Command, onSent: () -> Unit = {}) {
        viewModelScope.launch {
            if (call { it.send(sessionId, command) } != null) onSent()
        }
    }

    private suspend fun loadModels() {
        call { it.models() }?.let { models.value = it }
    }

    /**
     * Runs one call to the host. A failure becomes a snackbar and a null answer, so the caller
     * only acts on success.
     */
    private suspend fun <T> call(block: suspend (HostClient) -> T): T? {
        val host = currentClient()
        if (host == null) {
            failures.send(HostError.NotConnected)
            return null
        }
        return try {
            block(host)
        } catch (remote: RemoteCallException) {
            failures.send(HostError.Remote(remote.message.orEmpty()))
            null
        } catch (offline: NotConnectedException) {
            failures.send(HostError.NotConnected)
            null
        }
    }

    private fun currentClient(): HostClient? =
        container.hosts.value?.firstOrNull { it.fingerprint == fingerprint }?.let { container.clientFor(it) }

    private companion object {
        /** A feed stays attached this long after the screen stops collecting it, so a quick return does not refetch. */
        const val STOP_TIMEOUT_MS = 5_000L
    }
}
