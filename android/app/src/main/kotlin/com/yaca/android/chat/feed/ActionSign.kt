package com.yaca.android.chat.feed

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Block
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.ui.common.BusyDot
import com.yaca.core.feed.ActionRow
import com.yaca.core.feed.ActionState

/**
 * The mark at the left of an action row, the one place a row's state is drawn: a pulse while it
 * runs, a tick when done, and its own shape for each way it did not finish. A state never rests on
 * colour alone, and each mark has a description for the screen reader.
 */
@Composable
fun ActionSign(row: ActionRow, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Box(modifier.size(20.dp), contentAlignment = Alignment.Center) {
        when (row.state) {
            ActionState.RUNNING -> {
                val description = stringResource(R.string.row_status_running)
                BusyDot(
                    busy = true,
                    modifier = Modifier.semantics { contentDescription = description },
                )
            }
            ActionState.DONE -> SignGlyph(Icons.Filled.Check, colors.tertiary, stringResource(R.string.row_status_done))
            ActionState.FAILED -> SignGlyph(Icons.Filled.Close, colors.error, stringResource(R.string.row_status_failed))
            ActionState.REJECTED -> SignGlyph(
                Icons.Filled.Block,
                colors.onSurfaceVariant,
                stringResource(R.string.row_status_rejected),
            )
            ActionState.DENIED -> SignGlyph(
                Icons.Filled.Block,
                colors.onSurfaceVariant,
                stringResource(R.string.row_status_denied),
            )
            ActionState.STOPPED -> SignGlyph(
                Icons.Filled.Stop,
                colors.onSurfaceVariant,
                stringResource(R.string.row_status_stopped),
            )
        }
    }
}

@Composable
private fun SignGlyph(icon: ImageVector, tint: Color, description: String) {
    Icon(
        imageVector = icon,
        contentDescription = description,
        tint = tint,
        modifier = Modifier.size(16.dp),
    )
}
