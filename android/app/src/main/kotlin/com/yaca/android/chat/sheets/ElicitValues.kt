package com.yaca.android.chat.sheets

import com.yaca.core.model.ElicitField
import com.yaca.core.model.ElicitKind
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull

// The form's values as text while it is typed, and as the JSON the server asked for when it is sent.
// Kept apart from the composables so the rules (what is required, what is a number) are checked by
// plain tests.

/** What is wrong with a field, when the form is sent. */
enum class ElicitProblem { REQUIRED, NOT_A_NUMBER, NOT_AN_INTEGER }

/** The starting text of each field: the server's default, a boolean's "false" when it has none. */
fun elicitInitial(fields: List<ElicitField>): Map<String, String> = fields.associate { field ->
    val default = field.default
    val text = when {
        field.kind == ElicitKind.BOOLEAN -> if ((default as? JsonPrimitive)?.booleanOrNull == true) "true" else "false"
        default is JsonPrimitive && default !is JsonNull -> default.content
        else -> ""
    }
    field.name to text
}

/** The problems of the typed values, by field name. An empty map means the form can be sent. */
fun elicitProblems(fields: List<ElicitField>, typed: Map<String, String>): Map<String, ElicitProblem> {
    val out = LinkedHashMap<String, ElicitProblem>()
    for (field in fields) {
        val text = typed[field.name].orEmpty().trim()
        if (text.isEmpty()) {
            // A boolean always has a value ("false" when untouched), so only the others can be missing.
            if (field.required && field.kind != ElicitKind.BOOLEAN) out[field.name] = ElicitProblem.REQUIRED
            continue
        }
        when (field.kind) {
            ElicitKind.NUMBER -> if (text.toDoubleOrNull() == null) out[field.name] = ElicitProblem.NOT_A_NUMBER
            ElicitKind.INTEGER -> if (text.toLongOrNull() == null) out[field.name] = ElicitProblem.NOT_AN_INTEGER
            ElicitKind.TEXT, ElicitKind.BOOLEAN, ElicitKind.CHOICE -> Unit
        }
    }
    return out
}

/**
 * The values of a form, keyed by field name, in the JSON types the field takes. An empty optional
 * field is left out; a boolean is always sent. Call only when [elicitProblems] is empty.
 */
fun elicitValues(fields: List<ElicitField>, typed: Map<String, String>): Map<String, JsonElement> {
    val out = LinkedHashMap<String, JsonElement>()
    for (field in fields) {
        val text = typed[field.name].orEmpty().trim()
        when (field.kind) {
            ElicitKind.BOOLEAN -> out[field.name] = JsonPrimitive(text == "true")
            ElicitKind.NUMBER -> text.toDoubleOrNull()?.let { out[field.name] = JsonPrimitive(it) }
            ElicitKind.INTEGER -> text.toLongOrNull()?.let { out[field.name] = JsonPrimitive(it) }
            ElicitKind.TEXT, ElicitKind.CHOICE -> if (text.isNotEmpty()) out[field.name] = JsonPrimitive(text)
        }
    }
    return out
}
