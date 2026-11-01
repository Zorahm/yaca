package com.yaca.android.chat.composer

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.InputChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.chat.fileNameOf
import com.yaca.core.model.QueuedId
import com.yaca.core.model.QueuedMessage

/**
 * Messages sent during a turn and not yet taken into it, in order. Each chip cancels its own
 * message when tapped; the cancel is the core's to honour, so the chip stays until the list changes.
 */
@Composable
fun QueuedChips(
    queued: List<QueuedMessage>,
    onCancel: (QueuedId) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (queued.isEmpty()) return
    Column(
        modifier.fillMaxWidth().padding(bottom = 8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            text = stringResource(R.string.queued_title),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            queued.forEach { message ->
                val label = message.text.lineSequence().firstOrNull { it.isNotBlank() }
                    ?: message.attachments.joinToString { fileNameOf(it) }
                InputChip(
                    selected = false,
                    onClick = { onCancel(message.id) },
                    label = { Text(text = label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    trailingIcon = {
                        Icon(
                            imageVector = Icons.Filled.Close,
                            contentDescription = stringResource(R.string.queued_cancel_desc),
                            modifier = Modifier.size(16.dp),
                        )
                    },
                )
            }
        }
    }
}

/** The files picked with `@`, each a chip that removes it before the message goes. */
@Composable
fun AttachmentChips(
    paths: List<String>,
    onRemove: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (paths.isEmpty()) return
    Row(
        modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(bottom = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        paths.forEach { path ->
            val name = fileNameOf(path)
            InputChip(
                selected = false,
                onClick = { onRemove(path) },
                label = { Text(text = name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                trailingIcon = {
                    Icon(
                        imageVector = Icons.Filled.Close,
                        contentDescription = stringResource(R.string.composer_remove, name),
                        modifier = Modifier.size(16.dp),
                    )
                },
            )
        }
    }
}
