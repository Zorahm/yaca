//! Hooks: commands the human wrote, run around each tool call.
//!
//! A project knows things a model does not: a file that must not be touched, a
//! formatter that should see every write, a log of what ran. `.yaca/settings.json`
//! lists those as commands under `hooks`, and this module runs the ones that
//! match a call. The shape of the file is in [`crate::settings::HooksSettings`].
//!
//! Trust. A hook is not a tool call and does not go through the permission
//! broker, for the reason `verify` does not: the human wrote it, in the file they
//! own, and a question before every hook would make each call a prompt. What
//! hooks may do is narrower, and the two lists say so. A pre hook can only refuse
//! a call. Nothing a hook prints can grant one, and it runs before the permission
//! question, so a refused call never reaches the human at all. A post hook adds
//! words to the result; it changes nothing that already happened.
//!
//! Fail closed before a call. A pre hook that does not start, or does not finish
//! within its timeout, refuses the call. The alternative — letting the call run
//! because the check could not run — makes a guard that breaks silently into no
//! guard. A project that wants a broken hook to be ignored can remove it; it
//! cannot be ignored by accident.
//!
//! Fail open after. The tool already ran, so a post hook that fails can only
//! report that it failed. It is a note on the call, not a failure of the call:
//! the model is not told the edit went wrong when the edit did not.
//!
//! The process runs through the shell Bash uses, with stdin carrying a JSON
//! description of the call and two environment variables naming the tool and the
//! project root. Stdin is closed after the description, so a hook that reads its
//! input to the end gets one. Its process tree is killed on timeout and on stop,
//! the same as a background command, so a hook that starts a server cannot hold
//! the turn.
//!
//! A pre hook may also rewrite the call: a JSON object with `updatedInput` on
//! its standard output, exit code 0, replaces the call's arguments. The
//! rewritten call is the one the permission question is then asked about, so
//! a rewrite can narrow a call or fix it but never slip one past the human.
//!
//! The events that are not about a call — the session's start and end, a
//! message from the human, the end of a turn, a compaction, a question waiting
//! for the human — run through [`on_event`], which only reports how each hook
//! ended. What an ending means is the session's to decide, per event.
//!
//! This module knows nothing about turns or modes. The wiring is in
//! `session::calls` and `session::plugin_hooks`, and the tests here run real
//! processes in a real project.

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::process::kill;
use crate::process::shell::{self, Shell};
use crate::settings::{EventHookSettings, HookSettings, HooksSettings};
use crate::workspace::Workspace;

/// What the runner keeps of a hook's output. Past this the output is read and
/// dropped, so a chatty hook cannot stall on a full pipe, and memory is bounded.
const CAPTURE_BYTES: usize = 64 * 1024;
/// What of a hook's output reaches the model. Long enough for a lint report's
/// first screen; the rest is in the command's own log, not in the window.
const MODEL_BYTES: usize = 4 * 1024;
/// What of a refusal the feed shows. The human can read the rest in the
/// command, and a feed row is not the place for a wall of text.
const FEED_BYTES: usize = 600;
/// The result a post hook is handed on stdin. Enough to tell what happened;
/// the full text is what the model is about to read anyway.
const RESULT_BYTES: usize = 8 * 1024;
/// How long a hook gets to be reaped after it has been killed. A process that
/// survives its kill would otherwise hold the turn.
const REAP_SECS: u64 = 5;

/// What a pre hook decided about a call.
#[derive(Debug, PartialEq, Eq)]
pub enum PreOutcome {
    /// Every matching hook exited 0. `Some` when one of them rewrote the
    /// call's arguments: the call runs with these.
    Go(Option<Value>),
    /// A hook refused the call. Nothing it would have done was done.
    Refused(Refusal),
    /// The human stopped the turn while a hook ran.
    Stopped,
}

/// A refusal in both languages the call reads in: the feed's line for the human
/// and the sentence the model reads back.
#[derive(Debug, PartialEq, Eq)]
pub struct Refusal {
    /// Russian, like the rest of the feed. Shown as the reason of the row.
    pub for_human: String,
    /// English, like the rest of what the model reads.
    pub for_model: String,
}

/// What the post hooks of a call said. `for_model` is appended to the result
/// the model reads; `for_human` is a line of the call's row.
#[derive(Debug, PartialEq, Eq)]
pub struct Note {
    pub for_human: String,
    pub for_model: String,
}

/// Whether any post hook applies to this tool. Lets the caller skip copying a
/// call's input when nobody will read it.
pub fn wants_post(hooks: &HooksSettings, tool: &str) -> bool {
    hooks.post_tool_use.iter().any(|hook| hook.applies_to(tool))
}

/// Runs the pre hooks that match `tool`, in the order they are written. The
/// first refusal ends the list: a later hook would be told about a call that
/// will not happen.
pub async fn before(
    hooks: &HooksSettings,
    ws: &Workspace,
    shell: Shell,
    tool: &str,
    input: &Value,
    cancel: &CancellationToken,
) -> PreOutcome {
    let mut updated: Option<Value> = None;
    for hook in hooks.pre_tool_use.iter().filter(|h| h.applies_to(tool)) {
        // Each hook is told the call as the hooks before it left it.
        let current = updated.as_ref().unwrap_or(input);
        let stdin = json!({ "event": "preToolUse", "tool": tool, "input": current }).to_string();
        let env = [("YACA_TOOL", tool)];
        match run(hook.command(), hook.timeout(), ws, shell, &env, stdin.as_bytes(), cancel).await {
            Ending::Exited { success: true, output, .. } => {
                if let Some(input) = updated_input(&output) {
                    updated = Some(input);
                }
            }
            Ending::Exited { code, output, .. } => {
                return PreOutcome::Refused(refusal_by_exit(hook, code, &output));
            }
            Ending::TimedOut { secs, .. } => {
                return PreOutcome::Refused(refusal_by_timeout(hook, secs));
            }
            Ending::Broken(why) => return PreOutcome::Refused(refusal_by_start(hook, &why)),
            Ending::Stopped => return PreOutcome::Stopped,
        }
    }
    PreOutcome::Go(updated)
}

/// `{"updatedInput": {...}}` on a successful pre hook's output. Anything else
/// it prints is its own business, as before.
fn updated_input(output: &str) -> Option<Value> {
    let parsed: Value = serde_json::from_str(output.trim()).ok()?;
    let input = parsed.get("updatedInput")?;
    input.is_object().then(|| input.clone())
}

/// How one hook of an event ended.
#[derive(Debug, PartialEq, Eq)]
pub enum EventEnding {
    /// Exit code 0, with what it printed.
    Passed(String),
    /// Any other exit code, with what it printed.
    Failed { code: Option<i32>, output: String },
    TimedOut(u64),
    /// Could not be started.
    Broken(String),
}

/// One hook of an event and how it ended.
#[derive(Debug)]
pub struct EventSaid {
    pub command: String,
    pub ending: EventEnding,
}

impl EventSaid {
    /// What the hook printed, cut for the model; empty when it printed nothing.
    pub fn output_for_model(&self) -> String {
        match &self.ending {
            EventEnding::Passed(output) | EventEnding::Failed { output, .. } => {
                if output.trim().is_empty() {
                    String::new()
                } else {
                    clip(output, MODEL_BYTES)
                }
            }
            _ => String::new(),
        }
    }

    /// One line for the feed about a hook that did not pass.
    pub fn failure_for_human(&self) -> Option<String> {
        let command = &self.command;
        match &self.ending {
            EventEnding::Passed(_) => None,
            EventEnding::Failed { code, output } => Some(format!(
                "`{command}` {}\n{}",
                english_exited(*code),
                clip(body(output), FEED_BYTES)
            )),
            EventEnding::TimedOut(secs) => Some(format!("`{command}` did not respond within {secs} s")),
            EventEnding::Broken(why) => Some(format!("`{command}` failed to start: {why}")),
        }
    }
}

/// Runs the hooks of an event in the order they are written, each told
/// `payload` with the event's name added. `None` when the turn was stopped
/// while one ran: nobody is waiting for the answer any more.
pub async fn on_event(
    list: &[EventHookSettings],
    ws: &Workspace,
    shell: Shell,
    event: &str,
    payload: Value,
    cancel: &CancellationToken,
) -> Option<Vec<EventSaid>> {
    let mut stdin = json!({ "event": event });
    if let (Value::Object(base), Value::Object(more)) = (&mut stdin, payload) {
        base.extend(more);
    }
    let stdin = stdin.to_string();
    let env = [("YACA_HOOK_EVENT", event)];
    let mut out = Vec::new();
    for hook in list {
        let ending = match run(hook.command(), hook.timeout(), ws, shell, &env, stdin.as_bytes(), cancel).await {
            Ending::Exited { success: true, output, .. } => EventEnding::Passed(output),
            Ending::Exited { code, output, .. } => EventEnding::Failed { code, output },
            Ending::TimedOut { secs } => EventEnding::TimedOut(secs),
            Ending::Broken(why) => EventEnding::Broken(why),
            Ending::Stopped => return None,
        };
        out.push(EventSaid {
            command: hook.command().to_string(),
            ending,
        });
    }
    Some(out)
}

/// How a call ended, as a post hook is told it: whether its result is an error,
/// and the text the model is about to read.
pub struct Finished<'a> {
    pub failed: bool,
    pub result: &'a str,
}

/// Runs the post hooks that match `tool` after the call ran. `None` when nothing
/// is worth saying: no hook matched, or each was silent and succeeded.
pub async fn after(
    hooks: &HooksSettings,
    ws: &Workspace,
    shell: Shell,
    tool: &str,
    input: &Value,
    finished: Finished<'_>,
    cancel: &CancellationToken,
) -> Option<Note> {
    let stdin = json!({
        "event": "postToolUse",
        "tool": tool,
        "input": input,
        "failed": finished.failed,
        "result": clip(finished.result, RESULT_BYTES),
    })
    .to_string();

    let mut note = Note {
        for_human: String::new(),
        for_model: String::new(),
    };
    for hook in hooks.post_tool_use.iter().filter(|h| h.applies_to(tool)) {
        let env = [("YACA_TOOL", tool)];
        let said = match run(hook.command(), hook.timeout(), ws, shell, &env, stdin.as_bytes(), cancel).await {
            Ending::Exited { success: true, output, .. } if output.trim().is_empty() => continue,
            Ending::Exited { success: true, output, .. } => Said {
                human: format!("Hook \"{}\" after call:\n{}\n", hook.command(), clip(&output, FEED_BYTES)),
                model: format!(
                    "\n\n[hook] Output of `{}` after this call:\n{}",
                    hook.command(),
                    clip(&output, MODEL_BYTES)
                ),
            },
            Ending::Exited { code, output, .. } => Said {
                human: format!(
                    "Hook \"{}\" after call {}\n{}\n",
                    hook.command(),
                    english_exited(code),
                    clip(&output, FEED_BYTES)
                ),
                model: format!(
                    "\n\n[hook] `{}` {} after this call, so it is only a note on the call: the call itself is not affected. Its output:\n{}",
                    hook.command(),
                    english_exited(code),
                    clip(&output, MODEL_BYTES)
                ),
            },
            Ending::TimedOut { secs, .. } => Said {
                human: format!("Hook \"{}\" after call did not respond within {secs} s\n", hook.command()),
                model: format!(
                    "\n\n[hook] `{}` did not finish within {secs} s after this call, so it is only a note on the call.",
                    hook.command()
                ),
            },
            Ending::Broken(why) => Said {
                human: format!("Hook \"{}\" after call failed to start: {why}\n", hook.command()),
                model: format!(
                    "\n\n[hook] `{}` could not be started after this call ({why}), so it is only a note on the call.",
                    hook.command()
                ),
            },
            // The turn is being stopped. Nothing is said to a model that will
            // not be asked again.
            Ending::Stopped => return None,
        };
        note.for_human.push_str(&said.human);
        note.for_model.push_str(&said.model);
    }

    (!note.for_model.is_empty()).then_some(note)
}

struct Said {
    human: String,
    model: String,
}

fn refusal_by_exit(hook: &HookSettings, code: Option<i32>, output: &str) -> Refusal {
    let command = hook.command();
    Refusal {
        for_human: format!(
            "Hook \"{command}\" blocked the call: {}\n{}",
            english_exited(code),
            clip(body(output), FEED_BYTES)
        ),
        for_model: format!(
            "[hook] The PreToolUse hook `{command}` refused this call: it {}. The call was not run, and nothing it would have done was done. The hook's output follows:\n{}\n\
             Do not repeat the call unchanged and do not reach the same result another way. Change the call to what the hook allows, or tell the user what is in the way.",
            english_exited(code),
            clip(body(output), MODEL_BYTES)
        ),
    }
}

fn refusal_by_timeout(hook: &HookSettings, secs: u64) -> Refusal {
    let command = hook.command();
    Refusal {
        for_human: format!("Hook \"{command}\" did not respond within {secs} s, call was not executed"),
        for_model: format!(
            "[hook] The PreToolUse hook `{command}` did not finish within {secs} s, so the call was refused and not run. A hook that does not answer counts as a refusal. Tell the user, or change the call."
        ),
    }
}

fn refusal_by_start(hook: &HookSettings, why: &str) -> Refusal {
    let command = hook.command();
    Refusal {
        for_human: format!("Hook \"{command}\" failed to start: {why}; call was not executed"),
        for_model: format!(
            "[hook] The PreToolUse hook `{command}` could not be started ({why}), so the call was refused and not run. Tell the user, or change the call."
        ),
    }
}

fn english_exited(code: Option<i32>) -> String {
    match code {
        Some(code) => format!("exited with code {code}"),
        None => "was ended by a signal".to_string(),
    }
}

fn body(output: &str) -> &str {
    if output.trim().is_empty() {
        "(no output)"
    } else {
        output
    }
}

/// Cuts text to a byte budget at a character boundary, saying that it was cut.
fn clip(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut cut = max;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!(
        "{}\n[... output cut at {max} bytes]",
        text[..cut].trim_end()
    )
}

/// How a hook process ended.
enum Ending {
    Exited {
        code: Option<i32>,
        success: bool,
        output: String,
    },
    /// No output is kept: a hook that did not finish is refused or noted by its
    /// timeout alone, and what it printed before then is not what the verdict
    /// rests on.
    TimedOut {
        secs: u64,
    },
    Stopped,
    /// Could not be started or waited for.
    Broken(String),
}

/// Why the wait for a hook was cut short.
#[derive(Clone, Copy)]
enum Halt {
    Stopped,
    TimedOut,
}

/// Runs one hook to its end, its timeout, or a stop.
///
/// The output is read before the exit status, and both are bounded by the same
/// deadline: a command that closes its pipes and keeps running must not hold the
/// turn past the timeout any more than one that keeps talking.
async fn run(
    command: &str,
    limit: std::time::Duration,
    ws: &Workspace,
    shell: Shell,
    env: &[(&str, &str)],
    stdin: &[u8],
    cancel: &CancellationToken,
) -> Ending {
    // Hooks are the project's own (`.yaca/settings.json`) and run in the
    // primary root, whatever additional roots the session has.
    let root = ws.root().display().to_string();
    let mut env = env.to_vec();
    env.push(("YACA_PROJECT_ROOT", root.as_str()));
    let shell::Spawned {
        mut child,
        pid,
        mut output,
    } = match shell::spawn_with(shell, command, ws.root(), &env, Some(stdin.to_vec())) {
        Ok(spawned) => spawned,
        Err(err) => return Ending::Broken(err.to_string()),
    };

    let deadline = tokio::time::sleep(limit);
    tokio::pin!(deadline);

    let mut captured = Vec::new();
    let mut halt = None;
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => { halt = Some(Halt::Stopped); break; }
            _ = &mut deadline => { halt = Some(Halt::TimedOut); break; }
            chunk = output.recv() => match chunk {
                Some(raw) => {
                    let room = CAPTURE_BYTES.saturating_sub(captured.len());
                    captured.extend_from_slice(&raw[..room.min(raw.len())]);
                }
                // Both pipes at EOF: the command has said all it will say.
                None => break,
            },
        }
    }

    let halt = match halt {
        Some(halt) => halt,
        None => tokio::select! {
            biased;
            _ = cancel.cancelled() => Halt::Stopped,
            _ = &mut deadline => Halt::TimedOut,
            status = child.wait() => {
                return match status {
                    Ok(status) => Ending::Exited {
                        code: status.code(),
                        success: status.success(),
                        output: render(&captured),
                    },
                    Err(err) => Ending::Broken(err.to_string()),
                };
            }
        },
    };

    // The tree, not the shell: a hook that started something must not leave it
    // running after the call has been decided.
    if let Some(pid) = pid {
        let _ = tokio::task::spawn_blocking(move || kill::tree(pid)).await;
    }
    let _ = tokio::time::timeout(std::time::Duration::from_secs(REAP_SECS), child.wait()).await;

    match halt {
        Halt::Stopped => Ending::Stopped,
        Halt::TimedOut => Ending::TimedOut {
            secs: limit.as_secs(),
        },
    }
}

fn render(captured: &[u8]) -> String {
    String::from_utf8_lossy(captured)
        .replace("\r\n", "\n")
        .trim_end()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::HookSettings;
    use crate::testutil::TempDir;

    fn hook(matcher: &str, command: &str, timeout_secs: Option<u64>) -> HookSettings {
        HookSettings {
            matcher: matcher.into(),
            command: command.into(),
            timeout_secs,
        }
    }

    /// A command that prints `text` and exits with `code`, in whichever shell
    /// the platform runs commands through.
    fn echo_and_exit(text: &str, code: i32) -> String {
        format!("echo {text}; exit {code}")
    }

    #[test]
    fn a_hook_applies_to_its_tool_and_to_everything_under_a_star() {
        assert!(hook("Bash", "x", None).applies_to("Bash"));
        assert!(!hook("Bash", "x", None).applies_to("Edit"));
        assert!(hook("*", "x", None).applies_to("Edit"));
    }

    #[test]
    fn wants_post_is_true_only_when_a_post_hook_matches() {
        let hooks = HooksSettings {
            pre_tool_use: vec![hook("Write", "x", None)],
            post_tool_use: vec![hook("Edit", "x", None)],
            ..HooksSettings::default()
        };
        assert!(wants_post(&hooks, "Edit"));
        assert!(
            !wants_post(&hooks, "Write"),
            "a pre hook is not a post hook"
        );
    }

    #[tokio::test]
    async fn a_pre_hook_that_exits_zero_lets_the_call_go_on() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let hooks = HooksSettings {
            pre_tool_use: vec![hook("*", "echo ok", None)],
            ..HooksSettings::default()
        };
        let outcome = before(
            &hooks,
            &ws,
            Shell::default(),
            "Bash",
            &json!({}),
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(outcome, PreOutcome::Go(None));
    }

    #[tokio::test]
    async fn a_pre_hook_that_exits_non_zero_refuses_and_says_why() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let hooks = HooksSettings {
            pre_tool_use: vec![hook("Bash", &echo_and_exit("no-rm-rf", 2), None)],
            ..HooksSettings::default()
        };
        let outcome = before(
            &hooks,
            &ws,
            Shell::default(),
            "Bash",
            &json!({ "command": "rm -rf /" }),
            &CancellationToken::new(),
        )
        .await;
        let PreOutcome::Refused(refusal) = outcome else {
            panic!("expected a refusal, got {outcome:?}");
        };
        assert!(
            refusal.for_human.contains("blocked the call"),
            "{}",
            refusal.for_human
        );
        assert!(
            refusal.for_human.contains("no-rm-rf"),
            "{}",
            refusal.for_human
        );
        assert!(
            refusal.for_model.contains("exited with code 2"),
            "{}",
            refusal.for_model
        );
        assert!(
            refusal.for_model.contains("was not run"),
            "{}",
            refusal.for_model
        );
    }

    #[tokio::test]
    async fn a_pre_hook_that_does_not_match_the_tool_is_not_run() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        // Would refuse everything if it ran: only matching is what lets it pass.
        let hooks = HooksSettings {
            pre_tool_use: vec![hook("Bash", &echo_and_exit("nope", 1), None)],
            ..HooksSettings::default()
        };
        let outcome = before(
            &hooks,
            &ws,
            Shell::default(),
            "Edit",
            &json!({}),
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(outcome, PreOutcome::Go(None));
    }

    #[tokio::test]
    async fn a_pre_hook_that_does_not_finish_refuses_the_call() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let sleep = if cfg!(windows) {
            "Start-Sleep -Seconds 20"
        } else {
            "sleep 20"
        };
        let hooks = HooksSettings {
            pre_tool_use: vec![hook("*", sleep, Some(1))],
            ..HooksSettings::default()
        };
        let started = std::time::Instant::now();
        let outcome = before(
            &hooks,
            &ws,
            Shell::default(),
            "Bash",
            &json!({}),
            &CancellationToken::new(),
        )
        .await;

        // Fail closed: a hook that never answered is a refusal, not a pass.
        assert!(started.elapsed() < std::time::Duration::from_secs(15));
        let PreOutcome::Refused(refusal) = outcome else {
            panic!("a hook that timed out must refuse: {outcome:?}");
        };
        assert!(
            refusal.for_model.contains("did not finish within 1 s"),
            "{}",
            refusal.for_model
        );
        assert!(
            refusal.for_human.contains("did not respond within 1 s"),
            "{}",
            refusal.for_human
        );
    }

    #[tokio::test]
    async fn a_pre_hook_that_cannot_start_refuses_the_call() {
        // The project directory vanishes after the workspace was opened, so the
        // hook has no directory to start in. That is a hook that cannot start,
        // and it must refuse rather than let the call through unchecked.
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let hooks = HooksSettings {
            pre_tool_use: vec![hook("*", "echo never", None)],
            ..HooksSettings::default()
        };
        drop(dir);
        let outcome = before(
            &hooks,
            &ws,
            Shell::default(),
            "Bash",
            &json!({}),
            &CancellationToken::new(),
        )
        .await;
        let PreOutcome::Refused(refusal) = outcome else {
            panic!("a hook that cannot start must refuse: {outcome:?}");
        };
        assert!(
            refusal.for_human.contains("failed to start"),
            "{}",
            refusal.for_human
        );
    }

    #[tokio::test]
    async fn a_stopped_turn_stops_a_pre_hook_without_a_verdict() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let hooks = HooksSettings {
            pre_tool_use: vec![hook("*", "echo never", None)],
            ..HooksSettings::default()
        };
        let outcome = before(&hooks, &ws, Shell::default(), "Bash", &json!({}), &cancel).await;
        assert_eq!(outcome, PreOutcome::Stopped);
    }

    #[tokio::test]
    async fn a_post_hook_output_is_what_the_model_reads_after_the_result() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let hooks = HooksSettings {
            post_tool_use: vec![hook("Write", "echo lint-clean", None)],
            ..HooksSettings::default()
        };
        let note = after(
            &hooks,
            &ws,
            Shell::default(),
            "Write",
            &json!({ "path": "a.rs" }),
            Finished {
                failed: false,
                result: "File a.rs written",
            },
            &CancellationToken::new(),
        )
        .await
        .expect("the hook printed something");
        assert!(note.for_model.contains("lint-clean"), "{}", note.for_model);
        assert!(note.for_human.contains("lint-clean"), "{}", note.for_human);
    }

    #[tokio::test]
    async fn a_failing_post_hook_is_a_note_with_its_exit_code() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let hooks = HooksSettings {
            post_tool_use: vec![hook("*", &echo_and_exit("formatter-broke", 3), None)],
            ..HooksSettings::default()
        };
        let note = after(
            &hooks,
            &ws,
            Shell::default(),
            "Edit",
            &json!({}),
            Finished {
                failed: false,
                result: "ok",
            },
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(
            note.for_model.contains("exited with code 3"),
            "{}",
            note.for_model
        );
        assert!(
            note.for_model.contains("only a note on the call"),
            "{}",
            note.for_model
        );
        assert!(note.for_human.contains("exited with code 3"), "{}", note.for_human);
    }

    #[tokio::test]
    async fn a_silent_successful_post_hook_adds_nothing() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let hooks = HooksSettings {
            post_tool_use: vec![hook("*", "exit 0", None)],
            ..HooksSettings::default()
        };
        let note = after(
            &hooks,
            &ws,
            Shell::default(),
            "Edit",
            &json!({}),
            Finished {
                failed: false,
                result: "ok",
            },
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(note, None);
    }

    #[test]
    fn a_long_output_is_cut_at_a_character_boundary() {
        let text = "é".repeat(3_000); // two bytes each
        let cut = clip(&text, 101);
        assert!(cut.contains("output cut at 101 bytes"), "{cut}");
        assert!(cut.len() <= 101 + 40, "{}", cut.len());
    }

    #[tokio::test]
    async fn the_hook_environment_names_the_tool() {
        // Checked through the real process: a hook that prints its environment
        // is the only observer that sees what the shell was given.
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let command = if cfg!(windows) {
            "echo $env:YACA_TOOL"
        } else {
            "printf %s \"$YACA_TOOL\""
        };
        let note = after(
            &HooksSettings {
                post_tool_use: vec![hook("*", command, None)],
                ..HooksSettings::default()
            },
            &ws,
            Shell::default(),
            "Grep",
            &json!({}),
            Finished {
                failed: false,
                result: "",
            },
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(note.for_model.contains("Grep"), "{}", note.for_model);
    }

    #[tokio::test]
    async fn a_pre_hook_may_rewrite_the_call() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let hooks = HooksSettings {
            pre_tool_use: vec![hook("Bash", r#"echo '{"updatedInput":{"command":"ls -la"}}'"#, None)],
            ..HooksSettings::default()
        };
        let outcome = before(
            &hooks,
            &ws,
            Shell::default(),
            "Bash",
            &json!({ "command": "ls" }),
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(outcome, PreOutcome::Go(Some(json!({ "command": "ls -la" }))));
    }

    #[test]
    fn only_an_object_is_taken_as_updated_input() {
        assert_eq!(updated_input(r#"{"updatedInput":{"a":1}}"#), Some(json!({ "a": 1 })));
        assert_eq!(updated_input(r#"{"updatedInput":"x"}"#), None);
        assert_eq!(updated_input("plain words"), None);
    }

    #[tokio::test]
    async fn event_hooks_report_how_each_ended() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let list = vec![
            EventHookSettings { command: echo_and_exit("fine", 0), timeout_secs: None },
            EventHookSettings { command: echo_and_exit("tests-fail", 1), timeout_secs: None },
        ];
        let said = on_event(&list, &ws, Shell::default(), "stop", json!({}), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(said.len(), 2);
        assert!(matches!(&said[0].ending, EventEnding::Passed(out) if out.contains("fine")));
        assert!(matches!(&said[1].ending, EventEnding::Failed { code: Some(1), .. }));
        assert!(said[1].output_for_model().contains("tests-fail"));
        assert!(said[1].failure_for_human().unwrap().contains("exited with code 1"));
        assert!(said[0].failure_for_human().is_none());
    }
}
