package com.yaca.android

import android.content.Context
import com.yaca.android.attention.AttentionTracker
import com.yaca.android.attention.ChatKey
import com.yaca.android.attention.VisibleChat
import com.yaca.android.notify.AttentionNotifier
import com.yaca.android.settings.AppSettings
import com.yaca.android.settings.SettingsStore
import com.yaca.android.store.HostStore
import com.yaca.core.client.HostClient
import com.yaca.core.client.HostRecord
import com.yaca.core.client.RemoteHostClient
import com.yaca.core.model.PermissionAnswer
import com.yaca.core.model.RequestId
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.getAndUpdate
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/**
 * What the app shares for its whole process: the host store, one client per paired host,
 * the attention tracker, the notifications and the settings. Plain constructors, no DI library.
 *
 * Clients are made when a host first appears in the store (at start for stored hosts, at
 * pairing for new ones), not when a screen asks: the attention tracker has to see every
 * host, or a chat that waits on a computer nobody opened would stay silent.
 */
class AppContainer(context: Context, val scope: CoroutineScope) {
    val hostStore = HostStore(context)
    val settingsStore = SettingsStore(context)

    /** The settings; the defaults until the store has answered once. */
    val settings: StateFlow<AppSettings> = settingsStore.settings.stateIn(scope, SharingStarted.Eagerly, AppSettings())

    val attention = AttentionTracker(scope)

    /** The chat the human sees right now. A chat on screen gets no notification. */
    val visibleChat = VisibleChat()

    /** Guarded by itself: a client is made once per fingerprint, however many callers ask. */
    private val clients = HashMap<String, HostClient>()

    /** The paired hosts; null until the store has answered once. */
    val hosts: StateFlow<List<HostRecord>?> = hostStore.hosts
        .map<List<HostRecord>, List<HostRecord>?> { it }
        .stateIn(scope, SharingStarted.Eagerly, null)

    private val notifier = AttentionNotifier(
        context = context,
        visibleChat = visibleChat,
        settings = settings,
        attention = attention,
        clientFor = { fingerprint ->
            hosts.value?.firstOrNull { it.fingerprint == fingerprint }?.let { clientFor(it) }
        },
    )

    private val pendingLink = MutableStateFlow<String?>(null)

    /** A `yaca://pair` link another app handed to us, until the pairing screen takes it. */
    val incomingLink: StateFlow<String?> = pendingLink.asStateFlow()

    private val pendingChat = MutableStateFlow<ChatKey?>(null)

    /** A chat a notification asked the app to show, until the navigation takes it. */
    val openChat: StateFlow<ChatKey?> = pendingChat.asStateFlow()

    init {
        scope.launch {
            hosts.filterNotNull().collect { records ->
                records.forEach { clientFor(it) }
                retainOnly(records.map { it.fingerprint }.toSet())
            }
        }
        scope.launch {
            attention.pushed.collect { notifier.onPush(it) }
        }
    }

    /**
     * The client of [record]. A record that changed (the host was paired again and got a new
     * token) replaces the old client: the old connection is dropped, not reused with old credentials.
     */
    fun clientFor(record: HostRecord): HostClient = synchronized(clients) {
        val existing = clients[record.fingerprint]
        if (existing != null && existing.record == record) {
            existing
        } else {
            existing?.disconnect()
            RemoteHostClient(record, scope).also { client ->
                clients[record.fingerprint] = client
                attention.watch(client)
                client.connect()
            }
        }
    }

    /** Connects every client. A client that is already connected is left alone. */
    fun connectAll() {
        clientList().forEach { it.connect() }
    }

    /** Stops every connection: the app left the foreground without the connection service. */
    fun disconnectAll() {
        clientList().forEach { it.disconnect() }
    }

    private fun clientList(): List<HostClient> = synchronized(clients) { clients.values.toList() }

    /** A chat's screen came up: it is not notified about, it no longer waits for the human, and its notification goes. */
    fun onChatShown(key: ChatKey) {
        visibleChat.show(key)
        attention.markSeen(key.fingerprint, key.session)
        notifier.cancel(key)
    }

    fun onChatHidden(key: ChatKey) {
        visibleChat.hide(key)
    }

    /** Answers a permission request from its notification; false when the answer did not reach the computer. */
    suspend fun answerPermission(key: ChatKey, requestId: RequestId, answer: PermissionAnswer): Boolean =
        notifier.answerPermission(key, requestId, answer)

    fun offerIncomingLink(link: String) {
        pendingLink.value = link
    }

    fun takeIncomingLink(): String? = pendingLink.getAndUpdate { null }

    fun offerOpenChat(key: ChatKey) {
        pendingChat.value = key
    }

    fun takeOpenChat(): ChatKey? = pendingChat.getAndUpdate { null }

    /** Drops the clients of hosts that were forgotten: their connections stop and nothing waits on them. */
    private fun retainOnly(fingerprints: Set<String>) {
        val gone = synchronized(clients) {
            val stale = clients.keys.filter { it !in fingerprints }
            stale.mapNotNull { fingerprint ->
                clients.remove(fingerprint)?.also { attention.forget(fingerprint) }
            }
        }
        gone.forEach { it.disconnect() }
    }
}
