package com.yaca.android

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelProvider.AndroidViewModelFactory.Companion.APPLICATION_KEY
import androidx.lifecycle.viewmodel.CreationExtras

/**
 * A factory that builds a ViewModel from the process's [AppContainer]; anything else the
 * ViewModel needs (a fingerprint, a device name) is captured by [build] at the call site.
 */
fun <VM : ViewModel> appViewModelFactory(build: (AppContainer) -> VM): ViewModelProvider.Factory =
    object : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>, extras: CreationExtras): T {
            val app = extras[APPLICATION_KEY] as? YacaApplication
                ?: error("Yaca ViewModels are created inside YacaApplication")
            // The factory is only ever asked for the VM type it was built for, so the cast holds.
            return build(app.container) as T
        }
    }
