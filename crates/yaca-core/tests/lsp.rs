//! Language servers end to end: a real server process, a real session.
//!
//! Driven through the public surface only — `spawn`, `Command`, `AgentEvent` —
//! with `examples/lsp_fixture.rs` as the server on the other end of a pipe.
//! Its files end in `.zz`, an extension no real server (or built-in one found
//! on this machine's `PATH`) answers for.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::broadcast::Receiver;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{LspServerSettings, LspStatus, Mode, SessionConfig, SessionHandle};

use common::{ask, calls, fixture, is_turn_over, tool_result_of, until_within, TempProject};

const ROPE: Duration = Duration::from_secs(30);

fn open(dir: &TempProject, provider: Arc<MockProvider>, flag: Option<&str>) -> SessionHandle {
    let mut config = SessionConfig::new(dir.path());
    config.mode = Mode::Auto;
    config.journal = None;
    if let Some(flag) = flag {
        config.lsp_servers.insert(
            "fixture".into(),
            LspServerSettings {
                command: fixture("lsp_fixture").to_string_lossy().into_owned(),
                args: if flag.is_empty() { vec![] } else { vec![flag.to_string()] },
                extensions: vec!["zz".into()],
                ..Default::default()
            },
        );
    }
    yaca_core::spawn(config, provider).unwrap()
}

async fn turn(rx: &mut Receiver<AgentEvent>) -> Vec<AgentEvent> {
    until_within(rx, ROPE, is_turn_over).await
}

fn lsp(id: &str, input: Value) -> Turn {
    Turn::Events(calls(&[(id, "Lsp", input)]))
}

/// One question to the server, answered, with the text of the result.
async fn one(dir: &TempProject, flag: &str, input: Value) -> (String, bool) {
    let provider = Arc::new(MockProvider::new(vec![
        lsp("t1", input),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = open(dir, provider.clone(), Some(flag));
    let mut rx = handle.subscribe();
    ask(&handle, "look").await;
    turn(&mut rx).await;
    tool_result_of(&provider, "t1")
}

fn source() -> TempProject {
    let dir = TempProject::new();
    dir.write("a.zz", "struct Thing {\n}\nfn go() {}\n");
    dir
}

#[tokio::test]
async fn the_tool_is_offered_only_where_there_is_a_server_to_ask() {
    let dir = source();
    for (flag, expected) in [(None, false), (Some(""), true)] {
        let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("Hi."))]));
        let handle = open(&dir, provider.clone(), flag);
        let mut rx = handle.subscribe();
        ask(&handle, "hello").await;
        turn(&mut rx).await;
        let tools: Vec<String> = provider.seen.lock().unwrap()[0]
            .tools
            .iter()
            .map(|t| t.name.clone())
            .collect();
        assert_eq!(tools.contains(&"Lsp".to_string()), expected, "{tools:?}");
    }
}

#[tokio::test]
async fn a_definition_names_the_place_and_shows_the_line() {
    let dir = source();
    let (text, is_error) = one(
        &dir,
        "",
        json!({ "action": "definition", "path": "a.zz", "line": 3, "symbol": "go", "description": "Found go" }),
    )
    .await;
    assert!(!is_error, "{text}");
    // The fixture answers with line 0, character 3: `a.zz:1:4`, and the line.
    assert!(text.contains("a.zz:1:4\tstruct Thing {"), "{text}");
}

#[tokio::test]
async fn the_column_comes_from_the_symbol_not_from_the_model() {
    let dir = source();
    let (text, is_error) = one(
        &dir,
        "",
        json!({ "action": "hover", "path": "a.zz", "line": 3, "symbol": "go", "description": "Looked" }),
    )
    .await;
    assert!(!is_error, "{text}");
    // Line 3 is zero-based 2, and `go` in `fn go()` starts at character 3. The
    // server also asked the client for its configuration during the handshake
    // and was answered.
    assert_eq!(text, "at 2:3 configured=true");
}

#[tokio::test]
async fn references_list_every_place() {
    let dir = source();
    let (text, _) = one(
        &dir,
        "",
        json!({ "action": "references", "path": "a.zz", "line": 1, "symbol": "Thing", "description": "Found" }),
    )
    .await;
    assert!(text.contains("a.zz:1:4"), "{text}");
    assert!(text.contains("a.zz:3:5\tfn go() {}"), "{text}");
}

#[tokio::test]
async fn an_outline_and_a_name_search() {
    let dir = source();
    let (text, _) =
        one(&dir, "", json!({ "action": "symbols", "path": "a.zz", "description": "Overview" })).await;
    assert!(text.starts_with("struct Thing  :1\n  method go  :3"), "{text}");

    let (text, _) = one(
        &dir,
        "",
        json!({ "action": "workspace_symbols", "query": "needle", "description": "Search" }),
    )
    .await;
    assert!(text.contains("function needle (lib)"), "{text}");
}

#[tokio::test]
async fn diagnostics_are_asked_for_by_name() {
    let dir = TempProject::new();
    dir.write("a.zz", "fine\nlet x = BROKEN;\n");
    let (text, is_error) = one(
        &dir,
        "",
        json!({ "action": "diagnostics", "path": "a.zz", "description": "Checked" }),
    )
    .await;
    assert!(!is_error, "{text}");
    assert!(text.contains("error a.zz:2:9 this line is BROKEN [fixture E1]"), "{text}");

    let clean = source();
    let (text, _) = one(
        &clean,
        "",
        json!({ "action": "diagnostics", "path": "a.zz", "description": "Checked" }),
    )
    .await;
    assert!(text.contains("No errors or warnings in a.zz"), "{text}");
}

#[tokio::test]
async fn an_edit_reports_what_it_broke_in_the_same_result() {
    let dir = source();
    let provider = Arc::new(MockProvider::new(vec![
        // Starts the server: an edit alone does not.
        lsp("t1", json!({ "action": "symbols", "path": "a.zz", "description": "Overview" })),
        Turn::Events(calls(&[(
            "t2",
            "Write",
            json!({ "path": "b.zz", "content": "ok\nlet y = BROKEN;\n" }),
        )])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = open(&dir, provider.clone(), Some(""));
    let mut rx = handle.subscribe();
    ask(&handle, "edit").await;
    turn(&mut rx).await;

    let (text, is_error) = tool_result_of(&provider, "t2");
    assert!(!is_error, "{text}");
    assert!(text.contains("Diagnostics from fixture: 1 errors"), "{text}");
    assert!(text.contains("error b.zz:2:9 this line is BROKEN"), "{text}");
}

#[tokio::test]
async fn an_edit_alone_does_not_start_a_server() {
    let dir = source();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("t1", "Write", json!({ "path": "b.zz", "content": "BROKEN\n" }))])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = open(&dir, provider.clone(), Some(""));
    let mut rx = handle.subscribe();
    ask(&handle, "edit").await;
    turn(&mut rx).await;

    let (text, _) = tool_result_of(&provider, "t1");
    assert!(!text.contains("Diagnostics from"), "{text}");
    assert_eq!(handle.lsp()[0].status, LspStatus::Idle);
}

#[tokio::test]
async fn a_server_that_does_not_answer_is_said_so_not_taken_for_clean() {
    let dir = source();
    let provider = Arc::new(MockProvider::new(vec![
        lsp("t1", json!({ "action": "symbols", "path": "a.zz", "description": "Overview" })),
        Turn::Events(calls(&[("t2", "Write", json!({ "path": "b.zz", "content": "BROKEN\n" }))])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = open(&dir, provider.clone(), Some("--silent"));
    let mut rx = handle.subscribe();
    ask(&handle, "edit").await;
    turn(&mut rx).await;

    let (text, _) = tool_result_of(&provider, "t2");
    assert!(text.contains("did not arrive"), "{text}");
}

#[tokio::test]
async fn a_server_that_cannot_start_fails_with_its_own_words() {
    let dir = source();
    let provider = Arc::new(MockProvider::new(vec![
        lsp("t1", json!({ "action": "hover", "path": "a.zz", "line": 1, "description": "Looked" })),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = open(&dir, provider.clone(), Some("--crash"));
    let mut rx = handle.subscribe();
    ask(&handle, "look").await;
    let events = turn(&mut rx).await;

    let (text, is_error) = tool_result_of(&provider, "t1");
    assert!(is_error);
    assert!(text.contains("boom: no toolchain"), "{text}");

    let summary = &handle.lsp()[0];
    assert_eq!(summary.status, LspStatus::Failed);
    assert!(summary.reason.as_deref().unwrap().contains("boom"));
    // The interface was shown the attempt, and then the failure.
    let states: Vec<LspStatus> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::LspUpdated { servers } => Some(servers[0].status),
            _ => None,
        })
        .collect();
    assert!(states.contains(&LspStatus::Starting), "{states:?}");
    assert_eq!(states.last(), Some(&LspStatus::Failed));
}

#[tokio::test]
async fn a_server_that_dies_mid_session_is_failed_with_the_reason() {
    let dir = source();
    let (text, is_error) = one(
        &dir,
        "--crash-on-hover",
        json!({ "action": "hover", "path": "a.zz", "line": 1, "description": "Looked" }),
    )
    .await;
    assert!(is_error);
    assert!(text.contains("hover is not implemented"), "{text}");
}

#[tokio::test]
async fn a_file_no_server_answers_for_says_which_servers_there_are() {
    let dir = source();
    dir.write("a.txt", "plain\n");
    let (text, is_error) = one(
        &dir,
        "",
        json!({ "action": "symbols", "path": "a.txt", "description": "Overview" }),
    )
    .await;
    assert!(is_error);
    assert!(text.contains("fixture (zz)"), "{text}");
}

fn contents(dir: &TempProject, rel: &str) -> String {
    std::fs::read_to_string(dir.path().join(rel)).unwrap()
}

fn renamed() -> TempProject {
    let dir = TempProject::new();
    dir.write("a.zz", "struct Thing {\n}\nfn go() {}\n");
    dir.write("b.zz", "use Thing;\nfn other() {}\n");
    dir
}

/// One `Rename` call, answered: the result and the turn's events.
async fn rename(dir: &TempProject, input: Value) -> ((String, bool), Vec<AgentEvent>) {
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("t1", "Rename", input)])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = open(dir, provider.clone(), Some(""));
    let mut rx = handle.subscribe();
    ask(&handle, "rename").await;
    let events = turn(&mut rx).await;
    (tool_result_of(&provider, "t1"), events)
}

#[tokio::test]
async fn a_rename_changes_every_file_the_server_names() {
    let dir = renamed();
    let ((text, is_error), events) = rename(
        &dir,
        json!({ "path": "a.zz", "line": 1, "symbol": "Thing", "new_name": "Widget", "description": "Renamed" }),
    )
    .await;
    assert!(!is_error, "{text}");
    assert!(text.contains("Renamed `Thing` to `Widget` in 2 files"), "{text}");
    assert_eq!(contents(&dir, "a.zz"), "struct Widget {\n}\nfn go() {}\n");
    assert_eq!(contents(&dir, "b.zz"), "use Widget;\nfn other() {}\n");

    // Each file reaches the feed under the call's row, with its own diff.
    let changed: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::FileChanged { call_id, fact, .. } if call_id.as_str() == "t1" => {
                Some(fact.path.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(changed, vec!["a.zz".to_string(), "b.zz".to_string()]);
}

#[tokio::test]
async fn a_rename_off_the_name_is_refused_before_anything_changes() {
    let dir = renamed();
    let ((text, is_error), _) = rename(
        &dir,
        json!({ "path": "a.zz", "line": 3, "symbol": "go", "new_name": "run", "description": "Renamed" }),
    )
    .await;
    assert!(is_error, "{text}");
    assert!(text.contains("nothing to rename at this position"), "{text}");
    assert_eq!(contents(&dir, "a.zz"), "struct Thing {\n}\nfn go() {}\n");
}

#[tokio::test]
async fn a_rename_that_would_move_a_file_changes_nothing() {
    let dir = renamed();
    let ((text, is_error), _) = rename(
        &dir,
        json!({ "path": "a.zz", "line": 1, "symbol": "Thing", "new_name": "Moved", "description": "Renamed" }),
    )
    .await;
    assert!(is_error, "{text}");
    assert!(text.contains("Move"), "{text}");
    assert_eq!(contents(&dir, "a.zz"), "struct Thing {\n}\nfn go() {}\n");
    assert_eq!(contents(&dir, "b.zz"), "use Thing;\nfn other() {}\n");
}

#[tokio::test]
async fn a_server_that_only_answers_when_asked_is_asked() {
    let dir = TempProject::new();
    dir.write("a.zz", "fine\nlet x = BROKEN;\n");
    let (text, is_error) = one(
        &dir,
        "--pull",
        json!({ "action": "diagnostics", "path": "a.zz", "description": "Checked" }),
    )
    .await;
    assert!(!is_error, "{text}");
    assert!(text.contains("error a.zz:2:9 this line is BROKEN [fixture E1]"), "{text}");
}

#[tokio::test]
async fn a_server_can_be_restarted_from_the_panel() {
    let dir = source();
    let provider = Arc::new(MockProvider::new(vec![
        lsp("t1", json!({ "action": "symbols", "path": "a.zz", "description": "Overview" })),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = open(&dir, provider.clone(), Some(""));
    let mut rx = handle.subscribe();
    ask(&handle, "look").await;
    turn(&mut rx).await;

    handle
        .send(yaca_core::Command::RestartLspServer { server: "fixture".into() })
        .await
        .unwrap();
    // Down first, then up again: the panel sees both.
    let seen = until_within(&mut rx, ROPE, |e| {
        matches!(e, AgentEvent::LspUpdated { servers } if servers[0].status == LspStatus::Ready)
    })
    .await;
    assert!(seen.iter().any(|e| matches!(
        e,
        AgentEvent::LspUpdated { servers } if servers[0].status == LspStatus::Idle
    )), "{seen:?}");
}
