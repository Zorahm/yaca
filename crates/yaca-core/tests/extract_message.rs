//! `ExtractMessage`: a message of the conversation as a file, so that changing
//! a sentence of it is an edit and not the whole text typed again.
//!
//! The road is three tools long — extract, `Edit`, `SendText` — and it is
//! walked here end to end, because each step is only worth having if the next
//! one accepts what it left.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Mode, SessionConfig, SessionHandle};

use common::{ask, calls, is_turn_over, tool_result_of, until, TempProject};

/// The session's id is part of the draft's path, and the mock's script is
/// written before the session exists, so the id is chosen here.
const SESSION: &str = "drafts-test";

fn draft(number: usize) -> String {
    format!(".yaca/drafts/{SESSION}/{number}.md")
}

fn recorded(dir: &TempProject, provider: Arc<MockProvider>) -> SessionHandle {
    let mut config = SessionConfig::new(dir.path());
    config.mode = Mode::Auto;
    config.journal = Some(dir.chats());
    config.resume = Some(SessionId::from(SESSION));
    yaca_core::spawn(config, provider).unwrap()
}

async fn say(handle: &SessionHandle, text: &str) -> Vec<AgentEvent> {
    let mut rx = handle.subscribe();
    ask(handle, text).await;
    until(&mut rx, is_turn_over).await
}

fn step(id: &str, name: &str, input: serde_json::Value) -> Turn {
    Turn::Events(calls(&[(id, name, input)]))
}

#[tokio::test]
async fn the_humans_text_is_changed_in_place_and_shown_back() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        step("x1", "ExtractMessage", json!({})),
        step(
            "e1",
            "Edit",
            json!({ "path": draft(1), "old_string": "red", "new_string": "blue" }),
        ),
        step("s1", "SendText", json!({ "path": draft(1) })),
        Turn::Events(script_text("Fixed.")),
    ]));
    let handle = recorded(&dir, provider.clone());

    let seen = say(&handle, "House is red.\nFence is high.").await;

    // The file holds the message as it was typed, and the model is told where.
    let requests = provider.seen.lock().unwrap().clone();
    let extracted = serde_json::to_string(&requests[1].messages.last().unwrap().content).unwrap();
    assert!(extracted.contains(&draft(1)), "{extracted}");

    // The edit went through without a Read: the model had the text in front
    // of it as a message, which is the same thing.
    let failed: Vec<_> = seen
        .iter()
        .filter(|e| {
            matches!(
                e,
                AgentEvent::ToolCallFailed { .. } | AgentEvent::ToolCallRejected { .. }
            )
        })
        .collect();
    assert!(failed.is_empty(), "{failed:?}");

    let shown: Vec<&str> = seen
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextShown { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(shown, vec!["House is blue.\nFence is high."]);

    let (result, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{result}");
}

#[tokio::test]
async fn the_draft_is_the_message_and_nothing_else() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        step("x1", "ExtractMessage", json!({})),
        Turn::Events(script_text("ok")),
    ]));
    let handle = recorded(&dir, provider.clone());
    let text = "  Indent at the start.\n\nAnd empty line above.";

    say(&handle, text).await;

    let on_disk = std::fs::read_to_string(dir.path().join(draft(1))).unwrap();
    assert_eq!(on_disk, text);
}

#[tokio::test]
async fn a_number_picks_any_earlier_message_of_either_side() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(script_text("Letter draft: dear colleague.")),
        step("x1", "ExtractMessage", json!({ "number": 2 })),
        Turn::Events(script_text("ok")),
    ]));
    let handle = recorded(&dir, provider.clone());

    say(&handle, "write a letter").await;
    say(&handle, "extract your draft to a file").await;

    let on_disk = std::fs::read_to_string(dir.path().join(draft(2))).unwrap();
    assert_eq!(on_disk, "Letter draft: dear colleague.");
    let (result, is_error) = tool_result_of(&provider, "x1");
    assert!(!is_error, "{result}");
    assert!(result.contains(&draft(2)), "{result}");
}

#[tokio::test]
async fn a_message_that_does_not_exist_writes_nothing() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        step("x1", "ExtractMessage", json!({ "number": 40 })),
        Turn::Events(script_text("ok")),
    ]));
    let handle = recorded(&dir, provider.clone());

    say(&handle, "hello").await;

    let (result, is_error) = tool_result_of(&provider, "x1");
    assert!(is_error, "{result}");
    assert!(!dir.path().join(".yaca/drafts").exists());
}

#[tokio::test]
async fn extracting_again_keeps_the_edits_already_made() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        step("x1", "ExtractMessage", json!({})),
        step(
            "e1",
            "Edit",
            json!({ "path": draft(1), "old_string": "red", "new_string": "blue" }),
        ),
        step("x2", "ExtractMessage", json!({ "number": 1 })),
        Turn::Events(script_text("ok")),
    ]));
    let handle = recorded(&dir, provider.clone());

    say(&handle, "House is red.").await;

    let on_disk = std::fs::read_to_string(dir.path().join(draft(1))).unwrap();
    assert_eq!(on_disk, "House is blue.");
    let (result, is_error) = tool_result_of(&provider, "x2");
    assert!(!is_error, "{result}");
    assert!(result.contains(&draft(1)), "{result}");
}
