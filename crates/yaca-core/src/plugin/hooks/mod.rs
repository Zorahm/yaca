//! Hooks that plugins bring, in Claude Code's format and with its semantics.
//!
//! A second runner beside the project's own hooks (`crate::hooks`), not a
//! translation into them, because the two disagree on purpose. A project hook is
//! the human's guard and fails closed: a check that cannot run refuses the call.
//! A plugin hook was written for Claude Code, where only exit code 2 or an
//! explicit `deny` blocks and every other failure is a note; plugins rely on that
//! (they are written fail-open), so here it is the rule too. What the two share
//! is the boundary of trust: no hook grants anything. `permissionDecision:
//! "allow"` is ignored, and `"ask"` can only add a question the mode would have
//! skipped.
//!
//! Nine events: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
//! `Stop`, `SubagentStop`, `PreCompact`, `SessionEnd`, `Notification`. The
//! others Claude Code knows have no moment in Yaca and are reported on the
//! plugin rather than silently never run.
//!
//! Tool names are matched as Claude Code names them, which are mostly Yaca's
//! own. Where they differ ([`claude_names`]) a matcher written for Claude still
//! finds the call: `PowerShell` reaches Yaca's `Bash`, which is what runs
//! PowerShell on Windows.

mod output;
mod run;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

pub use output::Verdict;
use run::Ran;

use crate::workspace::roots::plain;

/// Claude Code's default for a command hook.
const DEFAULT_TIMEOUT_SECS: u64 = 60;
const MAX_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    Stop,
    SubagentStop,
    PreCompact,
    SessionEnd,
    Notification,
}

impl HookEvent {
    pub fn name(self) -> &'static str {
        match self {
            HookEvent::SessionStart => "SessionStart",
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::Stop => "Stop",
            HookEvent::SubagentStop => "SubagentStop",
            HookEvent::PreCompact => "PreCompact",
            HookEvent::SessionEnd => "SessionEnd",
            HookEvent::Notification => "Notification",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "SessionStart" => HookEvent::SessionStart,
            "UserPromptSubmit" => HookEvent::UserPromptSubmit,
            "PreToolUse" => HookEvent::PreToolUse,
            "PostToolUse" => HookEvent::PostToolUse,
            "Stop" => HookEvent::Stop,
            "SubagentStop" => HookEvent::SubagentStop,
            "PreCompact" => HookEvent::PreCompact,
            "SessionEnd" => HookEvent::SessionEnd,
            "Notification" => HookEvent::Notification,
            _ => return None,
        })
    }
}

/// How a hook's command is started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookRun {
    /// A command line for a shell. Claude Code's hooks are written for bash, on
    /// Windows too, so bash it is unless the hook asks for PowerShell.
    Shell { command: String, powershell: bool },
    /// `command` with `args`: a program started directly, no shell between.
    Exec { program: String, args: Vec<String> },
}

/// One hook a plugin declared.
#[derive(Debug, Clone)]
pub struct PluginHook {
    pub event: HookEvent,
    /// `None` matches everything.
    matcher: Option<Regex>,
    pub run: HookRun,
    /// Claude Code's `if`: a permission-rule pattern, `Bash(gh pr *)`.
    condition: Option<Condition>,
    pub timeout: Duration,
    /// `async`: started and not waited for. Its answer is never read.
    pub background: bool,
}

#[derive(Debug, Clone)]
struct Condition {
    tool: String,
    pattern: Option<Regex>,
}

/// Reads a `hooks.json` document, or the same object inline in a manifest.
/// What cannot be used is a note, and the rest is kept.
pub fn parse(doc: &Value, notes: &mut Vec<String>) -> Vec<PluginHook> {
    let events = doc.get("hooks").and_then(Value::as_object).or_else(|| doc.as_object());
    let Some(events) = events else {
        notes.push("хуки: ожидался объект событий".into());
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, groups) in events {
        if name == "description" {
            continue;
        }
        let Some(event) = HookEvent::from_name(name) else {
            notes.push(format!("хуки {name} в Yaca не поддерживаются"));
            continue;
        };
        for group in groups.as_array().into_iter().flatten() {
            let matcher = match group.get("matcher").and_then(Value::as_str).map(str::trim) {
                None | Some("") | Some("*") => None,
                Some(m) => match Regex::new(&format!("^(?:{m})$")) {
                    Ok(re) => Some(re),
                    Err(_) => {
                        notes.push(format!("хук {name}: matcher «{m}» не разбирается и пропущен"));
                        continue;
                    }
                },
            };
            for hook in group.get("hooks").and_then(Value::as_array).into_iter().flatten() {
                match one(event, matcher.clone(), hook) {
                    Ok(hook) => out.push(hook),
                    Err(why) => notes.push(format!("хук {name}: {why}")),
                }
            }
        }
    }
    out
}

fn one(event: HookEvent, matcher: Option<Regex>, hook: &Value) -> Result<PluginHook, String> {
    match hook.get("type").and_then(Value::as_str) {
        Some("command") | None => {}
        Some(other) => return Err(format!("тип «{other}» не поддерживается")),
    }
    let command = hook
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .ok_or("нет command")?;
    let run = match hook.get("args").and_then(Value::as_array) {
        Some(args) => HookRun::Exec {
            program: command.to_string(),
            args: args.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        },
        None => HookRun::Shell {
            command: command.to_string(),
            powershell: hook.get("shell").and_then(Value::as_str) == Some("powershell"),
        },
    };
    let secs = hook
        .get("timeout")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
        .clamp(1, MAX_TIMEOUT_SECS);
    let condition = match hook.get("if").and_then(Value::as_str) {
        Some(rule) => Some(condition(rule).ok_or(format!("условие if «{rule}» не разбирается"))?),
        None => None,
    };
    Ok(PluginHook {
        event,
        matcher,
        run,
        condition,
        timeout: Duration::from_secs(secs),
        background: hook.get("async").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// `Tool` or `Tool(pattern)`, with `*` as any run of characters and the
/// older `prefix:*` spelling read as `prefix*`.
fn condition(rule: &str) -> Option<Condition> {
    let rule = rule.trim();
    let Some(open) = rule.find('(') else {
        return Some(Condition {
            tool: rule.to_string(),
            pattern: None,
        });
    };
    let inner = rule[open + 1..].strip_suffix(')')?;
    let inner = inner.strip_suffix(":*").map(|p| format!("{p}*")).unwrap_or(inner.to_string());
    let glob = regex::escape(&inner).replace(r"\*", ".*");
    Some(Condition {
        tool: rule[..open].trim().to_string(),
        pattern: Some(Regex::new(&format!("^{glob}$")).ok()?),
    })
}

/// The names Claude Code gives the call a Yaca tool makes, first the one the
/// hook is told. Identity for every tool the two name alike, MCP tools too.
pub fn claude_names(tool: &str) -> &[&str] {
    match tool {
        "Bash" => &["Bash", "PowerShell"],
        "Edit" => &["Edit", "MultiEdit"],
        "AskUser" => &["AskUserQuestion"],
        "Task" => &["Task", "Agent"],
        "BashKill" => &["KillShell", "KillBash"],
        "BashOutput" => &["BashOutput", "TaskOutput"],
        _ => &[],
    }
}

fn names_of(tool: &str) -> Vec<&str> {
    let mapped = claude_names(tool);
    if mapped.is_empty() {
        vec![tool]
    } else {
        mapped.to_vec()
    }
}

/// What a hook is matched against.
pub enum Subject<'a> {
    Tool { name: &'a str, input: &'a Value },
    /// What the event's matcher is matched against: `SessionStart`'s
    /// `startup` or `resume`, `PreCompact`'s `manual` or `auto`, `SessionEnd`'s
    /// reason, `Notification`'s type.
    Source(&'a str),
    Nothing,
}

impl PluginHook {
    fn applies(&self, subject: &Subject) -> bool {
        let matched = match (&self.matcher, subject) {
            (None, _) => true,
            (Some(re), Subject::Tool { name, .. }) => names_of(name).iter().any(|n| re.is_match(n)),
            (Some(re), Subject::Source(source)) => re.is_match(source),
            // Prompt and stop events have nothing to match; Claude Code ignores
            // their matcher too.
            (Some(_), Subject::Nothing) => true,
        };
        matched && self.condition_holds(subject)
    }

    fn condition_holds(&self, subject: &Subject) -> bool {
        let Some(cond) = &self.condition else { return true };
        let Subject::Tool { name, input } = subject else { return true };
        if !names_of(name).contains(&cond.tool.as_str()) {
            return false;
        }
        let Some(pattern) = &cond.pattern else { return true };
        let target = ["command", "file_path", "path", "url", "pattern"]
            .iter()
            .find_map(|key| input.get(key).and_then(Value::as_str))
            .unwrap_or_default();
        pattern.is_match(target)
    }
}

/// The plugin a hook came from, as its process is told.
#[derive(Debug, Clone)]
pub struct HookOwner {
    pub name: String,
    pub root: PathBuf,
    pub data: PathBuf,
}

/// Where the call is being made from, for the hook's input.
pub struct Context<'a> {
    pub project: &'a Path,
    pub session_id: &'a str,
    pub transcript: Option<&'a Path>,
    /// Claude Code's name for the mode: `plan`, `default`, `acceptEdits`,
    /// `bypassPermissions`.
    pub permission_mode: &'a str,
}

/// What one hook said, with the plugin it belongs to.
pub struct Said {
    pub plugin: String,
    pub verdict: Verdict,
}

/// Every plugin hook of a session.
#[derive(Debug, Clone, Default)]
pub struct Hooks {
    entries: Vec<(Arc<HookOwner>, PluginHook)>,
}

impl Hooks {
    pub fn new(plugins: &[super::Plugin]) -> Self {
        let mut entries = Vec::new();
        for plugin in plugins {
            let owner = Arc::new(HookOwner {
                name: plugin.name.clone(),
                root: plugin.root.clone(),
                data: plugin.data.clone(),
            });
            entries.extend(plugin.hooks.iter().map(|h| (owner.clone(), h.clone())));
        }
        Hooks { entries }
    }

    pub fn has(&self, event: HookEvent) -> bool {
        self.entries.iter().any(|(_, h)| h.event == event)
    }

    /// Runs the hooks of `event` that apply to `subject`, one after another in
    /// the order the plugins declare them, and returns what each said. `extra`
    /// joins the input every hook is given.
    pub async fn fire(
        &self,
        event: HookEvent,
        subject: Subject<'_>,
        extra: Value,
        ctx: &Context<'_>,
        cancel: &CancellationToken,
    ) -> Vec<Said> {
        let mut stdin = json!({
            "session_id": ctx.session_id,
            "transcript_path": ctx.transcript.map(plain).unwrap_or_default(),
            "cwd": plain(ctx.project),
            "permission_mode": ctx.permission_mode,
            "hook_event_name": event.name(),
        });
        if let (Value::Object(base), Value::Object(more)) = (&mut stdin, extra) {
            base.extend(more);
        }
        let stdin = stdin.to_string();

        let mut out = Vec::new();
        for (owner, hook) in self.entries.iter().filter(|(_, h)| h.event == event && h.applies(&subject)) {
            if cancel.is_cancelled() {
                break;
            }
            let ran = run::run(hook, owner, ctx.project, stdin.clone(), cancel).await;
            if matches!(ran, Ran::Stopped) {
                break;
            }
            out.push(Said {
                plugin: owner.name.clone(),
                verdict: output::verdict(event, &ran),
            });
        }
        out
    }
}

/// A Yaca tool's input as Claude Code's hooks expect it: the file tools name
/// their file `file_path`, absolute.
pub fn claude_input(input: &Value, project: &Path) -> Value {
    let mut input = input.clone();
    if let Value::Object(fields) = &mut input {
        if let Some(path) = fields.get("path").and_then(Value::as_str).map(str::to_string) {
            if !fields.contains_key("file_path") {
                let abs = project.join(&path);
                fields.insert("file_path".into(), Value::String(plain(&abs)));
            }
        }
    }
    input
}

#[cfg(test)]
mod tests;
