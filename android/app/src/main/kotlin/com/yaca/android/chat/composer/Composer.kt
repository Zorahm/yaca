package com.yaca.android.chat.composer

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.AlternateEmail
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.chat.ComposerActions
import com.yaca.android.chat.ComposerDraft
import com.yaca.core.feed.Feed
import com.yaca.core.model.ModelInfo

/**
 * The composer: queued messages above, then the field with its attachments and its action row, and
 * the chips of mode, model and effort under it.
 *
 * While a turn runs the Stop button is there. The field stays editable, and a message sent meanwhile
 * is queued by the core, so the send button still shows next to Stop once there is text to send.
 */
@Composable
fun Composer(
    feed: Feed,
    models: List<ModelInfo>,
    draft: ComposerDraft,
    actions: ComposerActions,
    modifier: Modifier = Modifier,
) {
    val busy = feed.state?.busy == true
    val canSend = draft.text.isNotBlank() || draft.attachments.isNotEmpty()
    Column(
        modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.background)
            .imePadding()
            .navigationBarsPadding()
            .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        QueuedChips(queued = feed.queued, onCancel = actions.onCancelQueued)
        Surface(
            modifier = Modifier.fillMaxWidth(),
            shape = MaterialTheme.shapes.medium,
            color = MaterialTheme.colorScheme.surface,
            border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        ) {
            Column(Modifier.padding(horizontal = 6.dp, vertical = 6.dp)) {
                AttachmentChips(paths = draft.attachments, onRemove = actions.onRemoveAttachment)
                TextField(
                    value = draft.text,
                    onValueChange = actions.onTextChange,
                    modifier = Modifier.fillMaxWidth(),
                    placeholder = {
                        Text(
                            text = stringResource(
                                if (busy) R.string.composer_placeholder_busy else R.string.composer_placeholder,
                            ),
                        )
                    },
                    maxLines = 6,
                    colors = composerFieldColors(),
                )
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    IconButton(onClick = actions.onMention) {
                        Icon(Icons.Filled.AlternateEmail, contentDescription = stringResource(R.string.composer_mention))
                    }
                    ContextRing(usage = feed.usage)
                    Spacer(Modifier.weight(1f))
                    if (busy) {
                        FilledIconButton(
                            onClick = actions.onStop,
                            colors = IconButtonDefaults.filledIconButtonColors(
                                containerColor = MaterialTheme.colorScheme.error,
                                contentColor = MaterialTheme.colorScheme.onError,
                            ),
                        ) {
                            Icon(Icons.Filled.Stop, contentDescription = stringResource(R.string.composer_stop))
                        }
                        Spacer(Modifier.width(8.dp))
                    }
                    if (!busy || canSend) {
                        FilledIconButton(onClick = actions.onSend, enabled = canSend) {
                            Icon(
                                imageVector = Icons.AutoMirrored.Filled.Send,
                                contentDescription = stringResource(
                                    if (busy) R.string.composer_queue else R.string.composer_send,
                                ),
                            )
                        }
                    }
                }
            }
        }
        ComposerChips(
            state = feed.state,
            models = models,
            actions = actions,
            modifier = Modifier.padding(top = 8.dp),
        )
    }
}

/** The field sits on the surface of its card: no fill and no underline of its own. */
@Composable
private fun composerFieldColors() = TextFieldDefaults.colors(
    focusedContainerColor = Color.Transparent,
    unfocusedContainerColor = Color.Transparent,
    disabledContainerColor = Color.Transparent,
    focusedIndicatorColor = Color.Transparent,
    unfocusedIndicatorColor = Color.Transparent,
    disabledIndicatorColor = Color.Transparent,
)
