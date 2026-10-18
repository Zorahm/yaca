//! Project settings: `.yaca/settings.json` (§9, §10).
//!
//! §9 says every project keeps its own things under `.yaca/` — settings,
//! permission rules, history, the shadow repository. This is the first of
//! those.
//!
//! Only what the core actually consults lives here. Appearance, keyboard and
//! the provider list belong to the application rather than to one project, and
//! a field nothing reads is a setting that lies about having an effect.
//!
//! The core reads this file and never writes it on its own. §5.7 is explicit
//! that a rule granted in a hurry does not outlive the hurry: "Always allow"
//! and "Always deny" are session-lived, and a rule reaches `settings.json`
//! through the settings window (§10) and nowhere else. [`Settings::save`] is
//! what that window calls, so the shape of the file is written down once.
//! Editing it by hand still works — plain JSON with every field optional.
//!
//! A file that does not parse stops the session. Falling back to defaults would
//! be safe in the narrow sense — fewer rules, less trust — but it would leave a
//! human believing a rule applies when a typo has quietly dropped it.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::CoreError;
use crate::intent::Mode;
use crate::mcp::McpServerSettings;
use crate::permission::{Refusal, ScopeKey};

/// Where the file sits under the project root.
pub const FILE: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The mode a session starts in (§8, §10). Absent means Manual — Yaca runs
    /// with any model, weak local ones included, so the safe behaviour is the
    /// one you get without asking for it.
    pub mode: Option<Mode>,
    /// The provider a session starts with (§10).
    pub provider: Option<String>,
    /// The model a session starts with (§10).
    pub model: Option<String>,
    /// The model that summarizes history (§5.11, §10). Absent means the
    /// session's own model.
    pub compact_model: Option<String>,
    /// The model subagents run on (§10). Absent means the session's own model.
    /// Served by the session's provider, like the one above.
    pub subagent_model: Option<String>,
    pub permissions: PermissionSettings,
    pub shadow: ShadowSettings,
    /// The project's own check, run after a batch of calls that changed files
    /// (see `verify.rs`). Absent means nothing runs after an edit.
    pub verify: Option<VerifySettings>,
    /// Commands run around every tool call that matches them (see `hooks.rs`).
    /// Absent means no hooks, and a project that never wrote the section gets
    /// exactly the calls it got before. Read it through [`Settings::hooks`],
    /// which gives the empty set for both absent and `null`.
    #[ts(optional)]
    pub hooks: Option<HooksSettings>,
    /// How many requests to the model one turn may make before it is stopped
    /// (see `session/guard.rs`). Absent means [`Settings::DEFAULT_MAX_STEPS`].
    #[ts(optional)]
    pub max_steps: Option<u32>,
    /// MCP servers of this project, by name (see `mcp::config`). On top of the
    /// human's own list and the project's `.mcp.json`, and replacing a server
    /// of the same name from either.
    ///
    /// A declaration that cannot be used does not stop the session, unlike a
    /// broken hook: the server is listed as failed with the reason, and the
    /// session works without its tools.
    pub mcp_servers: BTreeMap<String, McpServerSettings>,
}

/// `"hooks": { "preToolUse": [...], "postToolUse": [...], "stop": [...] }` in
/// `.yaca/settings.json`.
///
/// Trust is the same as for `verify`: the human wrote the command in the file
/// they own, so the core runs it without a permission question. What a hook may
/// do is narrower, and that is the reason for the two lists. A pre hook can only
/// refuse a call that was about to run; nothing a hook says can grant one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct HooksSettings {
    /// Run before a call, ahead of the permission question. Exit code 0 lets the
    /// call go on; anything else refuses it, and the model reads the output.
    pub pre_tool_use: Vec<HookSettings>,
    /// Run after a call has run. Its output is added to what the model reads as
    /// the result, and a failure is a note on the call, not a failure of it.
    pub post_tool_use: Vec<HookSettings>,
    /// Run when the session opens. What they print is standing context for
    /// the model, in the system prompt; a failure is a line in the feed.
    pub session_start: Vec<EventHookSettings>,
    /// Run before each message of the human's reaches the model. Exit code 0
    /// lets it through, with what they print added to it; anything else keeps
    /// it from the model, and the human sees why.
    pub user_prompt_submit: Vec<EventHookSettings>,
    /// Run when the model is done with a turn. Exit code 0 lets the turn end;
    /// a non-zero exit sends the model back to work with the hook's output,
    /// a few times at most. A hook that cannot run is only a note: the model
    /// could do nothing about it.
    pub stop: Vec<EventHookSettings>,
    /// The same for a subagent about to hand in its report.
    pub subagent_stop: Vec<EventHookSettings>,
    /// Run before history is compacted. Informational: a failure is a note.
    pub pre_compact: Vec<EventHookSettings>,
    /// Run when the session closes. Informational.
    pub session_end: Vec<EventHookSettings>,
    /// Run when Yaca starts waiting for the human: a permission question, a
    /// question from the agent, a form from an MCP server. Informational, and
    /// not waited for: a notifier must not delay the question it announces.
    pub notification: Vec<EventHookSettings>,
}

/// One hook of an event that is not about a tool call, so it has no `match`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EventHookSettings {
    /// The command line, run through the shell Bash uses, with
    /// `YACA_HOOK_EVENT` and `YACA_PROJECT_ROOT` in its environment and a JSON
    /// description of the event on its standard input.
    pub command: String,
    /// Seconds before the command is stopped. Absent means
    /// [`HookSettings::DEFAULT_TIMEOUT_SECS`].
    pub timeout_secs: Option<u64>,
}

impl EventHookSettings {
    pub fn command(&self) -> &str {
        self.command.trim()
    }

    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_secs.unwrap_or(HookSettings::DEFAULT_TIMEOUT_SECS))
    }
}

/// One entry of a hook list. Both keys are required: an entry without a
/// `match` would not say which calls it applies to, and one without a
/// `command` would have nothing to run.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct HookSettings {
    /// `*` for every tool, or one tool name exactly as the registry names it
    /// (`Bash`, `Edit`, `Write`). Checked at load: a name no tool answers to
    /// would be a hook that silently never runs.
    #[serde(rename = "match")]
    pub matcher: String,
    /// The command line, run through the shell Bash uses, with `YACA_TOOL` and
    /// `YACA_PROJECT_ROOT` in its environment and a JSON description of the call
    /// on its standard input.
    pub command: String,
    /// Seconds before the command is stopped. Absent means
    /// [`HookSettings::DEFAULT_TIMEOUT_SECS`].
    pub timeout_secs: Option<u64>,
}

impl HookSettings {
    /// Short on purpose: a hook is a check that should answer quickly, and one
    /// that does not answer in this time is treated as refusing the call (see
    /// `hooks.rs`), so a long default would stall every matching call.
    pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
    /// The same ceiling as for `verify`.
    pub const MAX_TIMEOUT_SECS: u64 = 600;

    pub fn command(&self) -> &str {
        self.command.trim()
    }

    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_secs.unwrap_or(Self::DEFAULT_TIMEOUT_SECS))
    }

    /// Whether this hook runs for a call to `tool`.
    pub fn applies_to(&self, tool: &str) -> bool {
        self.matcher == "*" || self.matcher == tool
    }
}

impl HooksSettings {
    /// Why these hooks cannot be used, in words a human can act on. Checked at
    /// load for the same reason `verify` is: a broken entry should stop the
    /// session that would have run it, not fail some later call.
    fn validate(&self) -> Result<(), String> {
        let registry = crate::tool::ToolRegistry::standard();
        let lists = [
            ("preToolUse", &self.pre_tool_use),
            ("postToolUse", &self.post_tool_use),
        ];
        for (phase, list) in lists {
            for (at, hook) in list.iter().enumerate() {
                let here = format!("hooks.{phase}[{at}]");
                // An MCP tool is named by its server, which is not running
                // when the file is read: the prefix is all that can be checked.
                let known_name = registry.names().any(|name| name == hook.matcher)
                    || crate::mcp::RESOURCE_TOOLS.contains(&hook.matcher.as_str())
                    || hook.matcher.starts_with(crate::mcp::TOOL_PREFIX);
                if hook.matcher != "*" && !known_name {
                    let known = registry.names().collect::<Vec<_>>().join(", ");
                    return Err(format!(
                        "{here}.match: \"{}\" - no such tool; allowed: * or one of names: {known}",
                        hook.matcher
                    ));
                }
                if hook.command().is_empty() {
                    return Err(format!("{here}.command is empty: specify a command or remove the entry"));
                }
                check_timeout(&here, hook.timeout_secs)?;
            }
        }
        for (event, list) in self.events() {
            for (at, hook) in list.iter().enumerate() {
                let here = format!("hooks.{event}[{at}]");
                if hook.command().is_empty() {
                    return Err(format!("{here}.command is empty: specify a command or remove the entry"));
                }
                check_timeout(&here, hook.timeout_secs)?;
            }
        }
        Ok(())
    }

    /// The lists of the events that are not about a tool call, by the name
    /// they have in the file.
    pub fn events(&self) -> [(&'static str, &Vec<EventHookSettings>); 7] {
        [
            ("sessionStart", &self.session_start),
            ("userPromptSubmit", &self.user_prompt_submit),
            ("stop", &self.stop),
            ("subagentStop", &self.subagent_stop),
            ("preCompact", &self.pre_compact),
            ("sessionEnd", &self.session_end),
            ("notification", &self.notification),
        ]
    }
}

fn check_timeout(here: &str, secs: Option<u64>) -> Result<(), String> {
    match secs {
        Some(secs) if !(1..=HookSettings::MAX_TIMEOUT_SECS).contains(&secs) => Err(format!(
            "{here}.timeoutSecs must be between 1 and {} s, got {secs}",
            HookSettings::MAX_TIMEOUT_SECS
        )),
        _ => Ok(()),
    }
}

/// `"verify": { "command": "cargo check --message-format short" }` in
/// `.yaca/settings.json`.
///
/// The command is written by the human, in the file they own, and the core runs
/// it without asking the permission broker: a question before every check would
/// make an edit cost a prompt. That is the trust this field grants, and the
/// reason it lives here rather than as a tool the model can call.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct VerifySettings {
    /// The command line, run through the shell Bash uses. Required: an empty
    /// `verify` object is refused rather than read as "no check", because a
    /// human who wrote one meant to have a check.
    pub command: String,
    /// Seconds before the command is stopped and reported as timed out.
    /// Absent means [`VerifySettings::DEFAULT_TIMEOUT_SECS`].
    pub timeout_secs: Option<u64>,
    /// Directory the command runs in, relative to the project root. Absent
    /// means the root. Kept inside the project by refusing anything that is
    /// not a plain relative path.
    pub cwd: Option<String>,
}

impl VerifySettings {
    /// Two minutes: a type check or a unit-test run finishes well inside it;
    /// one that does not is usually waiting on something.
    pub const DEFAULT_TIMEOUT_SECS: u64 = 120;
    /// The same ceiling Bash puts on a foreground command.
    pub const MAX_TIMEOUT_SECS: u64 = 600;

    /// The command without the whitespace around it.
    pub fn command(&self) -> &str {
        self.command.trim()
    }

    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_secs.unwrap_or(Self::DEFAULT_TIMEOUT_SECS))
    }

    pub fn cwd(&self) -> &str {
        self.cwd.as_deref().unwrap_or(".")
    }

    /// Why these settings cannot be used, in words a human can act on. Checked
    /// at load: a bad value that surfaced only when an edit happened would
    /// fail that edit, far from the line that caused it.
    fn validate(&self) -> Result<(), String> {
        if self.command().is_empty() {
            return Err("verify.command is empty: specify a command or remove verify entirely".into());
        }
        if let Some(secs) = self.timeout_secs {
            if !(1..=Self::MAX_TIMEOUT_SECS).contains(&secs) {
                return Err(format!(
                    "verify.timeoutSecs must be between 1 and {} s, got {secs}",
                    Self::MAX_TIMEOUT_SECS
                ));
            }
        }
        if let Some(cwd) = &self.cwd {
            if !is_project_relative(cwd) {
                return Err(format!(
                    "verify.cwd must be a path inside the project, got \"{cwd}\""
                ));
            }
        }
        Ok(())
    }
}

/// A plain relative path: no root, no drive, no `..`. Anything else could
/// name a directory outside the project, and the human wrote a command, not
/// a place for it to run.
fn is_project_relative(raw: &str) -> bool {
    use std::path::Component;

    !raw.trim().is_empty()
        && Path::new(raw)
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct PermissionSettings {
    /// Standing rules, in the same shape "Always allow" offers (§5.7). These
    /// are the ones that survive a restart.
    pub rules: Vec<ScopeKey>,
    /// Standing refusals, in the shape "Always deny" offers. Kept apart from
    /// `rules` rather than folded into one list with a sign: a refusal carries
    /// a reason and a grant does not, and the two lists answer different
    /// questions — "what happens without me" and "what never happens".
    ///
    /// A refusal outranks a grant that covers the same call, and it is
    /// consulted before the mode, so bypass means "stop asking me", not "do
    /// what I said no to". That order lives in
    /// [`crate::permission::broker::Broker::standing`]; the file only decides
    /// which rules are in play.
    pub denies: Vec<Refusal>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct ShadowSettings {
    /// Directory names the snapshot walk does not enter, on top of the
    /// built-in list (§10, "exclusions of the shadow repository").
    ///
    /// Added to, never replacing: `.git` and `.yaca` are not negotiable, and a
    /// project that dropped them from its list would be snapshotting its own
    /// snapshots.
    pub exclude: Vec<String>,
}

impl Settings {
    /// What a turn may spend when the project does not say. Matches the
    /// subagent's ceiling: a task that needs more went round in circles or was
    /// too wide to be one turn, and the human's "continue" costs one message.
    pub const DEFAULT_MAX_STEPS: u32 = 100;

    /// Reads the file, or returns the defaults when there is none.
    pub fn load(root: &Path) -> Result<Settings, CoreError> {
        let path = crate::project::dir(root).join(FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
            Err(e) => {
                return Err(CoreError::Config(format!(
                    "failed to read {}: {e}",
                    path.display()
                )))
            }
        };

        let settings: Settings = serde_json::from_str(&text).map_err(|e| {
            CoreError::Config(format!("failed to parse {}: {e}", path.display()))
        })?;
        if let Some(verify) = &settings.verify {
            verify
                .validate()
                .map_err(|why| CoreError::Config(format!("{}: {why}", path.display())))?;
        }
        if let Some(hooks) = &settings.hooks {
            hooks
                .validate()
                .map_err(|why| CoreError::Config(format!("{}: {why}", path.display())))?;
        }
        Ok(settings)
    }

    /// Writes the file, creating `.yaca/` if needed.
    ///
    /// Nothing in the core calls this — the settings window does (§10). It
    /// lives here so that the window does not have to know the shape of the
    /// file a second time.
    pub fn save(&self, root: &Path) -> Result<(), CoreError> {
        let dir = crate::project::dir(root);
        std::fs::create_dir_all(&dir)
            .map_err(|e| CoreError::Config(format!("failed to create {}: {e}", dir.display())))?;
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| CoreError::Config(format!("failed to serialize settings: {e}")))?;
        std::fs::write(dir.join(FILE), text)
            .map_err(|e| CoreError::Config(format!("failed to write settings: {e}")))
    }

    /// The step ceiling of one turn. Never below two: with one, a turn could
    /// not read a file and then answer about it.
    pub fn max_steps(&self) -> u32 {
        self.max_steps.unwrap_or(Self::DEFAULT_MAX_STEPS).max(2)
    }

    pub fn mode_or_default(&self) -> Mode {
        self.mode.unwrap_or_default()
    }

    /// The project's hooks, or none. Borrowed from a static when the file has
    /// no section, so callers never have to decide between "absent" and "empty".
    pub fn hooks(&self) -> &HooksSettings {
        static NONE: HooksSettings = HooksSettings {
            pre_tool_use: Vec::new(),
            post_tool_use: Vec::new(),
            session_start: Vec::new(),
            user_prompt_submit: Vec::new(),
            stop: Vec::new(),
            subagent_stop: Vec::new(),
            pre_compact: Vec::new(),
            session_end: Vec::new(),
            notification: Vec::new(),
        };
        self.hooks.as_ref().unwrap_or(&NONE)
    }

    /// Which model summarizes history, given the session's own model (§5.11).
    pub fn compact_model_or<'a>(&'a self, session_model: &'a str) -> &'a str {
        self.compact_model.as_deref().unwrap_or(session_model)
    }

    /// Which model a subagent runs on, given the session's own model.
    pub fn subagent_model_or<'a>(&'a self, session_model: &'a str) -> &'a str {
        self.subagent_model.as_deref().unwrap_or(session_model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn a_project_without_a_file_gets_the_defaults() {
        let dir = TempDir::new();
        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.mode_or_default(), Mode::Auto);
    }

    #[test]
    fn a_half_written_file_still_loads() {
        let dir = TempDir::new();
        dir.write(".yaca/settings.json", r#"{ "mode": "auto" }"#);

        // Every field is optional, so a hand-written file may name one thing.
        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.mode, Some(Mode::Auto));
        assert!(settings.permissions.rules.is_empty());
    }

    #[test]
    fn a_broken_file_stops_the_session_instead_of_being_ignored() {
        let dir = TempDir::new();
        dir.write(".yaca/settings.json", "{ mode: auto }");

        // Silently defaulting would leave a human believing a rule applies
        // when a typo has dropped it.
        let err = Settings::load(dir.path()).unwrap_err();
        assert!(matches!(err, CoreError::Config(_)), "got {err:?}");
    }

    #[test]
    fn standing_rules_round_trip() {
        let dir = TempDir::new();
        let written = Settings {
            mode: Some(Mode::Edits),
            permissions: PermissionSettings {
                rules: vec![
                    ScopeKey::CommandPrefix {
                        prefix: "cargo".into(),
                    },
                    ScopeKey::Directory { path: "src".into() },
                ],
                denies: vec![
                    Refusal::new(
                        ScopeKey::CommandPrefix {
                            prefix: "git commit".into(),
                        },
                        Some("I make commits myself".into()),
                    ),
                    Refusal::new(
                        ScopeKey::Directory {
                            path: ".yaca".into(),
                        },
                        None,
                    ),
                ],
            },
            shadow: ShadowSettings {
                exclude: vec!["fixtures".into()],
            },
            ..Settings::default()
        };
        written.save(dir.path()).unwrap();

        assert_eq!(Settings::load(dir.path()).unwrap(), written);
    }

    /// A refusal with no reason survives the round trip, and one written by
    /// hand without the key at all parses too — the file is edited both ways.
    #[test]
    fn a_refusal_without_a_reason_round_trips() {
        let dir = TempDir::new();
        let written = Settings {
            permissions: PermissionSettings {
                // The canonical tool name: reading a `ScopeKey::Tool` runs it
                // through the legacy rename table, so a file naming the old
                // spelling comes back as the new one.
                denies: vec![Refusal::new(
                    ScopeKey::Tool {
                        name: "WebFetch".into(),
                    },
                    None,
                )],
                ..PermissionSettings::default()
            },
            ..Settings::default()
        };
        written.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()).unwrap(), written);

        // And the same refusal written by hand, with no `reason` key at all.
        dir.write(
            ".yaca/settings.json",
            r#"{ "permissions": { "denies": [{ "scope": { "type": "tool", "name": "WebFetch" } }] } }"#,
        );
        assert_eq!(Settings::load(dir.path()).unwrap(), written);
    }

    /// A file written before refusals existed loads with none of them, rather
    /// than refusing to parse.
    #[test]
    fn a_file_without_denies_still_loads() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "permissions": { "rules": [{ "type": "directory", "path": "src" }] } }"#,
        );

        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.permissions.rules.len(), 1);
        assert!(settings.permissions.denies.is_empty());
    }

    #[test]
    fn a_project_without_verify_has_no_check() {
        let dir = TempDir::new();
        dir.write(".yaca/settings.json", r#"{ "mode": "auto" }"#);
        assert_eq!(Settings::load(dir.path()).unwrap().verify, None);
    }

    #[test]
    fn verify_takes_a_command_and_defaults_the_rest() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "verify": { "command": "  cargo check  " } }"#,
        );

        let verify = Settings::load(dir.path()).unwrap().verify.unwrap();
        assert_eq!(verify.command(), "cargo check");
        assert_eq!(verify.timeout(), std::time::Duration::from_secs(120));
        assert_eq!(verify.cwd(), ".");
    }

    #[test]
    fn verify_reads_its_timeout_and_directory() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "verify": { "command": "npm test", "timeoutSecs": 30, "cwd": "web" } }"#,
        );

        let verify = Settings::load(dir.path()).unwrap().verify.unwrap();
        assert_eq!(verify.timeout(), std::time::Duration::from_secs(30));
        assert_eq!(verify.cwd(), "web");
    }

    /// Unknown keys are ignored everywhere else in the file, and `verify` is no
    /// exception: a key from a newer Yaca must not stop an older one.
    #[test]
    fn verify_ignores_keys_it_does_not_know() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "verify": { "command": "cargo check", "somethingNew": true } }"#,
        );
        assert!(Settings::load(dir.path()).unwrap().verify.is_some());
    }

    #[test]
    fn an_empty_verify_command_is_refused_at_load() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "verify": { "command": "  " } }"#,
        );
        let err = Settings::load(dir.path()).unwrap_err();
        assert!(matches!(err, CoreError::Config(_)), "got {err:?}");
    }

    #[test]
    fn a_verify_timeout_outside_the_range_is_refused_at_load() {
        for bad in ["0", "601"] {
            let dir = TempDir::new();
            dir.write(
                ".yaca/settings.json",
                &format!(r#"{{ "verify": {{ "command": "x", "timeoutSecs": {bad} }} }}"#),
            );
            let err = Settings::load(dir.path()).unwrap_err();
            assert!(matches!(err, CoreError::Config(_)), "{bad}: got {err:?}");
        }
    }

    #[test]
    fn a_verify_directory_outside_the_project_is_refused_at_load() {
        for bad in ["..", "../other", "/etc", "", "a/../../b"] {
            let dir = TempDir::new();
            dir.write(
                ".yaca/settings.json",
                &serde_json::json!({ "verify": { "command": "x", "cwd": bad } }).to_string(),
            );
            let err = Settings::load(dir.path()).unwrap_err();
            assert!(matches!(err, CoreError::Config(_)), "{bad:?}: got {err:?}");
        }
    }

    #[test]
    fn a_project_without_hooks_runs_nothing_around_a_call() {
        let dir = TempDir::new();
        dir.write(".yaca/settings.json", r#"{ "mode": "auto" }"#);
        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.hooks, None);
        assert_eq!(settings.hooks(), &HooksSettings::default());
    }

    #[test]
    fn hooks_read_their_matcher_command_and_timeout() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "hooks": {
                "preToolUse": [{ "match": "Bash", "command": " guard.sh ", "timeoutSecs": 5 }],
                "postToolUse": [{ "match": "*", "command": "lint.sh" }]
            } }"#,
        );

        let settings = Settings::load(dir.path()).unwrap();
        let hooks = settings.hooks();
        let pre = &hooks.pre_tool_use[0];
        assert_eq!(pre.matcher, "Bash");
        assert_eq!(pre.command(), "guard.sh");
        assert_eq!(pre.timeout(), std::time::Duration::from_secs(5));
        assert!(pre.applies_to("Bash") && !pre.applies_to("Edit"));

        let post = &hooks.post_tool_use[0];
        assert_eq!(post.timeout(), std::time::Duration::from_secs(30));
        assert!(post.applies_to("Edit"), "a star matches every tool");
    }

    #[test]
    fn a_hook_round_trips_with_its_match_key() {
        let dir = TempDir::new();
        let written = Settings {
            hooks: Some(HooksSettings {
                pre_tool_use: vec![HookSettings {
                    matcher: "Write".into(),
                    command: "check".into(),
                    timeout_secs: Some(10),
                }],
                ..HooksSettings::default()
            }),
            ..Settings::default()
        };
        written.save(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".yaca/settings.json")).unwrap();
        assert!(text.contains("\"match\": \"Write\""), "{text}");
        assert_eq!(Settings::load(dir.path()).unwrap(), written);
    }

    #[test]
    fn a_hook_for_a_tool_no_one_has_is_refused_at_load() {
        // A name the registry does not know would never match a call: the hook
        // would be a line in the file that never runs, which is a lie about
        // what is checked.
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "hooks": { "preToolUse": [{ "match": "ReadFile", "command": "x" }] } }"#,
        );
        let err = Settings::load(dir.path()).unwrap_err();
        let CoreError::Config(why) = &err else {
            panic!("got {err:?}");
        };
        assert!(why.contains("hooks.preToolUse[0].match"), "{why}");
        assert!(why.contains("Bash") && why.contains("Skill"), "lists the real names: {why}");
    }

    #[test]
    fn event_hooks_need_no_match_and_are_checked_too() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "hooks": { "stop": [{ "command": "cargo test -q" }], "notification": [{ "command": "notify-send yaca" }] } }"#,
        );
        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.hooks().stop[0].command(), "cargo test -q");
        assert_eq!(settings.hooks().notification.len(), 1);

        dir.write(".yaca/settings.json", r#"{ "hooks": { "preCompact": [{ "command": "  " }] } }"#);
        let err = Settings::load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("hooks.preCompact[0].command"), "{err}");
    }

    #[test]
    fn a_hook_may_name_an_mcp_tool() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "hooks": { "preToolUse": [
                { "match": "mcp__tracker__create_task", "command": "x" },
                { "match": "ReadMcpResource", "command": "x" }
            ] } }"#,
        );
        assert_eq!(Settings::load(dir.path()).unwrap().hooks().pre_tool_use.len(), 2);
    }

    #[test]
    fn mcp_servers_are_read_by_name() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "mcpServers": {
                "files": { "command": "server-files", "args": ["."] },
                "off": { "url": "https://example.test/mcp", "disabled": true }
            } }"#,
        );
        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.mcp_servers["files"].args, ["."]);
        assert!(settings.mcp_servers["off"].disabled);

        settings.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()).unwrap(), settings);
    }

    #[test]
    fn a_hook_with_an_empty_command_or_a_bad_timeout_is_refused_at_load() {
        let cases = [
            r#"{ "hooks": { "postToolUse": [{ "match": "*", "command": "  " }] } }"#,
            r#"{ "hooks": { "postToolUse": [{ "match": "*", "command": "x", "timeoutSecs": 0 }] } }"#,
            r#"{ "hooks": { "postToolUse": [{ "match": "*", "command": "x", "timeoutSecs": 601 }] } }"#,
        ];
        for text in cases {
            let dir = TempDir::new();
            dir.write(".yaca/settings.json", text);
            let err = Settings::load(dir.path()).unwrap_err();
            assert!(matches!(err, CoreError::Config(_)), "{text}: got {err:?}");
        }
    }

    #[test]
    fn a_hook_without_its_match_or_its_command_is_refused_at_load() {
        // Both keys are required: without a match there is no saying which calls
        // the hook is for, and without a command there is nothing to run.
        for text in [
            r#"{ "hooks": { "preToolUse": [{ "command": "x" }] } }"#,
            r#"{ "hooks": { "preToolUse": [{ "match": "Bash" }] } }"#,
        ] {
            let dir = TempDir::new();
            dir.write(".yaca/settings.json", text);
            let err = Settings::load(dir.path()).unwrap_err();
            assert!(matches!(err, CoreError::Config(_)), "{text}: got {err:?}");
        }
    }

    #[test]
    fn the_summarizing_model_falls_back_to_the_session_model() {
        let mut settings = Settings::default();
        assert_eq!(settings.compact_model_or("opus"), "opus");
        settings.compact_model = Some("haiku".into());
        assert_eq!(settings.compact_model_or("opus"), "haiku");
    }
}
