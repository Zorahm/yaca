//! MCP servers end to end: a real server process, a real session.
//!
//! Driven through the public surface only — `spawn`, `Command`, `AgentEvent` —
//! with `examples/mcp_fixture.rs` as the server on the other end of a pipe.
//! Nothing here mocks the protocol: what is tested is that a server's tool is
//! a tool of the session like any other.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::broadcast::Receiver;
use yaca_core::event::AgentEvent;
use yaca_core::mcp::{McpSource, McpStatus};
use yaca_core::permission::PermissionRequest;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{
    Command, McpServerSummary, Mode, PermissionAnswer, PermissionKind, ScopeKey, SessionConfig,
    SessionHandle,
};

use common::{ask, calls, fixture, is_turn_over, tool_result_of, until_within, TempProject};

/// A real process is started for every test, and a loaded machine starts it
/// slowly: the rope is long, and a test that passes does not wait it out.
const ROPE: Duration = Duration::from_secs(30);

/// Declares the fixture as the project's server `fixture`.
fn declare(dir: &TempProject, args: &[&str], more: Value) {
    let mut server = json!({ "command": fixture("mcp_fixture"), "args": args });
    for (key, value) in more.as_object().into_iter().flatten() {
        server[key] = value.clone();
    }
    dir.write(
        ".yaca/settings.json",
        &json!({ "mcpServers": { "fixture": server } }).to_string(),
    );
}

fn open(dir: &TempProject, provider: Arc<MockProvider>, mode: Mode) -> SessionHandle {
    common::session(dir, provider, mode)
}

async fn turn(rx: &mut Receiver<AgentEvent>) -> Vec<AgentEvent> {
    until_within(rx, ROPE, is_turn_over).await
}

/// The servers once none of them is still connecting.
async fn settled(handle: &SessionHandle) -> Vec<McpServerSummary> {
    let waited = tokio::time::timeout(ROPE, async {
        loop {
            let servers = handle.mcp();
            if servers.iter().all(|s| s.status != McpStatus::Connecting) {
                return servers;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    waited.expect("a server is still connecting")
}

fn offered(provider: &MockProvider, request: usize) -> Vec<String> {
    provider.seen.lock().unwrap()[request]
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect()
}

fn asked(events: &[AgentEvent]) -> PermissionRequest {
    events
        .iter()
        .find_map(|e| match e {
            AgentEvent::PermissionRequested { request } => Some(request.clone()),
            _ => None,
        })
        .expect("no permission was requested")
}

fn call(id: &str, tool: &str, input: Value) -> Turn {
    Turn::Events(calls(&[(id, &format!("mcp__fixture__{tool}"), input)]))
}

// --- tests ------------------------------------------------------------------

#[tokio::test]
async fn a_servers_tool_is_offered_and_called_like_any_other() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({}));
    let provider = Arc::new(MockProvider::new(vec![
        call("t1", "echo", json!({ "text": "привет", "description": "Проверил эхо" })),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "say it back").await;
    let events = turn(&mut rx).await;

    // The first request waited for the server, so the tool is in it.
    let seen = provider.seen.lock().unwrap()[0].clone();
    let echo = seen
        .tools
        .iter()
        .find(|tool| tool.name == "mcp__fixture__echo")
        .expect("the server's tool was not offered");
    assert_eq!(echo.description, "Repeats the text back.");
    // §5.4.1: an MCP call is captioned by the model, beside the server's own
    // arguments.
    assert_eq!(echo.input_schema["required"], json!(["text", "description"]));
    assert!(echo.input_schema["properties"]["text"].is_object());
    // And the prompt says whose words the results are, with the server's own.
    assert!(seen.system.contains("<mcp_servers>"), "{}", seen.system);
    assert!(seen.system.contains("Use echo to repeat a text back."));

    let started = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::ToolCallStarted {
                tool,
                description,
                target,
                input,
                ..
            } => Some((tool.clone(), description.clone(), target.clone(), input.clone())),
            _ => None,
        })
        .expect("the call has no row");
    assert_eq!(started.0, "mcp__fixture__echo");
    assert_eq!(started.1.as_deref(), Some("Проверил эхо"));
    assert_eq!(started.2.as_deref(), Some("fixture · echo"));
    // The caption is the feed's; the server is sent its own arguments only.
    assert_eq!(started.3, json!({ "text": "привет" }));

    // Read-only by the server's word: nobody was asked.
    let kinds: Vec<_> = events.iter().map(AgentEvent::kind).collect();
    assert!(!kinds.contains(&"permissionRequested"), "{kinds:?}");
    assert!(kinds.contains(&"toolCallFinished"), "{kinds:?}");
    assert_eq!(tool_result_of(&provider, "t1"), ("привет".to_string(), false));
}

#[tokio::test]
async fn a_tool_that_writes_asks_first_and_a_refusal_runs_nothing() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({}));
    let note = dir.path().join("note.txt");
    let provider = Arc::new(MockProvider::new(vec![
        call("t1", "write_note", json!({ "path": note, "text": "x", "description": "Записал" })),
        Turn::Events(script_text("Not written.")),
    ]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "write it").await;
    let asking = until_within(&mut rx, ROPE, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    let request = asked(&asking);

    // §5.7's row for MCP: the tool as the target, the arguments as the detail.
    assert_eq!(request.kind, PermissionKind::McpWrite);
    assert_eq!(request.target, "write_note");
    assert_eq!(request.server.as_deref(), Some("fixture"));
    assert!(request.detail.contains("note.txt"), "{}", request.detail);
    assert_eq!(
        request.scopes,
        vec![
            ScopeKey::Tool {
                name: "mcp__fixture__write_note".into()
            },
            ScopeKey::McpServer {
                server: "fixture".into()
            },
        ]
    );
    assert!(!note.exists(), "the question comes before the call");

    handle
        .send(Command::AnswerPermission {
            request_id: request.id,
            answer: PermissionAnswer::Deny { reason: None },
        })
        .await
        .unwrap();
    turn(&mut rx).await;

    assert!(!note.exists());
    assert!(tool_result_of(&provider, "t1").1, "a refusal reads as an error");
}

#[tokio::test]
async fn allowing_a_whole_server_stops_the_questions_about_its_tools() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({}));
    let (first, second) = (dir.path().join("a.txt"), dir.path().join("b.txt"));
    let provider = Arc::new(MockProvider::new(vec![
        call("t1", "write_note", json!({ "path": first, "text": "1", "description": "Первая" })),
        call("t2", "write_note", json!({ "path": second, "text": "2", "description": "Вторая" })),
        Turn::Events(script_text("Both written.")),
    ]));
    let handle = open(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "write both").await;
    let asking = until_within(&mut rx, ROPE, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    handle
        .send(Command::AnswerPermission {
            request_id: asked(&asking).id,
            answer: PermissionAnswer::AllowAlways {
                scope: ScopeKey::McpServer {
                    server: "fixture".into(),
                },
            },
        })
        .await
        .unwrap();

    let rest = turn(&mut rx).await;
    let questions = rest
        .iter()
        .filter(|e| matches!(e, AgentEvent::PermissionRequested { .. }))
        .count();
    assert_eq!(questions, 0, "the second call was covered by the rule");
    assert_eq!(std::fs::read_to_string(first).unwrap(), "1");
    assert_eq!(std::fs::read_to_string(second).unwrap(), "2");
}

#[tokio::test]
async fn a_tool_error_is_a_failed_call_with_the_servers_words() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({}));
    let provider = Arc::new(MockProvider::new(vec![
        call("t1", "fail", json!({ "description": "Спросил трекер" })),
        Turn::Events(script_text("It failed.")),
    ]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "try").await;
    let events = turn(&mut rx).await;

    let failure = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::ToolCallFailed { message, .. } => Some(message.clone()),
            _ => None,
        })
        .expect("the row did not fail");
    assert!(failure.contains("fixture"), "{failure}");

    let (text, is_error) = tool_result_of(&provider, "t1");
    assert!(is_error);
    assert!(text.contains("the tracker has no such task"), "{text}");
}

#[tokio::test]
async fn a_server_that_dies_on_start_is_listed_with_what_it_said() {
    let dir = TempProject::new();
    declare(&dir, &["--crash"], json!({}));
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("Hello."))]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    let servers = settled(&handle).await;
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].status, McpStatus::Failed);
    assert_eq!(servers[0].source, McpSource::Project);
    let reason = servers[0].reason.clone().unwrap_or_default();
    assert!(reason.contains("boom: the token is missing"), "{reason}");

    // The session works without it, and offers none of its tools.
    ask(&handle, "hi").await;
    let events = turn(&mut rx).await;
    assert!(events.iter().any(|e| matches!(e, AgentEvent::TurnFinished { .. })));
    assert!(offered(&provider, 0).iter().all(|name| !name.starts_with("mcp__")));
    assert!(!provider.seen.lock().unwrap()[0].system.contains("<mcp_servers>"));
}

#[tokio::test]
async fn a_server_that_never_answers_is_given_up_on_at_its_deadline() {
    let dir = TempProject::new();
    declare(&dir, &["--hang"], json!({ "startupTimeoutSecs": 1 }));
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("Hello."))]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    // Sent while the server is still starting: the turn waits for the
    // deadline and no longer.
    ask(&handle, "hi").await;
    let events = turn(&mut rx).await;
    assert!(events.iter().any(|e| matches!(e, AgentEvent::TurnFinished { .. })));

    let servers = handle.mcp();
    assert_eq!(servers[0].status, McpStatus::Failed);
    assert!(servers[0].reason.as_deref().unwrap_or_default().contains("1 с"));
    assert!(offered(&provider, 0).iter().all(|name| !name.starts_with("mcp__")));
}

#[tokio::test]
async fn a_failed_server_comes_back_when_reconnected() {
    let dir = TempProject::new();
    declare(&dir, &["--crash"], json!({}));
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("Hello."))]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    assert_eq!(settled(&handle).await[0].status, McpStatus::Failed);
    // Subscribed only now, so the first failure is not taken for the second.
    let mut rx = handle.subscribe();

    // The declaration is read when the session opens, so the same broken one
    // is tried again — and the list says so each time, which is what a button
    // in a panel needs.
    handle
        .send(Command::ReconnectMcpServer {
            server: "fixture".into(),
        })
        .await
        .unwrap();
    let seen = until_within(&mut rx, ROPE, |e| {
        matches!(e, AgentEvent::McpUpdated { servers } if servers[0].status == McpStatus::Failed)
    })
    .await;
    let passed_through_connecting = seen.iter().any(|e| {
        matches!(e, AgentEvent::McpUpdated { servers } if servers[0].status == McpStatus::Connecting)
    });
    assert!(passed_through_connecting, "the attempt was not shown");
}

#[tokio::test]
async fn a_server_from_the_projects_mcp_json_waits_for_approval() {
    let dir = TempProject::new();
    let approvals = TempProject::new();
    dir.write(
        ".mcp.json",
        &json!({ "mcpServers": { "fixture": { "command": fixture("mcp_fixture") } } }).to_string(),
    );
    let config = || {
        let mut config = SessionConfig::new(dir.path());
        config.journal = false;
        config.mcp_approvals = Some(approvals.path().to_path_buf());
        config
    };

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(script_text("Hello.")),
        Turn::Events(script_text("Hello again.")),
    ]));
    let handle = yaca_core::spawn(config(), provider.clone()).unwrap();
    let mut rx = handle.subscribe();

    // The file came with the repository; nothing from it has been started.
    let servers = settled(&handle).await;
    assert_eq!(servers[0].status, McpStatus::NeedsApproval);
    assert_eq!(servers[0].source, McpSource::ProjectFile);
    ask(&handle, "hi").await;
    turn(&mut rx).await;
    assert!(offered(&provider, 0).iter().all(|name| !name.starts_with("mcp__")));

    handle
        .send(Command::ApproveMcpServer {
            server: "fixture".into(),
        })
        .await
        .unwrap();
    until_within(&mut rx, ROPE, |e| {
        matches!(e, AgentEvent::McpUpdated { servers } if servers[0].status == McpStatus::Ready)
    })
    .await;

    // From the next turn the tools are there.
    ask(&handle, "and now").await;
    turn(&mut rx).await;
    assert!(offered(&provider, 1).contains(&"mcp__fixture__echo".to_string()));

    // And the approval outlives the session: the next one starts the server
    // without asking.
    let again = yaca_core::spawn(config(), Arc::new(MockProvider::new(vec![]))).unwrap();
    assert_eq!(settled(&again).await[0].status, McpStatus::Ready);

    // Until the declaration changes.
    dir.write(
        ".mcp.json",
        &json!({ "mcpServers": { "fixture": {
            "command": fixture("mcp_fixture"), "args": ["--something-new"],
        } } })
        .to_string(),
    );
    let changed = yaca_core::spawn(config(), Arc::new(MockProvider::new(vec![]))).unwrap();
    assert_eq!(settled(&changed).await[0].status, McpStatus::NeedsApproval);
}

#[tokio::test]
async fn a_tool_list_that_changes_applies_from_the_next_turn() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({}));
    let provider = Arc::new(MockProvider::new(vec![
        call("t1", "grow", json!({ "description": "Добавил инструмент" })),
        Turn::Events(script_text("Grown.")),
        Turn::Events(script_text("I see it.")),
    ]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "grow").await;
    let events = turn(&mut rx).await;
    // Within the turn the head of the request did not move.
    assert_eq!(offered(&provider, 0), offered(&provider, 1));
    assert!(!offered(&provider, 1).contains(&"mcp__fixture__late".to_string()));

    // The notice may arrive a moment after the turn ends.
    let knows_late = |servers: &[McpServerSummary]| servers[0].tools.iter().any(|t| t.name == "late");
    let already = events
        .iter()
        .any(|e| matches!(e, AgentEvent::McpUpdated { servers } if knows_late(servers)));
    if !already {
        until_within(&mut rx, ROPE, |e| {
            matches!(e, AgentEvent::McpUpdated { servers } if knows_late(servers))
        })
        .await;
    }

    ask(&handle, "look again").await;
    turn(&mut rx).await;
    assert!(offered(&provider, 2).contains(&"mcp__fixture__late".to_string()));
}

#[tokio::test]
async fn resources_are_listed_and_read() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({}));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            ("t1", "ListMcpResources", json!({ "description": "Посмотрел ресурсы" })),
            (
                "t2",
                "ReadMcpResource",
                json!({ "server": "fixture", "uri": "note://hello", "description": "Прочитал заметку" }),
            ),
        ])),
        Turn::Events(script_text("Read.")),
    ]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "what is there").await;
    let events = turn(&mut rx).await;
    assert!(!events.iter().any(|e| matches!(e, AgentEvent::PermissionRequested { .. })));

    let (listing, failed) = tool_result_of(&provider, "t1");
    assert!(!failed);
    assert!(listing.contains("fixture\tnote://hello\thello"), "{listing}");
    assert_eq!(
        tool_result_of(&provider, "t2"),
        ("hello from a resource".to_string(), false)
    );
}

#[tokio::test]
async fn a_tool_with_an_argument_called_description_keeps_it() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({}));
    let provider = Arc::new(MockProvider::new(vec![
        call("t1", "describe", json!({ "description": "a blue door" })),
        Turn::Events(script_text("Described.")),
    ]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "describe").await;
    turn(&mut rx).await;

    // The caption field of §5.4.1 is not added beside the server's own…
    let seen = provider.seen.lock().unwrap()[0].clone();
    let describe = seen
        .tools
        .iter()
        .find(|tool| tool.name == "mcp__fixture__describe")
        .unwrap();
    assert!(describe.input_schema.get("required").is_none());
    // …and what the model wrote there reaches the server.
    assert_eq!(tool_result_of(&provider, "t1").0, "described: a blue door");
}

#[tokio::test]
async fn a_server_is_told_the_sessions_roots() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({}));
    let provider = Arc::new(MockProvider::new(vec![
        call("t1", "roots", json!({ "description": "Спросил корни" })),
        Turn::Events(script_text("Known.")),
    ]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "where are we").await;
    turn(&mut rx).await;

    let (text, failed) = tool_result_of(&provider, "t1");
    assert!(!failed, "{text}");
    let name = dir.path().file_name().unwrap().to_string_lossy().to_string();
    assert!(text.contains("file:///"), "{text}");
    assert!(text.contains(&name), "{text}");
}

#[tokio::test]
async fn stopping_the_turn_does_not_wait_for_a_slow_server() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({}));
    let provider = Arc::new(MockProvider::new(vec![call(
        "t1",
        "slow",
        json!({ "ms": 60_000, "description": "Жду" }),
    )]));
    let handle = open(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "wait").await;
    until_within(&mut rx, ROPE, |e| matches!(e, AgentEvent::ToolCallStarted { .. })).await;
    handle.send(Command::Cancel).await.unwrap();

    // Far inside the minute the server sleeps for.
    let events = until_within(&mut rx, Duration::from_secs(10), is_turn_over).await;
    assert!(events.iter().any(|e| matches!(e, AgentEvent::TurnCancelled { .. })));
}

#[tokio::test]
async fn a_call_that_outlasts_its_deadline_fails_and_the_turn_goes_on() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({ "toolTimeoutSecs": 1 }));
    let provider = Arc::new(MockProvider::new(vec![
        call("t1", "slow", json!({ "ms": 60_000, "description": "Жду" })),
        Turn::Events(script_text("Gave up.")),
    ]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "wait").await;
    let events = turn(&mut rx).await;
    assert!(events.iter().any(|e| matches!(e, AgentEvent::TurnFinished { .. })));

    let (text, failed) = tool_result_of(&provider, "t1");
    assert!(failed);
    assert!(text.contains("1 с"), "{text}");
}

#[tokio::test]
async fn a_subagent_has_the_servers_tools_too() {
    let dir = TempProject::new();
    declare(&dir, &[], json!({}));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "Task",
            json!({ "description": "Отдал подагенту", "prompt": "echo something" }),
        )])),
        // The subagent's only answer.
        Turn::Events(script_text("Echoed.")),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = open(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "delegate").await;
    turn(&mut rx).await;

    let subagent = provider.seen.lock().unwrap()[1].clone();
    assert!(subagent.tools.iter().any(|t| t.name == "mcp__fixture__echo"));
    assert!(subagent.tools.iter().all(|t| t.name != "Task"));
    assert!(subagent.system.contains("<mcp_servers>"));
}
