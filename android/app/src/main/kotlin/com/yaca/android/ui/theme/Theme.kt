package com.yaca.android.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalContext

/**
 * Yaca's theme for the phone. From Android 12 the colours follow the wallpaper (Material You)
 * when [dynamic] is set; below that, and when it is not, Yaca's own tokens.
 */
@Composable
fun YacaTheme(dynamic: Boolean = true, content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val context = LocalContext.current
    val colors = when {
        dynamic && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
            if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        dark -> YacaDarkColors
        else -> YacaLightColors
    }
    CompositionLocalProvider(LocalCodeStyle provides YacaCodeStyle) {
        MaterialTheme(
            colorScheme = colors,
            typography = YacaTypography,
            shapes = YacaShapes,
            content = content,
        )
    }
}
