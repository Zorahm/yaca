package com.yaca.android.attention

import com.yaca.core.model.SessionId

/**
 * One chat on one computer. The host fingerprint and the session id together name the chat
 * the same way on every screen and in every notification.
 */
data class ChatKey(val fingerprint: String, val session: SessionId) {
    /**
     * The notification tag. One notification per chat: the next event of the chat replaces
     * the one before it, rather than stacking up.
     */
    val tag: String get() = "$fingerprint/${session.value}"
}
