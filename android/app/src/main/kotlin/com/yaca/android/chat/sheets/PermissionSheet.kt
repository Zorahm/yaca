@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.chat.sheets
import androidx.compose.material3.ExperimentalMaterial3Api

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.ui.theme.LocalCodeStyle
import com.yaca.core.model.PermissionAnswer
import com.yaca.core.model.PermissionAnswerAllow
import com.yaca.core.model.PermissionAnswerAllowAlways
import com.yaca.core.model.PermissionAnswerDeny
import com.yaca.core.model.PermissionRequest

/**
 * The agent asks to do something: what, where, why, and which reach of "always" is offered. "Only
 * this time" is picked; the broadest choice never is. Deny and Allow are the only two buttons.
 *
 * [reason] is the caption the agent wrote for the call; the request itself carries none.
 */
@Composable
fun PermissionSheet(
    request: PermissionRequest,
    reason: String?,
    onAnswer: (PermissionAnswer) -> Unit,
    onDismiss: () -> Unit,
) {
    val scopes = request.scopes
    // 0 is "only this time"; n is scopes[n - 1]. Preset to 0, so an "always" is never chosen by accident.
    var choice by rememberSaveable(request.id.value) { mutableStateOf(0) }
    val picked = choice.coerceIn(0, scopes.size)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = 20.dp, end = 20.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            SheetKicker(stringResource(R.string.permission_kicker))
            Text(
                text = kindLabel(request.kind),
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurface,
            )
            if (request.total > 1) {
                Text(
                    text = stringResource(R.string.permission_position, request.position, request.total),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            // The whole target, never truncated: this is where a chained `&& rm -rf` shows.
            Surface(
                modifier = Modifier.fillMaxWidth(),
                shape = MaterialTheme.shapes.small,
                color = MaterialTheme.colorScheme.surfaceVariant,
            ) {
                Text(
                    text = request.detail,
                    style = LocalCodeStyle.current,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.padding(10.dp),
                )
            }
            request.note?.let { note ->
                Text(
                    text = note,
                    style = LocalCodeStyle.current,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            request.server?.let { server ->
                Text(
                    text = stringResource(R.string.permission_server, server),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            reason?.takeIf { it.isNotBlank() }?.let { why ->
                Text(
                    text = stringResource(R.string.permission_why, why),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
            request.invariant?.let { invariant ->
                Text(
                    text = invariantLabel(invariant),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
            request.unsaved?.let { unsaved ->
                Text(
                    text = stringResource(R.string.permission_unsaved, unsaved.files, unsaved.dirCount),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                )
            }

            ChoiceRow(
                selected = picked == 0,
                title = stringResource(R.string.permission_scope_once),
                hint = stringResource(R.string.permission_scope_once_hint),
                onSelect = { choice = 0 },
            )
            scopes.forEachIndexed { index, scope ->
                ChoiceRow(
                    selected = picked == index + 1,
                    title = stringResource(R.string.scope_always, scopeLabel(scope)),
                    hint = null,
                    onSelect = { choice = index + 1 },
                )
            }

            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(onClick = { onAnswer(PermissionAnswerDeny()) }) {
                    Text(stringResource(R.string.permission_deny), color = MaterialTheme.colorScheme.error)
                }
                Spacer(Modifier.width(8.dp))
                Button(
                    onClick = {
                        onAnswer(
                            if (picked == 0) {
                                PermissionAnswerAllow
                            } else {
                                PermissionAnswerAllowAlways(scopes[picked - 1])
                            },
                        )
                    },
                ) {
                    Text(stringResource(R.string.permission_allow))
                }
            }
        }
    }
}
