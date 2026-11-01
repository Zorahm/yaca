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
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.core.model.QuestionReply
import com.yaca.core.model.QuestionReplyAnswered
import com.yaca.core.model.QuestionReplyDismissed
import com.yaca.core.model.QuestionRequest

/**
 * A question from the agent: its options with what each one means, an "Other" line when the request
 * allows it, and two ways out. Skip answers with "dismissed"; the agent is told the human chose not to.
 * Options are radios for one answer and checkboxes when several may be picked.
 */
@Composable
fun QuestionSheet(
    request: QuestionRequest,
    onAnswer: (QuestionReply) -> Unit,
    onDismiss: () -> Unit,
) {
    val otherIndex = request.options.size
    val picked = rememberSaveable(request.id.value) { mutableStateListOf<Int>() }
    var other by rememberSaveable(request.id.value) { mutableStateOf("") }
    val otherOn = otherIndex in picked
    val answerable = picked.isNotEmpty() && (!otherOn || other.isNotBlank())

    fun pick(index: Int) {
        if (request.multiSelect) {
            if (index in picked) picked.remove(index) else picked.add(index)
        } else {
            picked.clear()
            picked.add(index)
        }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = 20.dp, end = 20.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            SheetKicker(stringResource(R.string.question_block_title))
            Text(
                text = request.question,
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            if (request.multiSelect) {
                Text(
                    text = stringResource(R.string.question_pick_many),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            request.options.forEachIndexed { index, option ->
                ChoiceRow(
                    selected = index in picked,
                    title = option.label,
                    hint = option.description,
                    onSelect = { pick(index) },
                    multi = request.multiSelect,
                )
            }
            if (request.allowOther) {
                ChoiceRow(
                    selected = otherOn,
                    title = stringResource(R.string.question_other),
                    hint = null,
                    onSelect = { pick(otherIndex) },
                    multi = request.multiSelect,
                )
                if (otherOn) {
                    OutlinedTextField(
                        value = other,
                        onValueChange = { other = it },
                        modifier = Modifier.fillMaxWidth(),
                        placeholder = { Text(stringResource(R.string.question_other_hint)) },
                        maxLines = 3,
                    )
                }
            }

            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(onClick = { onAnswer(QuestionReplyDismissed) }) {
                    Text(stringResource(R.string.question_skip))
                }
                Spacer(Modifier.width(8.dp))
                Button(
                    enabled = answerable,
                    onClick = {
                        val text = other.trim()
                        onAnswer(
                            QuestionReplyAnswered(
                                selected = picked.filter { it < otherIndex },
                                other = text.takeIf { otherOn && it.isNotEmpty() },
                            ),
                        )
                    },
                ) {
                    Text(stringResource(R.string.question_submit))
                }
            }
        }
    }
}
