//! Display-only arguments from an unfinished JSON object. The real arguments
//! remain in the accumulator and are parsed and validated before execution.

use serde_json::{Map, Value};

use super::calls::Caller;
use crate::event::AgentEvent;

/// Scan each byte once, retaining only top-level fields used by tool captions.
/// In particular, large Write bodies and nested Edit replacements stay out of
/// both the event stream and this buffer.
#[derive(Debug, Default)]
pub(super) struct PreviewInput {
    depth: usize,
    in_string: bool,
    escaped: bool,
    key: Option<String>,
    reading_key: bool,
    capture: bool,
    raw: String,
    fields: Map<String, Value>,
}

impl PreviewInput {
    pub(super) fn push(&mut self, chunk: &str) -> Option<Value> {
        let mut changed = false;
        for ch in chunk.chars() {
            if self.in_string {
                if self.escaped {
                    if self.capture {
                        self.raw.push(ch);
                    }
                    self.escaped = false;
                } else if ch == '\\' {
                    if self.capture {
                        self.raw.push(ch);
                    }
                    self.escaped = true;
                } else if ch == '"' {
                    if self.reading_key {
                        self.key = serde_json::from_str(&format!("\"{}\"", self.raw)).ok();
                    } else if self.capture {
                        changed |= self.publish();
                    }
                    self.in_string = false;
                    self.capture = false;
                    self.raw.clear();
                } else if self.capture {
                    self.raw.push(ch);
                }
                continue;
            }
            match ch {
                '{' | '[' => {
                    self.depth += 1;
                    if self.depth == 1 {
                        self.reading_key = true;
                    }
                }
                '}' | ']' => self.depth = self.depth.saturating_sub(1),
                ',' if self.depth == 1 => {
                    self.reading_key = true;
                    self.key = None;
                }
                ':' if self.depth == 1 => self.reading_key = false,
                '"' => {
                    self.in_string = true;
                    self.capture = self.depth == 1
                        && (self.reading_key || self.key.as_deref().is_some_and(display_field));
                }
                _ => {}
            }
        }
        if self.in_string && self.capture && !self.reading_key {
            changed |= self.publish();
        }
        changed.then(|| Value::Object(self.fields.clone()))
    }

    fn publish(&mut self) -> bool {
        let Some(key) = self.key.as_ref() else {
            return false;
        };
        // A chunk may end inside an escape or a UTF-16 surrogate pair. Keep
        // the last valid prefix until the rest arrives, rather than flashing
        // invalid JSON escapes in the row.
        let Ok(value) = serde_json::from_str::<String>(&format!("\"{}\"", self.raw)) else {
            return false;
        };
        let value = Value::String(value);
        if self.fields.get(key) == Some(&value) {
            return false;
        }
        self.fields.insert(key.clone(), value);
        true
    }
}

fn display_field(key: &str) -> bool {
    matches!(
        key,
        "description"
            | "path"
            | "from"
            | "to"
            | "command"
            | "cmd"
            | "pattern"
            | "query"
            | "url"
            | "name"
            | "file"
            | "question"
    )
}

impl Caller {
    /// Add the registry's target and parent without running any tool logic.
    pub(super) fn emit_stream(&self, mut event: AgentEvent) {
        if let AgentEvent::ToolCallPreparing {
            tool,
            input,
            description,
            target,
            parent,
            ..
        } = &mut event
        {
            let registered = self.registry.get(tool, self.intent(), self.profile());
            if registered.as_ref().is_some_and(|tool| tool.silent()) {
                return;
            }
            *description = if registered
                .as_ref()
                .is_some_and(|tool| tool.keeps_description())
            {
                None
            } else {
                input
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            };
            *target = registered.and_then(|tool| tool.target(input));
            *parent = self.under.clone();
        }
        self.emit(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_stream_across_escapes_and_unicode_boundaries() {
        let mut input = PreviewInput::default();
        assert_eq!(
            input.push(r#"{"command":"echo "#).unwrap()["command"],
            "echo "
        );
        assert!(input.push(r#"\uD83D"#).is_none());
        assert_eq!(
            input.push(r#"\uDE00\n\"ok\""#).unwrap()["command"],
            "echo 😀\n\"ok\""
        );
        assert!(input.push(r#""}"#).is_none());
    }

    #[test]
    fn file_bodies_and_nested_edits_never_enter_the_preview() {
        let mut input = PreviewInput::default();
        let shown = input.push(r#"{"content":"large \"body\"","edits":[{"path":"wrong","new_string":"new"}],"path":"src/main.rs"}"#).unwrap();
        assert_eq!(shown, serde_json::json!({"path": "src/main.rs"}));
        assert!(input.raw.is_empty());
    }

    #[test]
    fn body_only_chunks_do_not_emit_repeated_snapshots() {
        let mut input = PreviewInput::default();
        assert!(input.push(r#"{"path":"file.rs","content":""#).is_some());
        assert!(input.push(&"x".repeat(100_000)).is_none());
        assert!(input.raw.is_empty());
        assert_eq!(
            input.push(r#"","description":"Writing file"}"#).unwrap()["description"],
            "Writing file"
        );
    }
}
