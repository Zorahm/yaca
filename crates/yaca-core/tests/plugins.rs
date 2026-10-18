//! Plugins end to end (`plugin`): a plugin loaded from a directory joins the
//! session through `SessionConfig::plugins` — its skills, its commands, its MCP
//! servers and its hooks — and the hooks act at the moments Claude Code's do.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use yaca_core::event::AgentEvent;
use yaca_core::mcp::McpStatus;
use yaca_core::message::ContentBlock;
use yaca_core::plugin::PluginSources;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Mode, SessionConfig, SessionHandle};

use common::{ask, calls, fixture, is_turn_over, tool_result_of, until_within, TempProject};

const LIMIT: Duration = Duration::from_secs(60);

/// A configuration directory holding one Claude Code plugin, `tools`.
fn tools_plugin(config: &TempProject) {
    config.write("plugins/tools/.claude-plugin/plugin.json", r#"{ "name": "tools" }"#);
}

fn hooks(config: &TempProject, doc: Value) {
    config.write("plugins/tools/hooks/hooks.json", &doc.to_string());
}

/// A hook that runs `script` in the platform's own shell, so the tests need no
/// bash.
fn native(script: &str) -> Value {
    if cfg!(windows) {
        json!({ "type": "command", "command": "cmd", "args": ["/d", "/c", script] })
    } else {
        json!({ "type": "command", "command": "sh", "args": ["-c", script] })
    }
}

fn on(event: &str, matcher: Option<&str>, hook: Value) -> Value {
    let mut group = json!({ "hooks": [hook] });
    if let Some(m) = matcher {
        group["matcher"] = json!(m);
    }
    json!({ "hooks": { event: [group] } })
}

fn open(dir: &TempProject, config: &TempProject, provider: Arc<MockProvider>, mode: Mode) -> SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = mode;
    cfg.journal = None;
    cfg.plugins = PluginSources::new(config.path(), None).load().enabled();
    assert!(!cfg.plugins.is_empty(), "the plugin did not load");
    yaca_core::spawn(cfg, provider).unwrap()
}

async fn one_turn(handle: &SessionHandle, text: &str) -> Vec<AgentEvent> {
    let mut rx = handle.subscribe();
    ask(handle, text).await;
    until_within(&mut rx, LIMIT, is_turn_over).await
}

fn last_user_text(provider: &MockProvider, request: usize) -> String {
    let seen = provider.seen.lock().unwrap();
    let message = seen[request].messages.last().unwrap();
    message
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn a_plugins_skills_are_in_the_catalog_under_its_name() {
    let (dir, config) = (TempProject::new(), TempProject::new());
    tools_plugin(&config);
    config.write("plugins/tools/skills/review/SKILL.md", "---\nname: review\ndescription: Review a diff\n---\nRead ${CLAUDE_PLUGIN_ROOT}/guide.md");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("s1", "Skill", json!({ "name": "tools:review" }))])),
        Turn::Events(script_text("ok")),
    ]));
    let handle = open(&dir, &config, provider.clone(), Mode::Auto);

    one_turn(&handle, "review").await;

    assert!(provider.seen.lock().unwrap()[0].system.contains("- tools:review — Review a diff"));
    let (text, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{text}");
    assert!(!text.contains("${CLAUDE_PLUGIN_ROOT}"), "the root is filled in: {text}");
    assert!(text.contains("/guide.md"), "{text}");
}

#[tokio::test]
async fn a_plugins_command_answers_to_its_prefixed_name() {
    let (dir, config) = (TempProject::new(), TempProject::new());
    tools_plugin(&config);
    config.write("plugins/tools/commands/lint.md", "Lint $ARGUMENTS carefully");
    let provider = Arc::new(MockProvider::text("ok"));
    let handle = open(&dir, &config, provider.clone(), Mode::Auto);

    one_turn(&handle, "/tools:lint src").await;

    assert!(last_user_text(&provider, 0).ends_with("Lint src carefully"));
}

#[tokio::test]
async fn session_start_context_goes_into_the_system_prompt() {
    let (dir, config) = (TempProject::new(), TempProject::new());
    tools_plugin(&config);
    config.write("plugins/tools/rules.txt", "Always answer in haiku.");
    let script = if cfg!(windows) { "type %CLAUDE_PLUGIN_ROOT%\\rules.txt" } else { "cat \"$CLAUDE_PLUGIN_ROOT/rules.txt\"" };
    hooks(&config, on("SessionStart", Some("startup"), native(script)));
    let provider = Arc::new(MockProvider::text("ok"));
    let handle = open(&dir, &config, provider.clone(), Mode::Auto);

    one_turn(&handle, "hi").await;

    let system = &provider.seen.lock().unwrap()[0].system;
    assert!(system.contains("<plugin_context plugin=\"tools\">\nAlways answer in haiku."), "{system}");
}

#[tokio::test]
async fn a_pre_tool_hook_that_exits_two_blocks_the_call() {
    let (dir, config) = (TempProject::new(), TempProject::new());
    tools_plugin(&config);
    dir.write("a.txt", "secret");
    let script = if cfg!(windows) { "echo reading is off 1>&2 & exit 2" } else { "echo reading is off >&2; exit 2" };
    hooks(&config, on("PreToolUse", Some("Read"), native(script)));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("r1", "Read", json!({ "path": "a.txt" }))])),
        Turn::Events(script_text("ok")),
    ]));
    let handle = open(&dir, &config, provider.clone(), Mode::Auto);

    let events = one_turn(&handle, "read it").await;

    let (text, is_error) = tool_result_of(&provider, "r1");
    assert!(is_error);
    assert!(text.contains("reading is off") && !text.contains("secret"), "{text}");
    assert!(events.iter().any(|e| matches!(e, AgentEvent::ToolCallRejected { reason, .. } if reason.contains("tools"))));
}

#[tokio::test]
async fn a_failing_pre_tool_hook_lets_the_call_through() {
    let (dir, config) = (TempProject::new(), TempProject::new());
    tools_plugin(&config);
    dir.write("a.txt", "content");
    hooks(&config, on("PreToolUse", None, native("exit 1")));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("r1", "Read", json!({ "path": "a.txt" }))])),
        Turn::Events(script_text("ok")),
    ]));
    let handle = open(&dir, &config, provider.clone(), Mode::Auto);

    one_turn(&handle, "read it").await;

    let (text, is_error) = tool_result_of(&provider, "r1");
    assert!(!is_error, "fail-open, as in Claude Code: {text}");
    assert!(text.contains("content"));
}

#[tokio::test]
async fn a_post_tool_hook_adds_its_context_to_the_result() {
    let (dir, config) = (TempProject::new(), TempProject::new());
    tools_plugin(&config);
    dir.write("a.txt", "content");
    config.write(
        "plugins/tools/answer.json",
        r#"{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"run the linter next"}}"#,
    );
    let script = if cfg!(windows) { "type %CLAUDE_PLUGIN_ROOT%\\answer.json" } else { "cat \"$CLAUDE_PLUGIN_ROOT/answer.json\"" };
    hooks(&config, on("PostToolUse", Some("Read|Edit"), native(script)));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("r1", "Read", json!({ "path": "a.txt" }))])),
        Turn::Events(script_text("ok")),
    ]));
    let handle = open(&dir, &config, provider.clone(), Mode::Auto);

    one_turn(&handle, "read it").await;

    let (text, _) = tool_result_of(&provider, "r1");
    assert!(text.contains("content") && text.contains("[hook] From the tools plugin:\nrun the linter next"), "{text}");
}

#[tokio::test]
async fn a_prompt_hook_that_blocks_keeps_the_message_from_the_model() {
    let (dir, config) = (TempProject::new(), TempProject::new());
    tools_plugin(&config);
    let script = if cfg!(windows) { "echo not today 1>&2 & exit 2" } else { "echo not today >&2; exit 2" };
    hooks(&config, on("UserPromptSubmit", None, native(script)));
    let provider = Arc::new(MockProvider::text("never sent"));
    let handle = open(&dir, &config, provider.clone(), Mode::Auto);

    let events = one_turn(&handle, "do it").await;

    assert!(provider.seen.lock().unwrap().is_empty(), "the model was asked nothing");
    let reported = events.iter().find_map(|e| match e {
        AgentEvent::HookReported { plugin, event, text, blocked } => Some((plugin, event, text, *blocked)),
        _ => None,
    });
    let (plugin, event, text, blocked) = reported.expect("the block is in the feed");
    assert_eq!((plugin.as_str(), event.as_str(), blocked), ("tools", "UserPromptSubmit", true));
    assert_eq!(text, "not today");
    assert!(events.iter().any(|e| matches!(e, AgentEvent::UserMessage { .. })));
}

#[tokio::test]
async fn a_stop_hook_sends_the_turn_back_once_with_its_reason() {
    let (dir, config) = (TempProject::new(), TempProject::new());
    tools_plugin(&config);
    // Refuses the first stop and lets the second through, by a file in the
    // project it runs in.
    let script = if cfg!(windows) {
        "if exist stopped.txt (exit 0) else (echo x>stopped.txt & echo write the tests too 1>&2 & exit 2)"
    } else {
        "[ -f stopped.txt ] && exit 0; touch stopped.txt; echo write the tests too >&2; exit 2"
    };
    hooks(&config, on("Stop", None, native(script)));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(script_text("done")),
        Turn::Events(script_text("tests written")),
    ]));
    let handle = open(&dir, &config, provider.clone(), Mode::Auto);

    let events = one_turn(&handle, "build it").await;

    assert_eq!(provider.seen.lock().unwrap().len(), 2);
    assert!(last_user_text(&provider, 1).contains("write the tests too"));
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::HookReported { event, blocked: true, .. } if event == "Stop")));
}

#[tokio::test]
async fn a_plugins_mcp_server_starts_without_approval_under_its_prefixed_name() {
    let (dir, config) = (TempProject::new(), TempProject::new());
    tools_plugin(&config);
    config.write(
        "plugins/tools/.mcp.json",
        &json!({ "mcpServers": { "fx": { "command": fixture("mcp_fixture") } } }).to_string(),
    );
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("m1", "mcp__plugin_tools_fx__echo", json!({ "text": "hi" }))])),
        Turn::Events(script_text("ok")),
    ]));
    let handle = open(&dir, &config, provider.clone(), Mode::Auto);

    one_turn(&handle, "echo").await;

    let server = handle.mcp().into_iter().find(|s| s.name == "plugin:tools:fx").expect("declared");
    assert_eq!(server.status, McpStatus::Ready);
    let (text, is_error) = tool_result_of(&provider, "m1");
    assert!(!is_error, "{text}");
    assert!(text.contains("hi"));
}

#[tokio::test]
async fn a_pre_tool_hook_can_ask_for_the_question_auto_would_skip() {
    let (dir, config) = (TempProject::new(), TempProject::new());
    tools_plugin(&config);
    config.write(
        "plugins/tools/ask.json",
        r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask"}}"#,
    );
    let script = if cfg!(windows) { r"type %CLAUDE_PLUGIN_ROOT%\ask.json" } else { "cat \"$CLAUDE_PLUGIN_ROOT/ask.json\"" };
    hooks(&config, on("PreToolUse", Some("Write"), native(script)));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("w1", "Write", json!({ "path": "new.txt", "content": "x" }))])),
        Turn::Events(script_text("ok")),
    ]));
    let handle = open(&dir, &config, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "write it").await;
    let seen = until_within(&mut rx, LIMIT, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. }) || is_turn_over(e)
    })
    .await;

    assert!(
        matches!(seen.last(), Some(AgentEvent::PermissionRequested { .. })),
        "Auto writes without asking, unless a hook asks"
    );
    assert!(!dir.path().join("new.txt").exists());
}
