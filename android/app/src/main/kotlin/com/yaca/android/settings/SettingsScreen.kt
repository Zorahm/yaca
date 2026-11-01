@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.settings

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.yaca.android.R
import com.yaca.android.appViewModelFactory
import com.yaca.android.ui.common.SectionLabel
import com.yaca.android.ui.theme.YacaTheme

/** The route: the settings, from the view model. The version is the installed app's, read once. */
@Composable
fun SettingsRoute(onBack: () -> Unit) {
    val vm: SettingsViewModel = viewModel(factory = appViewModelFactory { SettingsViewModel(it) })
    val settings by vm.settings.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val version = remember(context) { installedVersion(context) }
    SettingsScreen(
        settings = settings,
        version = version,
        onBack = onBack,
        onStayConnected = vm::setStayConnected,
        onNotifyTurnFinished = vm::setNotifyTurnFinished,
        onWallpaperColours = vm::setWallpaperColours,
        onOpenSystemNotifications = {
            // The app's own notification page; Android has no narrower screen for Yaca's channels.
            context.startActivity(
                Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                    .putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName),
            )
        },
    )
}

/** Stateless: the rows, drawn from [settings]. Every change goes back through its callback. */
@Composable
fun SettingsScreen(
    settings: AppSettings,
    version: String,
    onBack: () -> Unit,
    onStayConnected: (Boolean) -> Unit,
    onNotifyTurnFinished: (Boolean) -> Unit,
    onWallpaperColours: (Boolean) -> Unit,
    onOpenSystemNotifications: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Scaffold(
        modifier = modifier,
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.action_back))
                    }
                },
            )
        },
    ) { padding ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
        ) {
            item { SectionLabel(stringResource(R.string.settings_section_connection)) }
            item {
                SwitchRow(
                    title = stringResource(R.string.settings_stay_connected),
                    description = stringResource(R.string.settings_stay_connected_desc),
                    checked = settings.stayConnected,
                    onCheckedChange = onStayConnected,
                )
            }

            item { SectionLabel(stringResource(R.string.settings_section_notifications)) }
            item {
                SwitchRow(
                    title = stringResource(R.string.settings_turn_finished),
                    description = stringResource(R.string.settings_turn_finished_desc),
                    checked = settings.notifyTurnFinished,
                    onCheckedChange = onNotifyTurnFinished,
                )
            }
            item {
                ListItem(
                    headlineContent = { Text(stringResource(R.string.settings_system_notifications)) },
                    supportingContent = { Text(stringResource(R.string.settings_system_notifications_desc)) },
                    modifier = Modifier.clickable(onClick = onOpenSystemNotifications),
                )
            }

            // Wallpaper colours exist only from Android 12; below it the row would change nothing.
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                item { SectionLabel(stringResource(R.string.settings_section_appearance)) }
                item {
                    SwitchRow(
                        title = stringResource(R.string.settings_wallpaper_colours),
                        description = stringResource(R.string.settings_wallpaper_colours_desc),
                        checked = settings.wallpaperColours,
                        onCheckedChange = onWallpaperColours,
                    )
                }
            }

            item { SectionLabel(stringResource(R.string.settings_section_about)) }
            item {
                ListItem(headlineContent = { Text(stringResource(R.string.settings_version, version)) })
            }
        }
    }
}

/** A row with a switch; the whole row is the target, as the switch alone is small to hit on a phone. */
@Composable
private fun SwitchRow(
    title: String,
    description: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = { Text(description) },
        trailingContent = { Switch(checked = checked, onCheckedChange = null) },
        modifier = Modifier.toggleable(value = checked, role = Role.Switch, onValueChange = onCheckedChange),
    )
}

/** The version name from the package manager; empty only if the package is missing, which cannot happen for the running app. */
private fun installedVersion(context: Context): String =
    try {
        context.packageManager.getPackageInfo(context.packageName, 0).versionName.orEmpty()
    } catch (missing: PackageManager.NameNotFoundException) {
        ""
    }

@Preview(showBackground = true)
@Composable
private fun SettingsPreview() {
    YacaTheme {
        SettingsScreen(
            settings = AppSettings(),
            version = "1.0.0-beta",
            onBack = {},
            onStayConnected = {},
            onNotifyTurnFinished = {},
            onWallpaperColours = {},
            onOpenSystemNotifications = {},
        )
    }
}
