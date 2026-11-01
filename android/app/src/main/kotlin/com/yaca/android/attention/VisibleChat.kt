package com.yaca.android.attention

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update

/**
 * The chat the human sees: its screen is shown and the app is in the foreground. A chat on
 * screen is not notified about, because the human reads it there. Leaving the app makes no
 * chat visible, even one whose screen was still composed when the app went to the background.
 */
class VisibleChat {
    private val shown = MutableStateFlow<ChatKey?>(null)
    private val foreground = MutableStateFlow(false)

    /** The chat screen for [key] came up. */
    fun show(key: ChatKey) {
        shown.value = key
    }

    /**
     * The chat screen for [key] went away. Compose may dispose the old screen after the new one
     * came up, so only the chat that is still recorded is cleared.
     */
    fun hide(key: ChatKey) {
        shown.update { current -> if (current == key) null else current }
    }

    fun setForeground(value: Boolean) {
        foreground.value = value
    }

    fun isOnScreen(key: ChatKey): Boolean = foreground.value && shown.value == key
}
