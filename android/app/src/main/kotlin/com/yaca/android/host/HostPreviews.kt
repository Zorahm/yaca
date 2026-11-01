@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.host

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.tooling.preview.Preview
import com.yaca.android.ui.theme.YacaTheme
import com.yaca.core.client.ConnectionState
import com.yaca.core.protocol.HostInfo
import com.yaca.core.protocol.RefusalUnauthorized

private val sampleRunning = listOf(
    RunningChat(session = "s1", title = "Refactor the parser", project = "yaca", busy = true, waiting = false),
    RunningChat(session = "s2", title = "Migrate settings", project = "notes-api", busy = false, waiting = true),
)

@Preview(showBackground = true)
@Composable
private fun HostPreview() {
    YacaTheme {
        HostScreen(
            state = HostUiState.Ready(
                name = "ZORAH-PC",
                connection = ConnectionState.Connected(
                    host = HostInfo(name = "ZORAH-PC", version = "1.0.0-beta", protocol = 1),
                    address = "192.168.0.10",
                ),
                running = sampleRunning,
                projects = ProjectsState.Loaded(listOf("C:\\work\\yaca", "C:\\work\\notes-api")),
                expanded = mapOf("C:\\work\\yaca" to SessionsState.Loading),
                refreshing = false,
                opening = false,
                newChatOpen = false,
                forgetting = false,
            ),
            snackbarHostState = remember { SnackbarHostState() },
            onBack = {},
            onRefresh = {},
            onPairAgain = {},
            onOpenRunning = {},
            onToggleProject = {},
            onOpenRecorded = { _, _ -> },
            onShowNewChat = {},
            onDismissNewChat = {},
            onOpenNewChat = {},
            onAskForget = {},
            onDismissForget = {},
            onConfirmForget = {},
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun HostRefusedPreview() {
    YacaTheme {
        HostScreen(
            state = HostUiState.Ready(
                name = "NOTES-PC",
                connection = ConnectionState.Refused(RefusalUnauthorized),
                running = emptyList(),
                projects = ProjectsState.Unavailable,
                expanded = emptyMap(),
                refreshing = false,
                opening = false,
                newChatOpen = false,
                forgetting = false,
            ),
            snackbarHostState = remember { SnackbarHostState() },
            onBack = {},
            onRefresh = {},
            onPairAgain = {},
            onOpenRunning = {},
            onToggleProject = {},
            onOpenRecorded = { _, _ -> },
            onShowNewChat = {},
            onDismissNewChat = {},
            onOpenNewChat = {},
            onAskForget = {},
            onDismissForget = {},
            onConfirmForget = {},
        )
    }
}
