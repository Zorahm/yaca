package com.yaca.android.ui.common

import android.provider.Settings
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.core.client.ConnectionState

/** The one-line state of a host's connection, as the desktop words it. */
@Composable
fun connectionLabel(state: ConnectionState): String = when (state) {
    ConnectionState.Idle -> stringResource(R.string.connection_idle)
    is ConnectionState.Connecting -> stringResource(R.string.connection_connecting, state.attempt)
    is ConnectionState.Connected -> stringResource(R.string.connection_online, state.host.version)
    is ConnectionState.Refused -> stringResource(R.string.connection_refused)
    is ConnectionState.Lost -> stringResource(R.string.connection_offline)
}

/** Connection colours use the tertiary role (up) and the error role (refused); they never compete with a request. */
@Composable
fun connectionColor(state: ConnectionState): Color = when (state) {
    is ConnectionState.Connected -> MaterialTheme.colorScheme.tertiary
    is ConnectionState.Refused -> MaterialTheme.colorScheme.error
    else -> MaterialTheme.colorScheme.onSurfaceVariant
}

@Composable
fun ConnectionDot(color: Color, modifier: Modifier = Modifier) {
    Box(modifier.size(8.dp).background(color, CircleShape))
}

/**
 * A running chat pulses; an idle one stays still. A system with animations turned off gets
 * the still dot too: the pulse is a state signal, not decoration.
 */
@Composable
fun BusyDot(busy: Boolean, modifier: Modifier = Modifier) {
    val accent = MaterialTheme.colorScheme.primary
    if (busy && animationsEnabled()) {
        val transition = rememberInfiniteTransition(label = "busy")
        val pulse by transition.animateFloat(
            initialValue = 0.3f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(
                animation = tween(durationMillis = 900),
                repeatMode = RepeatMode.Reverse,
            ),
            label = "busy-pulse",
        )
        Box(modifier.size(10.dp).alpha(pulse).background(accent, CircleShape))
    } else {
        val color = if (busy) accent else MaterialTheme.colorScheme.onSurfaceVariant
        Box(modifier.size(10.dp).background(color, CircleShape))
    }
}

@Composable
private fun animationsEnabled(): Boolean {
    val resolver = LocalContext.current.contentResolver
    return remember(resolver) {
        Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) > 0f
    }
}
