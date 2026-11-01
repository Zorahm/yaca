package com.yaca.android.notify

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.yaca.android.AppContainer
import com.yaca.android.R
import kotlinx.coroutines.launch

/**
 * Asks once for the notification permission (Android 13+), with the reason. It shows when the first
 * computer appears in the list: right after the first pairing, or at the first launch of this version
 * for someone who was already paired. Either answer ends the prompt for good; a permission denied here
 * is changed in the system settings, which the settings screen links.
 */
@Composable
fun NotificationPrompt(container: AppContainer) {
    val settings by container.settings.collectAsStateWithLifecycle()
    val hosts by container.hosts.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }

    val due = !settings.notificationPromptDone &&
        hosts?.isNotEmpty() == true &&
        notificationPermissionMissing(context)
    if (!due) return

    fun finish() {
        scope.launch { container.settingsStore.markNotificationPromptDone() }
    }

    AlertDialog(
        onDismissRequest = ::finish,
        title = { Text(stringResource(R.string.notify_prompt_title)) },
        text = { Text(stringResource(R.string.notify_prompt_body)) },
        confirmButton = {
            TextButton(onClick = {
                finish()
                launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
            }) {
                Text(stringResource(R.string.notify_prompt_allow))
            }
        },
        dismissButton = {
            TextButton(onClick = ::finish) {
                Text(stringResource(R.string.notify_prompt_later))
            }
        },
    )
}
