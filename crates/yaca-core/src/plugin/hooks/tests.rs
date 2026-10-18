//! Plugin hooks: how `hooks.json` is read, what a hook applies to, and real
//! processes answering the way Claude Code's hooks do.

use serde_json::{json, Value};

use super::*;
use crate::testutil::TempDir;

fn parse_one(hook: Value, event: &str, matcher: Option<&str>) -> PluginHook {
    let mut group = json!({ "hooks": [hook] });
    if let Some(m) = matcher {
        group["matcher"] = json!(m);
    }
    let doc = json!({ "hooks": { event: [group] } });
    let mut notes = Vec::new();
    let mut hooks = parse(&doc, &mut notes);
    assert!(notes.is_empty(), "{notes:?}");
    hooks.remove(0)
}

fn tool<'a>(name: &'a str, input: &'a Value) -> Subject<'a> {
    Subject::Tool { name, input }
}

#[test]
fn the_two_forms_of_a_command_are_told_apart() {
    let shell = parse_one(json!({ "type": "command", "command": "echo hi" }), "Stop", None);
    assert_eq!(shell.run, HookRun::Shell { command: "echo hi".into(), powershell: false });
    let exec = parse_one(json!({ "command": "node", "args": ["a.mjs"] }), "Stop", None);
    assert_eq!(exec.run, HookRun::Exec { program: "node".into(), args: vec!["a.mjs".into()] });
    let ps = parse_one(json!({ "command": "Write-Output 1", "shell": "powershell" }), "Stop", None);
    assert!(matches!(ps.run, HookRun::Shell { powershell: true, .. }));
}

#[test]
fn timeout_and_async_are_read_and_the_timeout_is_bounded() {
    let h = parse_one(json!({ "command": "x", "timeout": 5, "async": true }), "Stop", None);
    assert_eq!(h.timeout.as_secs(), 5);
    assert!(h.background);
    let long = parse_one(json!({ "command": "x", "timeout": 100000 }), "Stop", None);
    assert_eq!(long.timeout.as_secs(), MAX_TIMEOUT_SECS);
    assert_eq!(parse_one(json!({ "command": "x" }), "Stop", None).timeout.as_secs(), DEFAULT_TIMEOUT_SECS);
}

#[test]
fn what_yaca_has_no_moment_for_is_a_note() {
    let doc = json!({ "hooks": {
        "Setup": [{ "hooks": [{ "command": "x" }] }],
        "Stop": [{ "hooks": [{ "type": "agent", "prompt": "?" }] }],
        "PreToolUse": [{ "matcher": "(", "hooks": [{ "command": "x" }] }],
    }, "description": "ignored" });
    let mut notes = Vec::new();
    assert!(parse(&doc, &mut notes).is_empty());
    assert_eq!(notes.len(), 3, "{notes:?}");
}

#[test]
fn matchers_are_claude_codes_regexes_over_claude_codes_tool_names() {
    let none = json!({});
    let edits = parse_one(json!({ "command": "x" }), "PostToolUse", Some("Edit|Write"));
    assert!(edits.applies(&tool("Edit", &none)));
    assert!(edits.applies(&tool("Write", &none)));
    assert!(!edits.applies(&tool("Read", &none)));

    // On Windows Claude Code calls its shell tool PowerShell; Yaca's Bash is it.
    let ps = parse_one(json!({ "command": "x" }), "PreToolUse", Some("PowerShell"));
    assert!(ps.applies(&tool("Bash", &none)));
    let ask = parse_one(json!({ "command": "x" }), "PreToolUse", Some("AskUserQuestion"));
    assert!(ask.applies(&tool("AskUser", &none)));

    let mcp = parse_one(json!({ "command": "x" }), "PreToolUse", Some("mcp__.*__create_pull_request"));
    assert!(mcp.applies(&tool("mcp__github__create_pull_request", &none)));
    assert!(!mcp.applies(&tool("mcp__github__list", &none)));

    let all = parse_one(json!({ "command": "x" }), "PreToolUse", Some("*"));
    assert!(all.applies(&tool("Anything", &none)));
}

#[test]
fn a_session_start_matcher_is_matched_against_the_source() {
    let h = parse_one(json!({ "command": "x" }), "SessionStart", Some("startup|clear|compact"));
    assert!(h.applies(&Subject::Source("startup")));
    assert!(!h.applies(&Subject::Source("resume")));
}

#[test]
fn an_if_condition_narrows_a_hook_to_the_calls_it_names() {
    let gh = parse_one(json!({ "command": "x", "if": "Bash(gh pr *)" }), "PreToolUse", Some("Bash"));
    assert!(gh.applies(&tool("Bash", &json!({ "command": "gh pr create --fill" }))));
    assert!(!gh.applies(&tool("Bash", &json!({ "command": "git status" }))));

    let npm = parse_one(json!({ "command": "x", "if": "Bash(npm:*)" }), "PreToolUse", None);
    assert!(npm.applies(&tool("Bash", &json!({ "command": "npm test" }))));
    assert!(!npm.applies(&tool("Edit", &json!({ "path": "npm" }))));
}

#[test]
fn a_file_tools_input_gains_claude_codes_absolute_file_path() {
    let project = std::path::Path::new("/p");
    let input = claude_input(&json!({ "path": "src/a.rs", "old_string": "x" }), project);
    assert_eq!(input["file_path"], plain(&project.join("src/a.rs")));
    assert_eq!(input["path"], "src/a.rs");
}

/// A program that runs `script` in the platform's own shell, as a hook's
/// `command` + `args`: no bash needed.
fn native(script: &str) -> Value {
    if cfg!(windows) {
        json!({ "command": "cmd", "args": ["/d", "/c", script] })
    } else {
        json!({ "command": "sh", "args": ["-c", script] })
    }
}

fn owner(dir: &TempDir) -> HookOwner {
    HookOwner {
        name: "tools".into(),
        root: dir.path().to_path_buf(),
        data: dir.path().join("data"),
    }
}

async fn fire_one(dir: &TempDir, event: &str, hook: Value) -> Vec<Said> {
    let h = parse_one(hook, event, None);
    let hooks = Hooks {
        entries: vec![(Arc::new(owner(dir)), h)],
    };
    let ctx = Context {
        project: dir.path(),
        session_id: "s1",
        transcript: None,
        permission_mode: "default",
    };
    let event = HookEvent::from_name(event).unwrap();
    hooks
        .fire(event, Subject::Nothing, json!({}), &ctx, &CancellationToken::new())
        .await
}

#[tokio::test]
async fn a_hook_answers_with_context_in_json() {
    let dir = TempDir::new();
    // From a file: `cmd` has quoting rules of its own for an echoed JSON line.
    dir.write("answer.json", r#"{"hookSpecificOutput":{"additionalContext":"be careful"}}"#);
    let script = if cfg!(windows) { "type answer.json" } else { "cat answer.json" };
    let said = fire_one(&dir, "SessionStart", native(script)).await;
    assert_eq!(said[0].plugin, "tools");
    assert_eq!(said[0].verdict.context, ["be careful"]);
}

#[tokio::test]
async fn exit_two_blocks_with_what_the_hook_wrote_to_stderr() {
    let dir = TempDir::new();
    let script = if cfg!(windows) { "echo no PRs 1>&2 & exit 2" } else { "echo no PRs >&2; exit 2" };
    let said = fire_one(&dir, "PreToolUse", native(script)).await;
    assert_eq!(said[0].verdict.block.as_deref(), Some("no PRs"));
}

#[tokio::test]
async fn the_plugins_root_is_in_the_hooks_environment() {
    let dir = TempDir::new();
    let script = if cfg!(windows) { "echo %CLAUDE_PLUGIN_ROOT%" } else { "echo $CLAUDE_PLUGIN_ROOT" };
    let said = fire_one(&dir, "SessionStart", native(script)).await;
    let seen = said[0].verdict.context[0].replace('\\', "/");
    assert_eq!(seen, plain(dir.path()));
    assert!(dir.path().join("data").is_dir(), "PLUGIN_DATA exists before the hook runs");
}

#[tokio::test]
async fn args_name_the_plugins_root_as_text() {
    let dir = TempDir::new();
    let hook = if cfg!(windows) {
        json!({ "command": "cmd", "args": ["/d", "/c", "echo", "${CLAUDE_PLUGIN_ROOT}/x"] })
    } else {
        json!({ "command": "echo", "args": ["${CLAUDE_PLUGIN_ROOT}/x"] })
    };
    let said = fire_one(&dir, "SessionStart", hook).await;
    assert_eq!(said[0].verdict.context, [format!("{}/x", plain(dir.path()))]);
}

#[tokio::test]
async fn a_hook_that_does_not_answer_in_time_is_a_note_not_a_block() {
    let dir = TempDir::new();
    let script = if cfg!(windows) { "ping -n 6 127.0.0.1 >nul" } else { "sleep 5" };
    let mut hook = native(script);
    hook["timeout"] = json!(1);
    let started = std::time::Instant::now();
    let said = fire_one(&dir, "PreToolUse", hook).await;
    assert!(started.elapsed().as_secs() < 5, "{:?}", started.elapsed());
    assert_eq!(said[0].verdict.block, None);
    assert!(said[0].verdict.notices[0].contains("1 с"), "{:?}", said[0].verdict.notices);
}

#[tokio::test]
async fn a_program_that_does_not_exist_is_a_note() {
    let dir = TempDir::new();
    let said = fire_one(&dir, "PreToolUse", json!({ "command": "no-such-program-yaca", "args": [] })).await;
    assert_eq!(said[0].verdict.block, None);
    assert!(said[0].verdict.notices[0].contains("не запустился"));
}

#[tokio::test]
async fn a_command_line_runs_through_bash_and_reads_its_input() {
    let dir = TempDir::new();
    // Into a file: echoed back, the input is JSON and reads as the hook's answer.
    let said = fire_one(&dir, "UserPromptSubmit", json!({ "command": "cat > got.json" })).await;
    match run::bash() {
        Some(_) => {
            assert!(said[0].verdict.notices.is_empty(), "{:?}", said[0].verdict.notices);
            let got = std::fs::read_to_string(dir.path().join("got.json")).unwrap();
            let input: Value = serde_json::from_str(&got).unwrap();
            assert_eq!(input["hook_event_name"], "UserPromptSubmit");
            assert_eq!(input["session_id"], "s1");
        }
        // No bash on this machine: the hook says so instead of running elsewhere.
        None => assert!(said[0].verdict.notices[0].contains("bash")),
    }
}

#[tokio::test]
async fn an_async_hook_is_not_waited_for() {
    let dir = TempDir::new();
    let script = if cfg!(windows) { "ping -n 4 127.0.0.1 >nul" } else { "sleep 3" };
    let mut hook = native(script);
    hook["async"] = json!(true);
    let started = std::time::Instant::now();
    let said = fire_one(&dir, "SessionStart", hook).await;
    assert!(started.elapsed().as_secs() < 2);
    assert_eq!(said[0].verdict, Verdict::default());
}
