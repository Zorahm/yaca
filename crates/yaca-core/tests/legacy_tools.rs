//! Backward compatibility with data on disk written with old tool names.
//!
//! Specifically:
//! 1. Standing permission rules in `.yaca/settings.json` naming old tools (e.g. `delete_file`, `bash`).
//! 2. Recorded session feed journals in `.yaca/sessions/<id>.jsonl` with old tool names.
//! 3. Recorded session history journals in `.yaca/sessions/<id>.messages.jsonl` with old tool names.

mod common;

use std::sync::Arc;

use yaca_core::event::AgentEvent;
use yaca_core::id::{CallId, RequestId, SessionId, TurnId};
use yaca_core::message::ContentBlock;
use yaca_core::permission::{PermissionKind, PermissionRequest, ScopeKey};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::store::SessionStore;
use yaca_core::{Mode, SessionConfig, Settings};

use common::{ask, calls, is_turn_over, until, TempProject};

#[test]
fn settings_with_old_tool_names_are_canonicalized_on_load() {
    let dir = TempProject::new();
    dir.write(
        ".yaca/settings.json",
        r#"{
            "permissions": {
                "rules": [
                    { "type": "tool", "name": "delete_file" },
                    { "type": "tool", "name": "bash" },
                    { "type": "tool", "name": "read_file" }
                ]
            }
        }"#,
    );

    let settings = Settings::load(dir.path()).expect("settings load cleanly");
    assert_eq!(
        settings.permissions.rules,
        vec![
            ScopeKey::Tool {
                name: "Delete".into()
            },
            ScopeKey::Tool {
                name: "Bash".into()
            },
            ScopeKey::Tool {
                name: "Read".into()
            },
        ]
    );
}

#[tokio::test]
async fn session_honors_old_tool_name_standing_permission_rule() {
    let dir = TempProject::new();
    dir.write("victim.txt", "to be deleted\n");
    dir.write(
        ".yaca/settings.json",
        r#"{
            "permissions": {
                "rules": [
                    { "type": "tool", "name": "delete_file" }
                ]
            }
        }"#,
    );

    // Manual mode would normally ask before deleting.
    // With delete_file in settings rules, Delete should be auto-approved.
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "Delete",
            serde_json::json!({ "path": "victim.txt" }),
        )])),
        Turn::Events(script_text("Deleted.")),
    ]));

    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Manual;
    cfg.journal = false;
    let handle = yaca_core::spawn(cfg, provider).unwrap();
    let mut rx = handle.subscribe();

    ask(&handle, "delete the file").await;
    let seen = until(&mut rx, is_turn_over).await;

    // Verify it did not request permission:
    let asked = seen
        .iter()
        .any(|e| matches!(e, AgentEvent::PermissionRequested { .. }));
    assert!(!asked, "should not ask permission since delete_file was in standing rules");

    let changed = seen
        .iter()
        .any(|e| matches!(e, AgentEvent::FileChanged { .. }));
    assert!(changed, "file should have been deleted");
}

#[test]
fn session_store_replays_events_with_canonical_tool_names() {
    let dir = TempProject::new();
    let id = SessionId::new();
    let turn = TurnId::new();

    // Write a .jsonl with old tool names in ToolCallStarted and PermissionRequested
    let jsonl_path = format!(".yaca/sessions/{id}.jsonl");
    let event1 = AgentEvent::ToolCallStarted {
        turn_id: turn.clone(),
        call_id: CallId::from("c1"),
        tool: "read_file".into(),
        description: Some("Read file".into()),
        target: Some("src/a.rs".into()),
        input: serde_json::json!({}),
        parent: None,
    };
    let event2 = AgentEvent::PermissionRequested {
        request: PermissionRequest {
            id: RequestId::from("req_1"),
            call_id: CallId::from("c2"),
            kind: PermissionKind::RunCommand,
            target: "cargo".into(),
            detail: "cargo test".into(),
            note: None,
            size_bytes: None,
            files: None,
            scopes: vec![ScopeKey::Tool {
                name: "bash".into(),
            }],
            position: 1,
            total: 1,
        },
    };

    dir.write(
        &jsonl_path,
        &format!(
            "{}\n{}\n",
            serde_json::to_string(&event1).unwrap(),
            serde_json::to_string(&event2).unwrap()
        ),
    );

    let store = SessionStore::open(dir.path());
    let events = store.events(&id);

    assert_eq!(events.len(), 2);
    match &events[0] {
        AgentEvent::ToolCallStarted { tool, .. } => {
            assert_eq!(tool, "Read", "tool name should be mapped to new name");
        }
        other => panic!("expected ToolCallStarted, got {:?}", other),
    }

    match &events[1] {
        AgentEvent::PermissionRequested { request } => {
            assert_eq!(
                request.scopes,
                vec![ScopeKey::Tool {
                    name: "Bash".into()
                }],
                "scope tool name should be mapped to new name"
            );
        }
        other => panic!("expected PermissionRequested, got {:?}", other),
    }
}

#[test]
fn session_store_replays_messages_with_canonical_tool_names() {
    let dir = TempProject::new();
    let id = SessionId::new();
    let turn = TurnId::new();

    // Write a .messages.jsonl containing old tool names
    let messages_path = format!(".yaca/sessions/{id}.messages.jsonl");
    let record = serde_json::json!({
        "kind": "message",
        "turn": turn,
        "message": {
            "role": "assistant",
            "content": [
                {
                    "kind": "tool_use",
                    "id": "tu_1",
                    "name": "edit_file",
                    "input": { "path": "test.txt", "old_string": "a", "new_string": "b" }
                }
            ]
        }
    });

    dir.write(&messages_path, &format!("{}\n", record));

    let store = SessionStore::open(dir.path());
    let history = store.history(&id);

    assert_eq!(history.len(), 1);
    match &history[0].content[0] {
        ContentBlock::ToolUse { name, .. } => {
            assert_eq!(name, "Edit", "tool use name should be canonicalized to Edit");
        }
        other => panic!("expected ToolUse, got {:?}", other),
    }
}
