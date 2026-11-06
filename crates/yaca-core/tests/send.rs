//! The public agent API delivers rich metadata while the model gets a receipt.
mod common;

use common::{ask, calls, is_turn_over, session, tool_result_of, until, TempProject};
use serde_json::json;
use std::sync::Arc;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Mode, Profile, SessionConfig, Workspace};

#[tokio::test]
async fn send_delivers_a_file_without_returning_its_contents_to_the_model() {
    let dir = TempProject::new();
    dir.write("result.csv", "name,value\nprivate-content,42\n");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "s1",
            "Send",
            json!({ "path": "result.csv", "title": "Result", "caption": "Ready" }),
        )])),
        Turn::Events(script_text("done")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "deliver the file").await;
    let seen = until(&mut rx, is_turn_over).await;
    let files: Vec<_> = seen
        .iter()
        .filter_map(|e| match e {
            AgentEvent::FileSent { file } => Some(file),
            _ => None,
        })
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].title, "Result");
    assert_eq!(files[0].mime, "text/csv");
    let reader = Workspace::open(handle.state().delivery_root.unwrap()).unwrap();
    assert_eq!(
        yaca_core::sent::preview(&reader, files[0].id.as_str()).unwrap(),
        "name,value\nprivate-content,42\n"
    );
    let (result, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{result}");
    assert!(result.contains("Success:"), "{result}");
    assert!(!result.contains("private-content"), "{result}");
    let started = seen
        .iter()
        .position(|e| matches!(e, AgentEvent::ToolCallStarted { tool, .. } if tool == "Send"))
        .unwrap();
    let delivered = seen
        .iter()
        .position(|e| matches!(e, AgentEvent::FileSent { .. }))
        .unwrap();
    let finished = seen
        .iter()
        .position(|e| matches!(e, AgentEvent::ToolCallFinished { .. }))
        .unwrap();
    assert!(started < delivered && delivered < finished);
}

#[tokio::test]
async fn document_chat_deliveries_open_from_live_state_after_resume() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("s1", "Send", json!({ "path": "result.csv" }))])),
        Turn::Events(script_text("done")),
    ]));
    let mut config = SessionConfig::new(dir.path());
    config.mode = Mode::Manual;
    config.profile = Profile::Chat;
    config.journal = Some(dir.chats());
    let handle = yaca_core::spawn(config, provider).unwrap();
    let id = handle.id().clone();
    let private = dir.store().chat_workspace(&id);
    std::fs::write(private.join("result.csv"), "name,value\nchat,42\n").unwrap();
    let mut rx = handle.subscribe();
    ask(&handle, "deliver the file").await;
    let events = until(&mut rx, is_turn_over).await;
    let file = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::FileSent { file } => Some(file.clone()),
            _ => None,
        })
        .expect("Send emitted a delivery");
    let state = handle.state();
    let root = state.delivery_root.unwrap();
    assert_ne!(root, state.project_root);
    let reader = Workspace::open(&root).unwrap();
    assert_eq!(reader.root(), std::fs::canonicalize(&private).unwrap());
    assert_eq!(
        yaca_core::sent::preview(&reader, file.id.as_str()).unwrap(),
        "name,value\nchat,42\n"
    );
    drop(rx);
    drop(handle);
    std::fs::remove_file(private.join("result.csv")).unwrap();

    // Reopening an existing chat must find its saved delivery without the source.
    let mut config = SessionConfig::new(dir.path());
    config.journal = Some(dir.chats());
    config.resume_session(id);
    let handle = yaca_core::spawn(config, Arc::new(MockProvider::new(vec![]))).unwrap();
    let attached = handle.attach();
    let replay = handle.replay(&attached.replay);
    assert!(replay.iter().any(|event| matches!(event,
        AgentEvent::FileSent { file: saved } if saved == &file
    )));
    let state = replay
        .iter()
        .rev()
        .find_map(|event| match event {
            AgentEvent::StateChanged { state } => Some(state),
            _ => None,
        })
        .expect("replay supplies live state");
    assert_eq!(state.delivery_root.as_deref(), Some(root.as_str()));
    let reader = Workspace::open(state.delivery_root.as_ref().unwrap()).unwrap();
    assert_eq!(
        yaca_core::sent::preview(&reader, file.id.as_str()).unwrap(),
        "name,value\nchat,42\n"
    );
}
