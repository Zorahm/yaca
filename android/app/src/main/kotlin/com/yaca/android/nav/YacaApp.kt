package com.yaca.android.nav

import android.app.Activity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import com.yaca.android.AppContainer
import com.yaca.android.attention.ChatKey
import com.yaca.android.chat.ChatScreen
import com.yaca.android.host.HostRoute
import com.yaca.android.hosts.HostsRoute
import com.yaca.android.notify.NotificationPrompt
import com.yaca.android.pairing.PairingRoute
import com.yaca.android.settings.SettingsRoute
import com.yaca.core.model.SessionId

/** The root: waits for the first answer of the store, then picks where the stack starts. */
@Composable
fun YacaApp(container: AppContainer) {
    val hosts by container.hosts.collectAsStateWithLifecycle()
    val incomingLink by container.incomingLink.collectAsStateWithLifecycle()
    val openChat by container.openChat.collectAsStateWithLifecycle()
    val loaded = hosts
    if (loaded == null) {
        // The store answers within a frame or two; until then nothing is drawn but the ground.
        Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background))
        return
    }
    YacaNavHost(
        container = container,
        startAtPairing = loaded.isEmpty(),
        incomingLink = incomingLink,
        openChat = openChat,
    )
    NotificationPrompt(container = container)
}

@Composable
fun YacaNavHost(container: AppContainer, startAtPairing: Boolean, incomingLink: String?, openChat: ChatKey?) {
    val navController = rememberNavController()
    // Decided once per stack: a computer paired later must not move the start under the human.
    val startsAtPairing = rememberSaveable { startAtPairing }

    LaunchedEffect(incomingLink) {
        if (incomingLink != null) navController.navigate(Pairing) { launchSingleTop = true }
    }

    // A notification's chat: opened on top of whatever the human had, so Back returns to it.
    LaunchedEffect(openChat) {
        val key = openChat ?: return@LaunchedEffect
        container.takeOpenChat()
        navController.navigate(Chat(key.fingerprint, key.session.value)) { launchSingleTop = true }
    }

    if (startsAtPairing) {
        NavHost(navController = navController, startDestination = Pairing) {
            yacaGraph(container, navController)
        }
    } else {
        NavHost(navController = navController, startDestination = Hosts) {
            yacaGraph(container, navController)
        }
    }
}

private fun NavGraphBuilder.yacaGraph(container: AppContainer, navController: NavHostController) {
    composable<Hosts> {
        HostsRoute(
            onPair = { navController.navigate(Pairing) },
            onOpenHost = { fingerprint -> navController.navigate(Host(fingerprint)) },
            onOpenSettings = { navController.navigate(Settings) },
        )
    }

    composable<Settings> {
        SettingsRoute(onBack = { navController.popBackStack() })
    }

    composable<Pairing> {
        val context = LocalContext.current
        PairingRoute(
            onPaired = { fingerprint ->
                navController.navigate(Host(fingerprint)) { popUpTo<Pairing> { inclusive = true } }
            },
            // Pairing can be the start of the stack (no computers yet): back then leaves the app.
            onBack = {
                if (!navController.popBackStack()) (context as? Activity)?.finish()
            },
        )
    }

    composable<Host> { entry ->
        val route = entry.toRoute<Host>()
        HostRoute(
            fingerprint = route.fingerprint,
            onBack = { navController.popBackStack() },
            onPairAgain = { navController.navigate(Pairing) },
            onOpenChat = { session -> navController.navigate(Chat(route.fingerprint, session)) },
        )
    }

    composable<Chat> { entry ->
        val route = entry.toRoute<Chat>()
        val key = ChatKey(route.fingerprint, SessionId(route.session))
        // While the chat is on screen it is not notified about; opening it answers its attention.
        DisposableEffect(key) {
            container.onChatShown(key)
            onDispose { container.onChatHidden(key) }
        }
        ChatScreen(
            fingerprint = route.fingerprint,
            session = route.session,
            onBack = { navController.popBackStack() },
        )
    }
}
