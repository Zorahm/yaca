package com.yaca.android.chat

import androidx.annotation.PluralsRes
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.yaca.android.R
import java.text.DateFormat
import java.text.NumberFormat
import java.util.Date
import java.util.Locale

/** A count in words, with the phone's plural rules ("1 file", "3 files", Russian has four forms). */
@Composable
fun pluralCount(@PluralsRes id: Int, count: Int): String =
    LocalContext.current.resources.getQuantityString(id, count, count)

/** A duration as the feed writes it: "42 s" or "3 min 5 s". */
@Composable
fun formatDuration(millis: Long): String {
    val seconds = millis.coerceAtLeast(0L) / 1000
    return if (seconds < 60) {
        stringResource(R.string.duration_sec, seconds)
    } else {
        stringResource(R.string.duration_min_sec, seconds / 60, seconds % 60)
    }
}

/** A token count with the grouping of the phone's locale. Numbers are not translated, only grouped. */
fun formatTokens(count: Long): String = NumberFormat.getIntegerInstance(Locale.getDefault()).format(count)

/** The time of day of an instant, in the phone's own short form. */
fun formatClock(epochMillis: Long): String =
    DateFormat.getTimeInstance(DateFormat.SHORT, Locale.getDefault()).format(Date(epochMillis))

/** The last component of a path, for both separators: the name a chip or a caption shows. */
fun fileNameOf(path: String): String {
    val trimmed = path.trimEnd('/', '\\')
    return trimmed.substringAfterLast('/').substringAfterLast('\\').ifEmpty { path }
}
