package com.yaca.core.client

import com.yaca.core.feed.Feed
import com.yaca.core.model.SessionId
import kotlin.time.Duration
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.stateIn

/**
 * The feed of one chat on one host. The attachment lives while [view] is collected: [onWanted] is
 * told when the first collector comes and when the last one leaves. The client keeps the flags
 * below under its own lock, because a reply and a frame can arrive while a collector comes or goes.
 */
internal class SessionFeed(
    val session: SessionId,
    scope: CoroutineScope,
    detachDelay: Duration,
    onWanted: (SessionFeed, Boolean) -> Unit,
) {
    /** The folded feed. Written only by the connection's reader, under the client's lock. */
    val state = MutableStateFlow(Feed.EMPTY)

    /** Whether a collector is subscribed. */
    var wanted = false

    /** The epoch of the current attachment, or null while there is none. */
    var epoch: Int? = null

    /** The request id of the latest attach. Only its reply may set the feed. */
    var attachId: Long? = null

    /**
     * What `HostClient.feed` returns. The upstream stops [detachDelay] after the last collector
     * leaves, so turning the screen off and on does not refetch the whole feed.
     */
    val view: StateFlow<Feed> = flow {
        onWanted(this@SessionFeed, true)
        try {
            emitAll(state)
        } finally {
            onWanted(this@SessionFeed, false)
        }
    }.stateIn(scope, SharingStarted.WhileSubscribed(detachDelay.inWholeMilliseconds), Feed.EMPTY)
}
