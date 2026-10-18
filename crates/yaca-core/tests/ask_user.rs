//! `AskUser` end to end: a question with options, the human's reply, and the
//! turn that carries on from it (§5.7-style blocking, but for a choice).
//!
//! Driven through the public surface only — `spawn`, `Command`, `AgentEvent` —
//! because that is the surface the feed has. The model's side is read off the
//! provider's own record of what it was sent, so a reply the model never saw
//! fails here, not in a screenshot.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::message::ContentBlock;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::ProviderEvent;
use yaca_core::question::{QuestionReply, QuestionRequest};
use yaca_core::{Command, Mode, PermissionAnswer, SessionConfig, SessionHandle};

use common::{ask, calls, is_turn_over, session, until, TempProject};

fn two_way_question(id: &str) -> Vec<ProviderEvent> {
    calls(&[(
        id,
        "AskUser",
        json!({
            "question": "Which way?",
            "options": [
                { "label": "Fast", "description": "skips the tests" },
                { "label": "Full" },
            ],
        }),
    )])
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

fn tool_results(messages: &[yaca_core::message::Message]) -> Vec<(String, bool)> {
    messages
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => Some((content.clone(), *is_error)),
            _ => None,
        })
        .collect()
}

fn journaled(dir: &TempProject, provider: Arc<MockProvider>) -> SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Manual;
    cfg.journal = Some(dir.chats());
    yaca_core::spawn(cfg, provider).unwrap()
}

// --- the answer goes back to the model ------------------------------------

#[tokio::test]
async fn an_answer_goes_back_to_the_model_in_the_same_turn() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(two_way_question("t1")),
        Turn::Events(script_text("Going with the full run.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "how should I run it?").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    let request = asked(&seen);
    assert_eq!(request.options.len(), 2);
    assert!(request.allow_other, "allow_other defaults to on");
    assert!(!request.multi_select, "multi_select defaults to off");

    handle
        .send(Command::AnswerQuestion {
            request_id: request.id.clone(),
            answer: QuestionReply::Answered {
                selected: vec![1],
                other: None,
            },
        })
        .await
        .unwrap();

    let rest = until(&mut rx, is_turn_over).await;
    let resolved = rest.iter().find_map(|e| match e {
        AgentEvent::QuestionResolved { reply, .. } => Some(reply.clone()),
        _ => None,
    });
    assert_eq!(
        resolved,
        Some(Some(QuestionReply::Answered {
            selected: vec![1],
            other: None,
        }))
    );
    assert!(matches!(rest.last(), Some(AgentEvent::TurnFinished { .. })));

    // The model made a second request, and that request carries the choice.
    let seen = provider.seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "the turn did not carry on after the answer");
    let results = tool_results(&seen[1].messages);
    assert!(
        results
            .iter()
            .any(|(text, is_error)| !is_error && text == "The user chose: \"Full\"."),
        "the model was not told the choice: {results:?}"
    );
}

#[tokio::test]
async fn an_own_answer_is_passed_on_in_the_users_words() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(two_way_question("t1")),
        Turn::Events(script_text("Understood.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "how?").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    let request = asked(&seen);

    handle
        .send(Command::AnswerQuestion {
            request_id: request.id,
            answer: QuestionReply::Answered {
                selected: Vec::new(),
                other: Some("only the parser".into()),
            },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;

    let seen = provider.seen.lock().unwrap();
    let results = tool_results(&seen[1].messages);
    assert!(
        results.iter().any(|(text, _)| text
            == "The user answered in their own words: \"only the parser\"."),
        "{results:?}"
    );
}

/// Asks one question with `multi_select` as given, answers it with a pick
/// and a written text, and returns what the model was told.
async fn answered_with_pick_and_text(multi_select: bool, selected: Vec<u32>) -> String {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "AskUser",
            json!({
                "question": "Which parts?",
                "multi_select": multi_select,
                "options": [{ "label": "Parser" }, { "label": "Lexer" }, { "label": "Docs" }],
            }),
        )])),
        Turn::Events(script_text("Noted.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "go").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    let request = asked(&seen);
    assert_eq!(request.multi_select, multi_select);

    handle
        .send(Command::AnswerQuestion {
            request_id: request.id,
            answer: QuestionReply::Answered {
                selected,
                other: Some("and the tests".into()),
            },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;

    let seen = provider.seen.lock().unwrap();
    let results = tool_results(&seen[1].messages);
    results
        .into_iter()
        .find(|(_, is_error)| !is_error)
        .map(|(text, _)| text)
        .expect("the model got no answer")
}

#[tokio::test]
async fn checkboxes_pass_the_ticked_options_and_the_text_together() {
    let text = answered_with_pick_and_text(true, vec![0, 2]).await;
    assert_eq!(
        text,
        "The user chose: \"Parser\", \"Docs\". They also wrote: \"and the tests\"."
    );
}

#[tokio::test]
async fn radio_passes_only_the_text_when_a_pick_and_text_are_both_given() {
    let text = answered_with_pick_and_text(false, vec![1]).await;
    assert_eq!(
        text,
        "The user answered in their own words: \"and the tests\"."
    );
}

// --- dismissal and cancellation ------------------------------------------

#[tokio::test]
async fn a_dismissed_question_tells_the_model_there_is_no_answer() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(two_way_question("t1")),
        Turn::Events(script_text("Stopping to ask again.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "how?").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    let request = asked(&seen);

    handle
        .send(Command::AnswerQuestion {
            request_id: request.id,
            answer: QuestionReply::Dismissed,
        })
        .await
        .unwrap();
    let rest = until(&mut rx, is_turn_over).await;
    assert!(rest.iter().any(|e| matches!(
        e,
        AgentEvent::QuestionResolved {
            reply: Some(QuestionReply::Dismissed),
            ..
        }
    )));

    let seen = provider.seen.lock().unwrap();
    let results = tool_results(&seen[1].messages);
    assert!(
        results
            .iter()
            .any(|(text, is_error)| !is_error && text.contains("dismissed the question")),
        "{results:?}"
    );
}

#[tokio::test]
async fn cancelling_the_turn_abandons_the_question() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(two_way_question("t1")),
        // Never reached: a cancelled turn asks the model nothing more.
        Turn::Events(script_text("should not run")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "how?").await;
    until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    handle.send(Command::Cancel).await.unwrap();

    let rest = until(&mut rx, is_turn_over).await;
    assert!(
        rest.iter().any(|e| matches!(
            e,
            AgentEvent::QuestionResolved { reply: None, .. }
        )),
        "the question stayed on screen after the turn was cancelled"
    );
    assert!(matches!(rest.last(), Some(AgentEvent::TurnCancelled { .. })));
    assert_eq!(provider.seen.lock().unwrap().len(), 1);
}

// --- refusals -------------------------------------------------------------

#[tokio::test]
async fn an_answer_the_question_cannot_take_is_refused_and_the_question_stays_open() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(two_way_question("t1")),
        Turn::Events(script_text("Fine.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "how?").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    let request = asked(&seen);

    // Option 9 does not exist: the answer is refused, and nothing resolves yet.
    handle
        .send(Command::AnswerQuestion {
            request_id: request.id.clone(),
            answer: QuestionReply::Answered {
                selected: vec![9],
                other: None,
            },
        })
        .await
        .unwrap();
    let refused = until(&mut rx, |e| matches!(e, AgentEvent::Error { .. })).await;
    assert!(matches!(
        refused.last(),
        Some(AgentEvent::Error { code, .. }) if code == "invalid_answer"
    ));
    assert!(
        !refused
            .iter()
            .any(|e| matches!(e, AgentEvent::QuestionResolved { .. })),
        "a refused answer closed the question"
    );

    handle
        .send(Command::AnswerQuestion {
            request_id: request.id,
            answer: QuestionReply::Answered {
                selected: vec![0],
                other: None,
            },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;
    assert_eq!(provider.seen.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn a_question_with_one_option_never_reaches_the_human() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "AskUser",
            json!({ "question": "Sure?", "options": [{ "label": "Yes" }] }),
        )])),
        Turn::Events(script_text("Then I will not ask.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "go").await;
    let rest = until(&mut rx, is_turn_over).await;

    assert!(
        !rest
            .iter()
            .any(|e| matches!(e, AgentEvent::QuestionAsked { .. })),
        "a question of one option was put to the human"
    );
    assert!(rest.iter().any(|e| matches!(e, AgentEvent::ToolCallFailed { .. })));

    let seen = provider.seen.lock().unwrap();
    let results = tool_results(&seen[1].messages);
    assert!(
        results
            .iter()
            .any(|(text, is_error)| *is_error && text.contains("between 2 and 6")),
        "the model was not told the bounds: {results:?}"
    );
}

#[tokio::test]
async fn seven_options_are_refused_as_well() {
    let dir = TempProject::new();
    let options: Vec<_> = (0..7).map(|i| json!({ "label": format!("o{i}") })).collect();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "AskUser",
            json!({ "question": "Which?", "options": options }),
        )])),
        Turn::Events(script_text("Narrowing it down myself.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "go").await;
    let rest = until(&mut rx, is_turn_over).await;
    assert!(
        !rest
            .iter()
            .any(|e| matches!(e, AgentEvent::QuestionAsked { .. })),
        "seven options were put to the human"
    );
}

// --- batches --------------------------------------------------------------

/// A question and a permission in one answer of the model. The question
/// holds the lane until it is answered, so the deletion waits for it and is
/// asked after: one at a time, and nothing deadlocks.
#[tokio::test]
async fn a_question_and_a_permission_in_one_batch_are_asked_one_after_another() {
    let dir = TempProject::new();
    let victim = dir.write("junk.rs", "dead code\n");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            (
                "q1",
                "AskUser",
                json!({ "question": "Drop it?", "options": [{ "label": "Yes" }, { "label": "No" }] }),
            ),
            ("d1", "Delete", json!({ "path": "junk.rs" })),
        ])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "clean up").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    let question = asked(&seen);
    assert!(
        victim.exists(),
        "the deletion ran before the question was answered"
    );

    handle
        .send(Command::AnswerQuestion {
            request_id: question.id,
            answer: QuestionReply::Answered {
                selected: vec![0],
                other: None,
            },
        })
        .await
        .unwrap();

    let next = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    let permission = next
        .iter()
        .find_map(|e| match e {
            AgentEvent::PermissionRequested { request } => Some(request.id.clone()),
            _ => None,
        })
        .unwrap();
    handle
        .send(Command::AnswerPermission {
            request_id: permission,
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();

    until(&mut rx, is_turn_over).await;
    assert!(!victim.exists(), "the permitted deletion did not happen");
}

// --- the journal ----------------------------------------------------------

#[tokio::test]
async fn a_recorded_question_and_its_answer_replay_as_they_happened() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(two_way_question("t1")),
        Turn::Events(script_text("Fast it is.")),
    ]));
    let handle = journaled(&dir, provider);
    let mut rx = handle.subscribe();

    ask(&handle, "how?").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::QuestionAsked { .. })).await;
    let request = asked(&seen);
    handle
        .send(Command::AnswerQuestion {
            request_id: request.id.clone(),
            answer: QuestionReply::Answered {
                selected: vec![0],
                other: None,
            },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;

    let store = dir.store();
    let session_id = handle.id().clone();
    let journal = store.events(&session_id);
    let asks = journal
        .iter()
        .filter(|e| matches!(e, AgentEvent::QuestionAsked { .. }))
        .count();
    assert_eq!(asks, 1, "the question went into the journal once");

    // A reopened feed draws the answered question, and nothing is left open.
    let feed = store.feed(&session_id);
    let settled: Vec<_> = feed
        .iter()
        .filter_map(|e| match e {
            AgentEvent::QuestionResolved { reply, .. } => Some(reply.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        settled,
        vec![Some(QuestionReply::Answered {
            selected: vec![0],
            other: None,
        })]
    );
}
