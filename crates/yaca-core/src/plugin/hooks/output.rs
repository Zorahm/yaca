//! What a plugin hook's answer means, by Claude Code's rules.
//!
//! * Exit 0 — stdout is read. JSON is the structured answer; plain text is
//!   context for the model on `SessionStart` and `UserPromptSubmit`, and
//!   nothing on the other events.
//! * Exit 2 — a block. stderr is the reason.
//! * Anything else, a timeout, a hook that would not start — a note for the
//!   human, and the work goes on.
//!
//! The human-facing words are English; the reason a
//! hook gives is passed through as it wrote it.

use serde_json::Value;

use super::run::Ran;
use super::HookEvent;

/// One hook's answer, reduced to what Yaca acts on.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Verdict {
    /// The hook stops what was about to happen, for this reason. On
    /// `PostToolUse` the call already ran, and the reason is feedback for the
    /// model.
    pub block: Option<String>,
    /// `PreToolUse` asked for the human's question even if the mode skips it.
    pub ask: bool,
    /// Text the model is to read alongside.
    pub context: Vec<String>,
    /// Lines for the human: what the hook said to them, or that it failed.
    pub notices: Vec<String>,
    /// `PreToolUse` rewrote the call's arguments: the call runs with these,
    /// and the permission question is asked about them.
    pub updated_input: Option<Value>,
}

pub fn verdict(event: HookEvent, ran: &Ran) -> Verdict {
    let mut v = Verdict::default();
    match ran {
        Ran::Exited { code: Some(0), stdout, .. } => match serde_json::from_str::<Value>(stdout.trim()) {
            Ok(Value::Object(fields)) => structured(&fields, &mut v),
            _ if !stdout.trim().is_empty() && takes_plain_context(event) => {
                v.context.push(stdout.trim().to_string());
            }
            _ => {}
        },
        Ran::Exited { code: Some(2), stdout, stderr } => {
            let reason = if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() };
            v.block = Some(if reason.is_empty() {
                "the hook refused without an explanation".to_string()
            } else {
                reason.to_string()
            });
        }
        Ran::Exited { code, stderr, .. } => {
            let how = code.map_or("by a signal".to_string(), |c| format!("with code {c}"));
            let tail = if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", clip(stderr.trim()))
            };
            v.notices.push(format!("exited {how}{tail}"));
        }
        Ran::TimedOut(secs) => v.notices.push(format!("did not answer within {secs} s")),
        Ran::Broken(why) => v.notices.push(format!("did not start: {why}")),
        Ran::Stopped | Ran::Detached => {}
    }
    v
}

/// A field's text, when it has any.
fn text(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

fn takes_plain_context(event: HookEvent) -> bool {
    matches!(event, HookEvent::SessionStart | HookEvent::UserPromptSubmit)
}

fn structured(fields: &serde_json::Map<String, Value>, v: &mut Verdict) {
    let specific = fields.get("hookSpecificOutput").and_then(Value::as_object);

    if fields.get("continue").and_then(Value::as_bool) == Some(false) {
        v.block = Some(text(fields.get("stopReason")).unwrap_or("the hook stopped the work").to_string());
    }
    // `approve` is the older spelling Claude Code still reads; it grants, and
    // so is ignored like `allow`.
    if fields.get("decision").and_then(Value::as_str) == Some("block") {
        v.block = Some(text(fields.get("reason")).unwrap_or("the hook refused without an explanation").to_string());
    }
    if let Some(specific) = specific {
        match specific.get("permissionDecision").and_then(Value::as_str) {
            Some("deny") => {
                let reason = text(specific.get("permissionDecisionReason")).unwrap_or("the hook refused without an explanation");
                v.block = Some(reason.to_string());
            }
            Some("ask") => v.ask = true,
            _ => {}
        }
        if let Some(context) = text(specific.get("additionalContext")) {
            v.context.push(context.to_string());
        }
        if let Some(input) = specific.get("updatedInput").filter(|i| i.is_object()) {
            v.updated_input = Some(input.clone());
        }
    }
    if let Some(message) = text(fields.get("systemMessage")) {
        v.notices.push(message.to_string());
    }
}

fn clip(text: &str) -> String {
    const MAX: usize = 600;
    if text.len() <= MAX {
        return text.to_string();
    }
    let mut cut = MAX;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…", &text[..cut])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exited(code: i32, stdout: &str, stderr: &str) -> Ran {
        Ran::Exited {
            code: Some(code),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    #[test]
    fn plain_stdout_is_context_only_where_claude_reads_it_so() {
        let start = verdict(HookEvent::SessionStart, &exited(0, "rules", ""));
        assert_eq!(start.context, ["rules"]);
        let pre = verdict(HookEvent::PreToolUse, &exited(0, "chatter", ""));
        assert_eq!(pre, Verdict::default());
    }

    #[test]
    fn additional_context_comes_out_of_the_json() {
        let json = r#"{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"be careful"}}"#;
        assert_eq!(verdict(HookEvent::SessionStart, &exited(0, json, "")).context, ["be careful"]);
    }

    #[test]
    fn exit_two_blocks_with_stderr_as_the_reason() {
        let v = verdict(HookEvent::PreToolUse, &exited(2, "", "no PRs here"));
        assert_eq!(v.block.as_deref(), Some("no PRs here"));
    }

    #[test]
    fn deny_blocks_ask_asks_and_allow_grants_nothing() {
        let deny = r#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"nope"}}"#;
        assert_eq!(verdict(HookEvent::PreToolUse, &exited(0, deny, "")).block.as_deref(), Some("nope"));
        let ask = r#"{"hookSpecificOutput":{"permissionDecision":"ask"}}"#;
        assert!(verdict(HookEvent::PreToolUse, &exited(0, ask, "")).ask);
        let allow = r#"{"hookSpecificOutput":{"permissionDecision":"allow"}}"#;
        assert_eq!(verdict(HookEvent::PreToolUse, &exited(0, allow, "")), Verdict::default());
    }

    #[test]
    fn decision_block_and_continue_false_block() {
        let block = r#"{"decision":"block","reason":"keep going"}"#;
        assert_eq!(verdict(HookEvent::Stop, &exited(0, block, "")).block.as_deref(), Some("keep going"));
        let stop = r#"{"continue":false,"stopReason":"halt"}"#;
        assert_eq!(verdict(HookEvent::UserPromptSubmit, &exited(0, stop, "")).block.as_deref(), Some("halt"));
    }

    #[test]
    fn failures_are_notes_and_never_blocks() {
        for ran in [exited(1, "", "boom"), Ran::TimedOut(5), Ran::Broken("no bash".into())] {
            let v = verdict(HookEvent::PreToolUse, &ran);
            assert_eq!(v.block, None);
            assert_eq!(v.notices.len(), 1);
        }
        assert!(verdict(HookEvent::PreToolUse, &exited(1, "", "boom")).notices[0].contains("boom"));
    }

    #[test]
    fn a_quiet_success_says_nothing() {
        let quiet = r#"{"continue":true,"suppressOutput":true}"#;
        assert_eq!(verdict(HookEvent::SessionStart, &exited(0, quiet, "")), Verdict::default());
    }

    #[test]
    fn updated_input_is_taken_only_as_an_object() {
        let json = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"ls -la"}}}"#;
        let v = verdict(HookEvent::PreToolUse, &exited(0, json, ""));
        assert_eq!(v.updated_input, Some(serde_json::json!({ "command": "ls -la" })));
        let bad = r#"{"hookSpecificOutput":{"updatedInput":"rm -rf /"}}"#;
        assert_eq!(verdict(HookEvent::PreToolUse, &exited(0, bad, "")).updated_input, None);
    }
}
