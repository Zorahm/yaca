package com.yaca.android.chat.composer

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.chat.formatTokens
import com.yaca.core.model.UsageSnapshot

/**
 * How full the model's context window is, as a ring. A chat with no usage yet, or a model whose
 * window size is unknown, shows an empty ring and says so: a guess is not drawn as a fact.
 */
@Composable
fun ContextRing(usage: UsageSnapshot?, modifier: Modifier = Modifier) {
    val known = usage != null && usage.contextMax > 0
    val fraction = if (usage != null && known) {
        (usage.contextUsed.toFloat() / usage.contextMax).coerceIn(0f, 1f)
    } else {
        0f
    }
    val description = if (usage != null && known) {
        stringResource(
            R.string.context_used,
            formatTokens(usage.contextUsed.toLong()),
            formatTokens(usage.contextMax.toLong()),
        )
    } else {
        stringResource(R.string.context_unknown)
    }
    val track = MaterialTheme.colorScheme.outlineVariant
    val fill = MaterialTheme.colorScheme.primary
    Canvas(modifier.size(32.dp).semantics { contentDescription = description }) {
        val stroke = 3.dp.toPx()
        val inset = stroke / 2
        val arc = Size(size.width - stroke, size.height - stroke)
        drawArc(
            color = track,
            startAngle = -90f,
            sweepAngle = 360f,
            useCenter = false,
            topLeft = Offset(inset, inset),
            size = arc,
            style = Stroke(width = stroke),
        )
        if (known && fraction > 0f) {
            drawArc(
                color = fill,
                startAngle = -90f,
                sweepAngle = 360f * fraction,
                useCenter = false,
                topLeft = Offset(inset, inset),
                size = arc,
                style = Stroke(width = stroke, cap = StrokeCap.Round),
            )
        }
    }
}
