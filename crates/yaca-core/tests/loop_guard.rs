//! A turn that goes round in circles: the step ceiling and the repeat check.
//!
//! Driven through the public surface only, like the rest of the end-to-end
//! tests. The model's side is read off the provider's own record of what it
//! was sent, so a note the model never saw fails here.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::{AgentEvent, StopReason};
use yaca_core::message::ContentBlock;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::question::{QuestionReply, QuestionRequest};
use yaca_core::{Command, Mode};

use common::{ask, calls, is_turn_over, session, until, TempProject};

fn same_read(id: &str) -> Turn {
    Turn::Events(calls(&[(id, "Read", json!({ "path": "a.rs" }))]))
}

fn asked(events: &[AgentEvent]) -> QuestionRequest {
    events
        .iter()
        .find_map(|e| match e {
            AgentEvent::QuestionAsked { request } => Some(request.clone()),
            _ => None,
        })
        .expect("no question was asked")
}

/// Every text block the model was sent in the request with this index.
fn texts_of_request(provider: &MockProvider, index: usize) -> Vec<String> {
    let seen = provider.seen.lock().unwrap();
    seen[index]
        .messages
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

async fn answer(handle: &yaca_core::SessionHandle, request: &QuestionRequest, reply: QuestionReply) {
    handle
        .send(Command::AnswerQuestion {
            request_id: request.id.clone(),
            answer: reply,
        })
        .await
        .unwrap();
}

fn picked(index: u32) -> QuestionReply {
    QuestionReply::Answered {
        selected: vec![index],
        other: None,
    }
}

// --- the repeat check ---------------------------------------------------------

#[tokio::test]
async fn the_third_identical_call_with_the_same_result_asks_the_human() {
    let dir = TempProject::new();
    dir.write("a.rs", "fn main() {}\n");
    let provider = Arc::new(MockProvider::new(vec![
        same_read("t1"),
        same_read("t2"),
        same_read("t3"),
        Turn::Events(script_text("Trying something else.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "look at a.rs").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    let request = asked(&seen);
    assert_eq!(request.options.len(), 2);
    assert!(request.question.contains("Read"), "{}", request.question);

    // Nothing more was asked of the model while the question is up.
    assert_eq!(provider.seen.lock().unwrap().len(), 3);

    answer(&handle, &request, picked(0)).await;
    let rest = until(&mut rx, is_turn_over).await;
    assert!(matches!(rest.last(), Some(AgentEvent::TurnFinished { .. })));

    // The model is told it was looping, in the same message as the results.
    let notes = texts_of_request(&provider, 3);
    assert!(
        notes.iter().any(|t| t.contains("[loop check]")),
        "the model was not warned: {notes:?}"
    );
}

#[tokio::test]
async fn stopping_on_a_repeat_ends_the_turn_without_asking_the_model_again() {
    let dir = TempProject::new();
    dir.write("a.rs", "fn main() {}\n");
    let provider = Arc::new(MockProvider::new(vec![
        same_read("t1"),
        same_read("t2"),
        same_read("t3"),
        Turn::Events(script_text("never asked")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "look at a.rs").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    answer(&handle, &asked(&seen), picked(1)).await;

    let rest = until(&mut rx, is_turn_over).await;
    assert!(
        matches!(rest.last(), Some(AgentEvent::TurnCancelled { .. })),
        "{:?}",
        rest.iter().map(AgentEvent::kind).collect::<Vec<_>>()
    );
    assert_eq!(provider.seen.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn words_typed_instead_of_an_option_reach_the_model() {
    let dir = TempProject::new();
    dir.write("a.rs", "fn main() {}\n");
    let provider = Arc::new(MockProvider::new(vec![
        same_read("t1"),
        same_read("t2"),
        same_read("t3"),
        Turn::Events(script_text("Reading the other file.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "look at a.rs").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    answer(
        &handle,
        &asked(&seen),
        QuestionReply::Answered {
            selected: Vec::new(),
            other: Some("look at b.rs instead".into()),
        },
    )
    .await;
    until(&mut rx, is_turn_over).await;

    let notes = texts_of_request(&provider, 3);
    assert!(
        notes.iter().any(|t| t.contains("look at b.rs instead")),
        "{notes:?}"
    );
}

#[tokio::test]
async fn a_different_call_in_between_breaks_the_run() {
    let dir = TempProject::new();
    dir.write("a.rs", "fn main() {}
");
    dir.write("b.rs", "fn other() {}
");
    let provider = Arc::new(MockProvider::new(vec![
        same_read("t1"),
        same_read("t2"),
        Turn::Events(calls(&[("t3", "Read", json!({ "path": "b.rs" }))])),
        same_read("t4"),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "look at both").await;
    let events = until(&mut rx, is_turn_over).await;

    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::QuestionAsked { .. })),
        "asked about a run that was broken"
    );
    assert!(matches!(events.last(), Some(AgentEvent::TurnFinished { .. })));
}

// --- the step ceiling ---------------------------------------------------------

#[tokio::test]
async fn a_turn_stops_at_the_step_ceiling_with_a_block_the_human_can_read() {
    let dir = TempProject::new();
    dir.write(".yaca/settings.json", r#"{ "maxSteps": 3 }"#);
    for name in ["a", "b", "c", "d", "e"] {
        dir.write(&format!("{name}.txt"), name);
    }
    // Different files each time: only the ceiling is under test here.
    let script: Vec<Turn> = ["a", "b", "c", "d", "e"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            Turn::Events(calls(&[(
                &format!("t{i}"),
                "Read",
                json!({ "path": format!("{name}.txt") }),
            )]))
        })
        .collect();
    let provider = Arc::new(MockProvider::new(script));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "read everything").await;
    let events = until(&mut rx, is_turn_over).await;

    let code = events.iter().find_map(|e| match e {
        AgentEvent::Error { code, retryable, .. } => Some((code.clone(), *retryable)),
        _ => None,
    });
    assert_eq!(code, Some(("step_limit".to_string(), false)));
    assert!(matches!(
        events.last(),
        Some(AgentEvent::TurnFinished {
            stop_reason: StopReason::Failed,
            ..
        })
    ));
    assert_eq!(provider.seen.lock().unwrap().len(), 3, "the model was asked past the ceiling");
}
