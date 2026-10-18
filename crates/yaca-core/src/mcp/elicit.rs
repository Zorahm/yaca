//! Elicitation: an MCP server asking the human to fill in a few fields.
//!
//! A server in the middle of its work may need something only the person
//! knows — which account, a confirmation, a name — and asks with
//! `elicitation/create`: a message and a flat schema of primitive fields. The
//! feed shows it as a form under the server's name; what the person submits,
//! declines or cancels goes back to the server as its answer.
//!
//! The schema is the protocol's restricted one: an object of strings, numbers,
//! integers, booleans and enums, nothing nested. A property of any other shape
//! is offered as text rather than refused, so a server that bends the rules
//! still gets an answer a person could give.
//!
//! The answer is checked here before it is sent: a required field left empty,
//! a number that is not one, an option that was not offered. The form stays
//! open after a refused answer, as a question does.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use ts_rs::TS;

use crate::id::RequestId;

/// What kind of input a field takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ElicitKind {
    Text,
    Number,
    Integer,
    Boolean,
    /// One of `options`.
    Choice,
}

/// One field of the form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ElicitField {
    /// The key the server reads the value under.
    pub name: String,
    /// What the form shows as the field's label: the schema's `title`, or the
    /// name.
    pub title: String,
    pub description: Option<String>,
    pub kind: ElicitKind,
    pub required: bool,
    /// The values a `Choice` takes, as sent back.
    pub options: Vec<String>,
    /// How each option is shown (`enumNames`), the same length as `options`.
    pub option_labels: Vec<String>,
    /// What the field starts with.
    #[ts(type = "unknown")]
    pub default: Option<Value>,
}

/// A form on screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationRequest {
    pub id: RequestId,
    /// The server that asks, as the session names it.
    pub server: String,
    /// What the server says it needs, in its own words.
    pub message: String,
    pub fields: Vec<ElicitField>,
}

/// What the human did with a form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ElicitationReply {
    /// Submitted, with a value per field that was filled.
    Accept {
        #[ts(type = "Record<string, unknown>")]
        values: BTreeMap<String, Value>,
    },
    /// Said no: the server is told the person declined.
    Decline,
    /// Closed without deciding.
    Cancel,
}

/// Reads `elicitation/create`'s params into a form. `Err` with the reason when
/// there is nothing a person could fill in.
pub(crate) fn parse(server: &str, params: &Value) -> Result<ElicitationRequest, String> {
    let message = params["message"].as_str().unwrap_or_default().trim().to_string();
    let schema = &params["requestedSchema"];
    let required: Vec<&str> = schema["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut fields = Vec::new();
    if let Some(properties) = schema["properties"].as_object() {
        for (name, property) in properties {
            fields.push(field(name, property, required.contains(&name.as_str())));
        }
    }
    if message.is_empty() && fields.is_empty() {
        return Err("the server asked for input with neither a message nor fields".into());
    }
    Ok(ElicitationRequest {
        id: RequestId::new(),
        server: server.to_string(),
        message,
        fields,
    })
}

fn field(name: &str, property: &Value, required: bool) -> ElicitField {
    let options: Vec<String> = property["enum"]
        .as_array()
        .map(|values| values.iter().map(value_text).collect())
        .unwrap_or_default();
    let labels: Vec<String> = property["enumNames"]
        .as_array()
        .map(|names| names.iter().map(value_text).collect())
        .filter(|names: &Vec<String>| names.len() == options.len())
        .unwrap_or_else(|| options.clone());
    let kind = if !options.is_empty() {
        ElicitKind::Choice
    } else {
        match property["type"].as_str() {
            Some("number") => ElicitKind::Number,
            Some("integer") => ElicitKind::Integer,
            Some("boolean") => ElicitKind::Boolean,
            _ => ElicitKind::Text,
        }
    };
    ElicitField {
        name: name.to_string(),
        title: property["title"]
            .as_str()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or(name)
            .to_string(),
        description: property["description"].as_str().map(str::to_string),
        kind,
        required,
        options,
        option_labels: labels,
        default: property.get("default").filter(|d| !d.is_null()).cloned(),
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl ElicitationRequest {
    /// Why this reply cannot be sent, or `None` when it can.
    pub fn refuses(&self, reply: &ElicitationReply) -> Option<String> {
        let ElicitationReply::Accept { values } = reply else {
            return None;
        };
        for field in &self.fields {
            let value = values.get(&field.name).filter(|v| !is_empty(v));
            let Some(value) = value else {
                if field.required {
                    return Some(format!("\"{}\" is required", field.title));
                }
                continue;
            };
            let fits = match field.kind {
                ElicitKind::Text => value.is_string(),
                ElicitKind::Number => value.is_number(),
                ElicitKind::Integer => value.is_i64() || value.is_u64(),
                ElicitKind::Boolean => value.is_boolean(),
                ElicitKind::Choice => field.options.contains(&value_text(value)),
            };
            if !fits {
                return Some(format!("\"{}\" does not take {value}", field.title));
            }
        }
        if let Some(stray) = values.keys().find(|k| !self.fields.iter().any(|f| &f.name == *k)) {
            return Some(format!("the form has no field \"{stray}\""));
        }
        None
    }

    /// The reply as the server reads it. An accepted choice goes back as the
    /// value the schema listed, with its own type.
    pub(crate) fn answer(&self, reply: &ElicitationReply, schema: &Value) -> Value {
        match reply {
            ElicitationReply::Accept { values } => {
                let mut content = Map::new();
                for (name, value) in values {
                    if is_empty(value) {
                        continue;
                    }
                    let original = schema["properties"][name]["enum"]
                        .as_array()
                        .and_then(|all| all.iter().find(|v| value_text(v) == value_text(value)))
                        .cloned();
                    content.insert(name.clone(), original.unwrap_or_else(|| value.clone()));
                }
                json!({ "action": "accept", "content": content })
            }
            ElicitationReply::Decline => json!({ "action": "decline" }),
            ElicitationReply::Cancel => json!({ "action": "cancel" }),
        }
    }
}

fn is_empty(value: &Value) -> bool {
    value.is_null() || value.as_str().is_some_and(|s| s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Value {
        json!({
            "message": "Which account should the invoice go to?",
            "requestedSchema": {
                "type": "object",
                "properties": {
                    "account": { "type": "string", "title": "Account", "enum": ["a1", "b2"], "enumNames": ["Main", "Backup"] },
                    "copies": { "type": "integer", "minimum": 1, "default": 1 },
                    "notify": { "type": "boolean", "description": "Email me" },
                    "note": { "type": "string" },
                    "weird": { "type": "object" }
                },
                "required": ["account", "copies"]
            }
        })
    }

    #[test]
    fn a_schema_becomes_fields_a_person_can_fill() {
        let form = parse("billing", &params()).unwrap();
        assert_eq!(form.server, "billing");
        let by = |name: &str| form.fields.iter().find(|f| f.name == name).unwrap().clone();
        let account = by("account");
        assert_eq!(account.kind, ElicitKind::Choice);
        assert_eq!(account.title, "Account");
        assert_eq!(account.option_labels, vec!["Main", "Backup"]);
        assert!(account.required);
        assert_eq!(by("copies").kind, ElicitKind::Integer);
        assert_eq!(by("copies").default, Some(json!(1)));
        assert_eq!(by("notify").kind, ElicitKind::Boolean);
        assert!(!by("notify").required);
        assert_eq!(by("weird").kind, ElicitKind::Text, "a shape the protocol does not allow is text");
    }

    #[test]
    fn an_answer_is_checked_before_it_goes() {
        let form = parse("billing", &params()).unwrap();
        let accept = |values: Value| ElicitationReply::Accept {
            values: serde_json::from_value(values).unwrap(),
        };
        assert!(form.refuses(&accept(json!({ "account": "a1", "copies": 2 }))).is_none());
        assert!(form.refuses(&accept(json!({ "account": "a1" }))).unwrap().contains("copies"));
        assert!(form.refuses(&accept(json!({ "account": "zz", "copies": 2 }))).is_some());
        assert!(form.refuses(&accept(json!({ "account": "a1", "copies": 2.5 }))).is_some());
        assert!(form.refuses(&accept(json!({ "account": "a1", "copies": 2, "x": 1 }))).is_some());
        assert!(form.refuses(&ElicitationReply::Decline).is_none());
    }

    #[test]
    fn the_server_reads_accept_decline_and_cancel() {
        let p = params();
        let form = parse("billing", &p).unwrap();
        let reply = ElicitationReply::Accept {
            values: serde_json::from_value(json!({ "account": "b2", "copies": 3, "note": "  " })).unwrap(),
        };
        assert_eq!(
            form.answer(&reply, &p["requestedSchema"]),
            json!({ "action": "accept", "content": { "account": "b2", "copies": 3 } })
        );
        assert_eq!(form.answer(&ElicitationReply::Decline, &p), json!({ "action": "decline" }));
        assert_eq!(form.answer(&ElicitationReply::Cancel, &p), json!({ "action": "cancel" }));
    }

    #[test]
    fn nothing_to_ask_is_not_a_form() {
        assert!(parse("s", &json!({ "message": "  " })).is_err());
    }
}
