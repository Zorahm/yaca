package com.yaca.android.ui.common

import android.text.format.DateUtils
import java.text.NumberFormat
import java.util.Locale

/** The last folder of a project path, for the lists: `C:\work\yaca` shows as `yaca`. */
fun projectShortName(project: String): String {
    val trimmed = project.trimEnd('/', '\\')
    return trimmed.substringAfterLast('/').substringAfterLast('\\').ifEmpty { project }
}

/** A chat's cost in US dollars, as the desktop shows it. */
fun formatCostUsd(usd: Double): String = NumberFormat.getCurrencyInstance(Locale.US).format(usd)

/** "12 min ago", in the phone's language (the system formats it, so no string of ours is needed). */
fun relativeTime(epochMillis: Long, now: Long = System.currentTimeMillis()): String =
    DateUtils.getRelativeTimeSpanString(
        epochMillis,
        now,
        DateUtils.MINUTE_IN_MILLIS,
        DateUtils.FORMAT_ABBREV_RELATIVE,
    ).toString()
