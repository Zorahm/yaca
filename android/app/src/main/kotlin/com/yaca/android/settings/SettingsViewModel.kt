package com.yaca.android.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.yaca.android.AppContainer
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

/** The settings screen's state is the container's settings; the view model only writes them back to the store. */
class SettingsViewModel(private val container: AppContainer) : ViewModel() {
    val settings: StateFlow<AppSettings> = container.settings

    fun setStayConnected(value: Boolean) = write { setStayConnected(value) }

    fun setNotifyTurnFinished(value: Boolean) = write { setNotifyTurnFinished(value) }

    fun setWallpaperColours(value: Boolean) = write { setWallpaperColours(value) }

    private fun write(change: suspend SettingsStore.() -> Unit) {
        viewModelScope.launch { container.settingsStore.change() }
    }
}
