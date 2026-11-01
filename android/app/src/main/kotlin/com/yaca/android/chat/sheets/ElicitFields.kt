package com.yaca.android.chat.sheets

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.core.model.ElicitField
import com.yaca.core.model.ElicitKind

/**
 * One field of an MCP form, drawn by its kind: a text line, a number line, a switch, or a list of
 * choices. The supporting line says what is wrong, or else the field's description, or that it is
 * optional.
 */
@Composable
fun ElicitFieldView(
    field: ElicitField,
    value: String,
    problem: ElicitProblem?,
    onChange: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val problemText = problemText(problem)
    val support = problemText ?: field.description ?: if (field.required) null else stringResource(R.string.elicit_optional)
    when (field.kind) {
        ElicitKind.TEXT -> OutlinedTextField(
            value = value,
            onValueChange = onChange,
            modifier = modifier.fillMaxWidth(),
            label = { Text(field.title) },
            supportingText = { if (support != null) Text(support) },
            isError = problem != null,
            singleLine = true,
        )
        ElicitKind.NUMBER -> OutlinedTextField(
            value = value,
            onValueChange = onChange,
            modifier = modifier.fillMaxWidth(),
            label = { Text(field.title) },
            supportingText = { if (support != null) Text(support) },
            isError = problem != null,
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
        )
        ElicitKind.INTEGER -> OutlinedTextField(
            value = value,
            onValueChange = onChange,
            modifier = modifier.fillMaxWidth(),
            label = { Text(field.title) },
            supportingText = { if (support != null) Text(support) },
            isError = problem != null,
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        )
        ElicitKind.BOOLEAN -> Row(
            modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // The core's model lives in another module: its properties are read into a local first, not smart-cast.
            val description = field.description
            Column(Modifier.weight(1f)) {
                Text(text = field.title, style = MaterialTheme.typography.bodyMedium)
                if (description != null) {
                    Text(
                        text = description,
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            Switch(checked = value == "true", onCheckedChange = { onChange(if (it) "true" else "false") })
        }
        ElicitKind.CHOICE -> Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(text = field.title, style = MaterialTheme.typography.bodyMedium)
            field.options.forEachIndexed { index, option ->
                ChoiceRow(
                    selected = value == option,
                    title = field.optionLabels.getOrElse(index) { option },
                    hint = null,
                    onSelect = { onChange(option) },
                )
            }
            if (support != null) {
                Text(
                    text = support,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun problemText(problem: ElicitProblem?): String? = when (problem) {
    null -> null
    ElicitProblem.REQUIRED -> stringResource(R.string.elicit_required_error)
    ElicitProblem.NOT_A_NUMBER -> stringResource(R.string.elicit_bad_number)
    ElicitProblem.NOT_AN_INTEGER -> stringResource(R.string.elicit_bad_integer)
}
