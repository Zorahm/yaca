//! A response with nothing in it for the human: reasoning alone, or not even
//! that. The turn must end with a block that says so, not close as if the
//! model had finished.

mod common;

use std::sync::Arc;

use yaca_core::event::{AgentEvent, StopReason};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::{BlockKind, ProviderEvent};
use yaca_core::Mode;

use common::{ask, is_turn_over, session, until, TempProject};

fn thinking_only(stop: StopReason) -> Vec<ProviderEvent> {
    vec![
        ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::Thinking,
        },
        ProviderEvent::ThinkingDelta {
            index: 0,
            text: "let me think about it".into(),
        },
        ProviderEvent::SignatureDelta {
            index: 0,
            signature: "sig".into(),
        },
        ProviderEvent::BlockStop { index: 0 },
        ProviderEvent::MessageDelta {
            stop_reason: Some(stop),
            usage: Default::default(),
        },
        ProviderEvent::MessageStop,
    ]
}

fn error_of(events: &[AgentEvent]) -> Option<(String, String, bool)> {
    events.iter().find_map(|e| match e {
        AgentEvent::Error {
            code,
            message,
            retryable,
        } => Some((code.clone(), message.clone(), *retryable)),
        _ => None,
    })
}

#[tokio::test]
async fn reasoning_with_no_answer_ends_the_turn_with_an_error_block() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(thinking_only(
        StopReason::EndTurn,
    ))]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "go on").await;
    let events = until(&mut rx, is_turn_over).await;

    let (code, message, retryable) = error_of(&events).expect("the turn closed without a word");
    assert_eq!(code, "empty_answer");
    assert!(message.contains("no answer"), "{message}");
    assert!(retryable);
    assert!(matches!(
        events.last(),
        Some(AgentEvent::TurnFinished {
            stop_reason: StopReason::Failed,
            ..
        })
    ));
}

#[tokio::test]
async fn reasoning_that_ate_the_output_budget_says_so() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(thinking_only(
        StopReason::MaxTokens,
    ))]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "go on").await;
    let events = until(&mut rx, is_turn_over).await;

    let (_, message, _) = error_of(&events).expect("the turn closed without a word");
    assert!(message.contains("output budget"), "{message}");
}

#[tokio::test]
async fn an_answer_after_the_reasoning_is_not_an_error() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("done"))]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "go on").await;
    let events = until(&mut rx, is_turn_over).await;

    assert_eq!(error_of(&events), None);
    assert!(matches!(
        events.last(),
        Some(AgentEvent::TurnFinished {
            stop_reason: StopReason::EndTurn,
            ..
        })
    ));
}