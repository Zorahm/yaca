package com.yaca.android.settings

import android.content.Context
import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.preferencesDataStore
import java.io.IOException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map

private val Context.settingsDataStore: DataStore<Preferences> by preferencesDataStore(name = "yaca_settings")
private val STAY_CONNECTED = booleanPreferencesKey("stay_connected")
private val NOTIFY_TURN_FINISHED = booleanPreferencesKey("notify_turn_finished")
private val WALLPAPER_COLOURS = booleanPreferencesKey("wallpaper_colours")
private val NOTIFICATION_PROMPT_DONE = booleanPreferencesKey("notification_prompt_done")
private const val TAG = "SettingsStore"

/**
 * What the human chose in the settings screen, and one flag the app keeps for itself. The defaults
 * are the values a fresh install has; they are also what an unreadable store reads as.
 */
data class AppSettings(
    /** Keep the computers connected while the app is in the background, through the connection service. */
    val stayConnected: Boolean = true,
    /** Post a quiet notification when a chat finishes its turn. */
    val notifyTurnFinished: Boolean = true,
    /** Material You: the colours follow the wallpaper. Read only from Android 12 on. */
    val wallpaperColours: Boolean = true,
    /** The notification permission was asked for, or the prompt was declined: it is not asked again. */
    val notificationPromptDone: Boolean = false,
)

/** The settings in DataStore Preferences. Plain booleans: nothing here is secret, unlike the host records. */
class SettingsStore(context: Context) {
    private val dataStore = context.applicationContext.settingsDataStore

    /** The stored settings. A store that cannot be read yields the defaults and a logged warning; it never throws. */
    val settings: Flow<AppSettings> = dataStore.data
        .catch { cause ->
            if (cause !is IOException) throw cause
            Log.w(TAG, "The settings could not be read; using the defaults (${cause.javaClass.simpleName})")
            emit(emptyPreferences())
        }
        .map { prefs ->
            val defaults = AppSettings()
            AppSettings(
                stayConnected = prefs[STAY_CONNECTED] ?: defaults.stayConnected,
                notifyTurnFinished = prefs[NOTIFY_TURN_FINISHED] ?: defaults.notifyTurnFinished,
                wallpaperColours = prefs[WALLPAPER_COLOURS] ?: defaults.wallpaperColours,
                notificationPromptDone = prefs[NOTIFICATION_PROMPT_DONE] ?: defaults.notificationPromptDone,
            )
        }
        .distinctUntilChanged()

    suspend fun setStayConnected(value: Boolean) = write(STAY_CONNECTED, value)

    suspend fun setNotifyTurnFinished(value: Boolean) = write(NOTIFY_TURN_FINISHED, value)

    suspend fun setWallpaperColours(value: Boolean) = write(WALLPAPER_COLOURS, value)

    suspend fun markNotificationPromptDone() = write(NOTIFICATION_PROMPT_DONE, true)

    private suspend fun write(key: Preferences.Key<Boolean>, value: Boolean) {
        dataStore.edit { prefs -> prefs[key] = value }
    }
}
