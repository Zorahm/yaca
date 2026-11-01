package com.yaca.android.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/** The M3 type scale as it is: the phone's system sans already is the sans of the desktop's fallback stack. */
val YacaTypography: Typography = Typography()

/** Monospace for code, output and paths, at the desktop's mono-code size. */
val YacaCodeStyle: TextStyle = TextStyle(
    fontFamily = FontFamily.Monospace,
    fontWeight = FontWeight.Normal,
    fontSize = 12.5.sp,
    lineHeight = 20.sp,
)

/** Read by composables that show code: `LocalCodeStyle.current`. */
val LocalCodeStyle = staticCompositionLocalOf { YacaCodeStyle }
