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
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.core.model.ElicitationReply
import com.yaca.core.model.ElicitationReplyAccept
import com.yaca.core.model.ElicitationReplyCancel
import com.yaca.core.model.ElicitationReplyDecline
import com.yaca.core.model.ElicitationRequest

/**
 * An MCP server's form. Decline refuses to give anything, Cancel leaves the server to decide; Submit
 * sends what is filled in and checks it first, showing what is wrong only after the first try. The
 * typed values survive a rotation through the sheet's own key.
 */
@Composable
fun ElicitationSheet(
    request: ElicitationRequest,
    onReply: (ElicitationReply) -> Unit,
    onDismiss: () -> Unit,
) {
    val typed = remember(request.id.value) {
        mutableStateMapOf<String, String>().apply { putAll(elicitInitial(request.fields)) }
    }
    var attempted by rememberSaveable(request.id.value) { mutableStateOf(false) }
    val problems = elicitProblems(request.fields, typed)

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
            SheetKicker(stringResource(R.string.elicit_title, request.server))
            Text(
                text = request.message,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            request.fields.forEach { field ->
                ElicitFieldView(
                    field = field,
                    value = typed[field.name].orEmpty(),
                    problem = if (attempted) problems[field.name] else null,
                    onChange = { typed[field.name] = it },
                )
            }

            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(onClick = { onReply(ElicitationReplyDecline) }) {
                    Text(stringResource(R.string.elicit_decline))
                }
                TextButton(onClick = { onReply(ElicitationReplyCancel) }) {
                    Text(stringResource(R.string.elicit_cancel))
                }
                Spacer(Modifier.width(8.dp))
                Button(
                    onClick = {
                        if (problems.isEmpty()) {
                            onReply(ElicitationReplyAccept(elicitValues(request.fields, typed)))
                        } else {
                            attempted = true
                        }
                    },
                ) {
                    Text(stringResource(R.string.elicit_submit))
                }
            }
        }
    }
}
