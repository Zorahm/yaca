//! Every `tool_use` in the history gets its `tool_result`, whatever the
//! response that carried it looked like.
//!
//! The provider rejects a conversation in which a call goes unanswered, and it
//! rejects it on every later request, not just the next one: the history is
//! journalled, so even a reopened session carries the hole. Three shapes of
//! response used to leave one behind — a call cut short by the output cap, a
//! call whose arguments are not JSON, and a call under a stop reason other
//! than `tool_use`, which local OpenAI-compatible servers send.

mod common;

use std::collections::HashSet;
use std::sync::Arc;

use yaca_core::event::{AgentEvent, StopReason};
use yaca_core::message::{ContentBlock, Message};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::{BlockKind, ProviderEvent};
use yaca_core::usage::TokenCounts;
use yaca_core::Mode;

use common::{ask, is_turn_over, session, until, TempProject};

/// One assistant message with one call whose arguments arrive as `raw`.
fn one_call(id: &str, name: &str, raw: &str, stop: StopReason) -> Vec<ProviderEvent> {
    vec![
        ProviderEvent::MessageStart {
            model: "mock".into(),
            usage: TokenCounts::default(),
        },
        ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::ToolUse {
                id: id.into(),
                name: name.into(),
            },
        },
        ProviderEvent::ToolInputDelta {
            index: 0,
            partial_json: raw.into(),
        },
        ProviderEvent::BlockStop { index: 0 },
        ProviderEvent::MessageDelta {
            stop_reason: Some(stop),
            usage: TokenCounts::default(),
        },
        ProviderEvent::MessageStop,
    ]
}

/// What the model was told about `id` in the last request.
fn answer(provider: &MockProvider, id: &str) -> (String, bool) {
    let seen = provider.seen.lock().unwrap();
    let last = seen.last().expect("no request was made");
    last.messages
        .iter()
        .flat_map(|m| &m.content)
        .find_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } if tool_use_id == id => Some((content.clone(), *is_error)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{id} was never answered"))
}

/// Calls in the history that have no result — what the provider refuses.
fn unanswered(messages: &[Message]) -> Vec<String> {
    let answered: HashSet<&str> = messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|block| match block {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
            _ => None,
        })
        .collect();
    messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, .. } if !answered.contains(id.as_str()) => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn finished_with(events: &[AgentEvent]) -> Option<StopReason> {
    events.iter().find_map(|e| match e {
        AgentEvent::TurnFinished { stop_reason, .. } => Some(*stop_reason),
        _ => None,
    })
}

fn failed(events: &[AgentEvent], id: &str) -> bool {
    events
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolCallFailed { call_id, .. } if call_id.0 == id))
}

#[tokio::test]
async fn a_call_cut_off_by_the_output_cap_is_answered_and_not_run() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        // The cap hit in the middle of the file's content.
        Turn::Events(one_call(
            "w1",
            "Write",
            r#"{"path": "big.txt", "content": "aaaa"#,
            StopReason::MaxTokens,
        )),
        Turn::Events(script_text("I will write it in parts.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "write the big file").await;
    let seen = until(&mut rx, is_turn_over).await;

    // Half a file on disk would be worse than none.
    assert!(!dir.path().join("big.txt").exists());
    assert!(failed(&seen, "w1"), "the feed shows the call as failed");

    // The turn goes on, so the model can do it in smaller pieces.
    assert_eq!(finished_with(&seen), Some(StopReason::EndTurn));
    let (text, is_error) = answer(&provider, "w1");
    assert!(is_error);
    assert!(text.contains("output limit"), "{text}");

    let requests = provider.seen.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(unanswered(&requests[1].messages), Vec::<String>::new());
}

#[tokio::test]
async fn arguments_that_are_not_json_are_named_as_such() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(one_call(
            "g1",
            "Grep",
            r#"{"pattern": "fn main"}}"#,
            StopReason::ToolUse,
        )),
        Turn::Events(script_text("Retrying.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "find main").await;
    let seen = until(&mut rx, is_turn_over).await;

    // Run with `{}` instead, the call would fail on a missing field the model
    // did send, and the model would be told to fix what was never wrong.
    assert!(failed(&seen, "g1"));
    let (text, is_error) = answer(&provider, "g1");
    assert!(is_error);
    assert!(text.contains("not valid JSON"), "{text}");
}

#[tokio::test]
async fn calls_run_whatever_the_stop_reason_says() {
    let dir = TempProject::new();
    dir.write("a.rs", "fn main() {}\n");
    let provider = Arc::new(MockProvider::new(vec![
        // Some OpenAI-compatible servers finish a message of tool calls with
        // "stop" rather than "tool_calls".
        Turn::Events(one_call(
            "r1",
            "Read",
            r#"{"path": "a.rs"}"#,
            StopReason::EndTurn,
        )),
        Turn::Events(script_text("It is empty.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "what is in a.rs").await;
    let seen = until(&mut rx, is_turn_over).await;

    assert!(seen
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolCallFinished { call_id, .. } if call_id.0 == "r1")));
    let (text, is_error) = answer(&provider, "r1");
    assert!(!is_error);
    assert!(text.contains("fn main"), "{text}");
}
