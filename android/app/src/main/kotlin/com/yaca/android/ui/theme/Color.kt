package com.yaca.android.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color

// Fallback palette for Android 11 and older, where no dynamic colour exists. The values are
// Yaca's tokens from src/ui/tokens.css, mapped onto M3 roles; each comment names the token.
// Screens never read these values: they read roles from MaterialTheme.colorScheme.

internal val YacaLightColors: ColorScheme = lightColorScheme(
    primary = Color(0xFF0F6B6A),                 // --accent
    onPrimary = Color(0xFFFFFFFF),               // --on-accent
    primaryContainer = Color(0xFFDDEEEC),        // --accent-soft
    onPrimaryContainer = Color(0xFF0B5352),      // --accent-ink
    secondary = Color(0xFF5C646B),               // --muted
    onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = Color(0xFFE7ECE9),      // --thumb
    onSecondaryContainer = Color(0xFF1B2024),    // --ink
    tertiary = Color(0xFF2E6B45),                // --ok: a connection that is up, a tick
    onTertiary = Color(0xFFFFFFFF),
    tertiaryContainer = Color(0xFFE3F1E6),       // --add
    onTertiaryContainer = Color(0xFF2E6B45),     // --ok
    error = Color(0xFFA0302A),                   // --err
    onError = Color(0xFFFFFFFF),
    errorContainer = Color(0xFFF7E0DC),          // --del
    onErrorContainer = Color(0xFFA0302A),        // --err
    background = Color(0xFFF3F4F1),              // --bg
    onBackground = Color(0xFF1B2024),            // --ink
    surface = Color(0xFFFFFFFF),                 // --surface
    onSurface = Color(0xFF1B2024),               // --ink
    surfaceVariant = Color(0xFFF7F8F6),          // --sunk
    onSurfaceVariant = Color(0xFF5C646B),        // --muted
    surfaceContainerLowest = Color(0xFFFFFFFF),  // --surface
    surfaceContainerLow = Color(0xFFF7F8F6),     // --sunk
    surfaceContainer = Color(0xFFF7F8F6),        // --sunk
    surfaceContainerHigh = Color(0xFFE7ECE9),    // --thumb
    surfaceContainerHighest = Color(0xFFD6DBD8), // --line
    outline = Color(0xFFA3ADAA),                 // --line-strong
    outlineVariant = Color(0xFFD6DBD8),          // --line
)

internal val YacaDarkColors: ColorScheme = darkColorScheme(
    // The ink accent, not the fill: M3 draws text buttons and links in primary,
    // and the darker fill reads too dim as text on the night ground.
    primary = Color(0xFF8ED8D6),                 // --accent-ink
    onPrimary = Color(0xFF15181B),               // --bg
    primaryContainer = Color(0xFF1D3B3B),        // --accent-soft
    onPrimaryContainer = Color(0xFF8ED8D6),      // --accent-ink
    secondary = Color(0xFF9AA3A8),               // --muted
    onSecondary = Color(0xFF15181B),             // --bg
    secondaryContainer = Color(0xFF262D32),      // --thumb
    onSecondaryContainer = Color(0xFFE7EBE9),    // --ink
    tertiary = Color(0xFF6CC08C),                // --ok
    onTertiary = Color(0xFF15181B),              // --bg
    tertiaryContainer = Color(0xFF1F3A27),       // --add
    onTertiaryContainer = Color(0xFF6CC08C),     // --ok
    error = Color(0xFFF0867E),                   // --err
    onError = Color(0xFF15181B),                 // --bg
    errorContainer = Color(0xFF3D2421),          // --del
    onErrorContainer = Color(0xFFF0867E),        // --err
    background = Color(0xFF15181B),              // --bg
    onBackground = Color(0xFFE7EBE9),            // --ink
    surface = Color(0xFF1E2327),                 // --surface
    onSurface = Color(0xFFE7EBE9),               // --ink
    surfaceVariant = Color(0xFF1A1F23),          // --sunk
    onSurfaceVariant = Color(0xFF9AA3A8),        // --muted
    surfaceContainerLowest = Color(0xFF1E2327),  // --surface
    surfaceContainerLow = Color(0xFF1A1F23),     // --sunk
    surfaceContainer = Color(0xFF1A1F23),        // --sunk
    surfaceContainerHigh = Color(0xFF262D32),    // --thumb
    surfaceContainerHighest = Color(0xFF343C42), // --line
    outline = Color(0xFF5A656C),                 // --line-strong
    outlineVariant = Color(0xFF343C42),          // --line
)
