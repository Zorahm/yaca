//! The profile of a chat (§7.1): what the request carries in code and in
//! documents.
//!
//! Through the public surface only: what the provider was asked is what the
//! model saw, and that is the whole of what a profile changes.

mod common;

use std::sync::Arc;

use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, Profile, SessionConfig};

use common::{ask, is_turn_over, until, TempProject};

/// A session that answers in one turn, remembering what it was asked.
async fn seen_requests(
    dir: &TempProject,
    profile: Profile,
) -> Vec<yaca_core::provider::ProviderRequest> {
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("Done."))]));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Auto;
    cfg.profile = profile;
    cfg.journal = None;
    let handle = yaca_core::spawn(cfg, provider.clone()).unwrap();
    let mut rx = handle.subscribe();

    ask(&handle, "hello").await;
    until(&mut rx, is_turn_over).await;

    let seen = provider.seen.lock().unwrap().clone();
    seen
}

fn tool_names(request: &yaca_core::provider::ProviderRequest) -> Vec<&str> {
    request.tools.iter().map(|t| t.name.as_str()).collect()
}

#[tokio::test]
async fn a_chat_about_documents_is_not_offered_the_code_only_tools() {
    let dir = TempProject::new();
    let seen = seen_requests(&dir, Profile::Chat).await;
    let request = seen.last().expect("the model was asked");
    let names = tool_names(request);

    // The tools a document is made with are there.
    for tool in ["Read", "Write", "Edit", "Bash", "Skill", "ExtractImages"] {
        assert!(names.contains(&tool), "{tool} missing: {names:?}");
    }
    // The ones that only mean something in a repository are not.
    for tool in ["Lsp", "Rename", "NotebookEdit"] {
        assert!(!names.contains(&tool), "{tool} offered: {names:?}");
    }
    assert!(
        request.system.contains("ZIP of XML"),
        "the office rule is stated"
    );
    assert!(!request.system.contains("Write code that reads like"));
}

#[tokio::test]
async fn a_chat_about_code_keeps_the_coding_tools() {
    let dir = TempProject::new();
    let seen = seen_requests(&dir, Profile::Code).await;
    let request = seen.last().expect("the model was asked");
    let names = tool_names(request);

    // `NotebookEdit` is always in the set; `Lsp` needs a server and is added
    // per turn, so it is not asserted here.
    assert!(names.contains(&"NotebookEdit"));
    assert!(request.system.contains("Write code that reads like"));
    assert!(!request.system.contains("ZIP of XML"));
}

#[tokio::test]
async fn the_profile_is_fixed_but_switching_mode_moves_the_prefix_and_tools() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(script_text("First.")),
        Turn::Events(script_text("Second.")),
        Turn::Events(script_text("Third.")),
    ]));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Auto;
    cfg.journal = None;
    let handle = yaca_core::spawn(cfg, provider.clone()).unwrap();
    let mut rx = handle.subscribe();

    ask(&handle, "one").await;
    until(&mut rx, is_turn_over).await;

    handle
        .send(Command::SetProfile {
            profile: Profile::Chat,
        })
        .await
        .unwrap();
    until(
        &mut rx,
        |e| matches!(e, AgentEvent::Error { code, .. } if code == "profile_fixed"),
    )
    .await;

    ask(&handle, "two").await;
    until(&mut rx, is_turn_over).await;

    handle
        .send(Command::SetMode { mode: Mode::Plan })
        .await
        .unwrap();
    until(
        &mut rx,
        |e| matches!(e, AgentEvent::StateChanged { state } if state.mode == Mode::Plan),
    )
    .await;
    ask(&handle, "three").await;
    until(&mut rx, is_turn_over).await;

    let seen = provider.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 3);
    assert!(tool_names(&seen[0]).contains(&"Write"));
    assert_eq!(tool_names(&seen[0]), tool_names(&seen[1]));
    assert_eq!(seen[0].system, seen[1].system);
    assert!(!tool_names(&seen[2]).contains(&"Write"));
    assert_ne!(seen[1].system, seen[2].system);
}

#[tokio::test]
async fn a_code_only_tool_asked_for_in_a_chat_is_an_unknown_tool() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(common::tool_call(
            0,
            "t1",
            "NotebookEdit",
            serde_json::json!({ "path": "a.ipynb", "op": "insert", "source": "x" }),
        )),
        Turn::Events(script_text("Alright.")),
    ]));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Auto;
    cfg.profile = Profile::Chat;
    cfg.journal = None;
    let handle = yaca_core::spawn(cfg, provider).unwrap();
    let mut rx = handle.subscribe();

    ask(&handle, "edit the notebook").await;
    let rest = until(&mut rx, is_turn_over).await;

    // §7.1: a tool the profile leaves out is not "no permission" but "no such
    // tool" — the model must not be told it could ask.
    let refused = rest.iter().any(|e| {
        matches!(e, AgentEvent::ToolCallFailed { message, .. } if message.contains("unknown tool"))
    });
    assert!(refused, "{rest:?}");
    assert!(!rest
        .iter()
        .any(|e| matches!(e, AgentEvent::PermissionRequested { .. })));
}
