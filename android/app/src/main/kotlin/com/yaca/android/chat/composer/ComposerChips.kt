package com.yaca.android.chat.composer

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material3.AssistChip
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
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
import com.yaca.android.chat.ComposerActions
import com.yaca.core.model.Effort
import com.yaca.core.model.Mode
import com.yaca.core.model.ModelInfo
import com.yaca.core.model.SessionState
import com.yaca.core.model.effortLevels

/**
 * The composer's settings, as chips in one row that scrolls sideways: the mode, the model, and the
 * effort, which the model must support to be offered at all.
 */
@Composable
fun ComposerChips(
    state: SessionState?,
    models: List<ModelInfo>,
    actions: ComposerActions,
    modifier: Modifier = Modifier,
) {
    if (state == null) return
    val current = models.firstOrNull { it.id == state.model }
    Row(
        modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ModeChip(mode = state.mode, onPick = actions.onMode)
        ModelChip(
            label = current?.displayName ?: state.model,
            selectedId = state.model,
            models = models,
            onPick = actions.onModel,
        )
        val levels = current?.effortLevels().orEmpty()
        if (levels.isNotEmpty()) {
            EffortChip(effort = state.effort.takeIf { it in levels } ?: Effort.AUTO,
                levels = listOf(Effort.AUTO) + levels, onPick = actions.onEffort)
        }
    }
}

@Composable
private fun ModeChip(mode: Mode, onPick: (Mode) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    Box {
        AssistChip(
            onClick = { open = true },
            label = { Text(modeLabel(mode)) },
            trailingIcon = { ChevronIcon() },
        )
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            Mode.entries.forEach { choice ->
                DropdownMenuItem(
                    text = { Text(modeLabel(choice)) },
                    onClick = {
                        open = false
                        onPick(choice)
                    },
                    trailingIcon = {
                        if (choice == mode) Icon(Icons.Filled.Check, contentDescription = null)
                    },
                )
            }
        }
    }
}

@Composable
private fun EffortChip(effort: Effort, levels: List<Effort>, onPick: (Effort) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    Box {
        AssistChip(
            onClick = { open = true },
            label = { Text(effortLabel(effort)) },
            trailingIcon = { ChevronIcon() },
        )
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            levels.forEach { choice ->
                DropdownMenuItem(
                    text = { Text(effortLabel(choice)) },
                    onClick = {
                        open = false
                        onPick(choice)
                    },
                    trailingIcon = {
                        if (choice == effort) Icon(Icons.Filled.Check, contentDescription = null)
                    },
                )
            }
        }
    }
}

/** The model chip opens the model list in a sheet: a long list does not fit a menu on a phone. */
@Composable
private fun ModelChip(label: String, selectedId: String, models: List<ModelInfo>, onPick: (String) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    AssistChip(
        onClick = { open = true },
        label = { Text(label) },
        trailingIcon = { ChevronIcon() },
    )
    if (open) {
        ModelSheet(
            models = models,
            selectedId = selectedId,
            onPick = {
                open = false
                onPick(it)
            },
            onDismiss = { open = false },
        )
    }
}

@Composable
private fun ChevronIcon() {
    Icon(
        imageVector = Icons.Filled.KeyboardArrowDown,
        contentDescription = null,
        tint = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.size(18.dp),
    )
}

@Composable
fun modeLabel(mode: Mode): String = stringResource(
    when (mode) {
        Mode.PLAN -> R.string.mode_plan
        Mode.MANUAL -> R.string.mode_manual
        Mode.AUTO -> R.string.mode_auto
        Mode.EDITS -> R.string.mode_edits
    },
)

@Composable
fun effortLabel(effort: Effort): String = stringResource(
    when (effort) {
        Effort.AUTO -> R.string.effort_auto
        Effort.NONE -> R.string.effort_none
        Effort.MINIMAL -> R.string.effort_minimal
        Effort.LOW -> R.string.effort_low
        Effort.MEDIUM -> R.string.effort_medium
        Effort.HIGH -> R.string.effort_high
        Effort.EXTRA -> R.string.effort_extra
        Effort.MAX -> R.string.effort_max
    },
)
