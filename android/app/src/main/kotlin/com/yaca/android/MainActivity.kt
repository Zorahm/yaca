package com.yaca.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.yaca.android.nav.YacaApp
import com.yaca.android.notify.ChatIntents
import com.yaca.android.ui.theme.YacaTheme

/** The only activity. Screens are Compose destinations; the intent only hands over a pairing link or a chat to show. */
class MainActivity : ComponentActivity() {
    private val container: AppContainer
        get() = (application as YacaApplication).container

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // A recreated activity (rotation) gets the same intent again; the link or chat was already taken.
        if (savedInstanceState == null) offerIntent(intent)
        setContent {
            val settings by container.settings.collectAsStateWithLifecycle()
            YacaTheme(dynamic = settings.wallpaperColours) {
                YacaApp(container = container)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        offerIntent(intent)
    }

    /** A pairing link from another app, or a chat a notification asks to show; anything else is not ours. */
    private fun offerIntent(intent: Intent?) {
        if (intent == null) return
        when (intent.action) {
            Intent.ACTION_VIEW -> intent.dataString?.let { container.offerIncomingLink(it) }
            ChatIntents.ACTION_OPEN_CHAT -> ChatIntents.chatKey(intent)?.let { container.offerOpenChat(it) }
        }
    }
}
