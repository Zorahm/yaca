//! A message sent while the agent works (§7.4).
//!
//! It joins the conversation after the current group of actions, not after
//! the whole turn. A long turn is exactly when the human wants to steer, and
//! "stop, not that way" that waits for the turn to end arrives once the work
//! it was about is done.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::id::RequestId;
use yaca_core::message::{ContentBlock, Message};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, PermissionAnswer};

use common::{ask, calls, is_turn_over, session, until, TempProject};

const LATER: &str = "and add a test for it";

fn asked(events: &[AgentEvent]) -> RequestId {
    events
        .iter()
        .find_map(|e| match e {
            AgentEvent::PermissionRequested { request } => Some(request.id.clone()),
            _ => None,
        })
        .expect("no permission was requested")
}

fn position(events: &[AgentEvent], hit: impl Fn(&AgentEvent) -> bool) -> usize {
    events
        .iter()
        .position(hit)
        .expect("the event never came")
}

fn texts(message: &Message) -> Vec<&str> {
    message
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// A turn stopped on a permission question, with a message sent meanwhile.
async fn message_while_asked(
    provider: Arc<MockProvider>,
    dir: &TempProject,
) -> (yaca_core::SessionHandle, tokio::sync::broadcast::Receiver<AgentEvent>, Vec<AgentEvent>) {
    let handle = session(dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "create notes.txt").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::PermissionRequested { .. })).await;
    // Sent before the answer, so it is queued while the batch is still
    // running: the dispatcher takes commands in order.
    ask(&handle, LATER).await;
    (handle, rx, seen)
}

fn write_notes() -> Turn {
    Turn::Events(calls(&[(
        "w1",
        "Write",
        json!({ "path": "notes.txt", "content": "x\n" }),
    )]))
}

#[tokio::test]
async fn a_message_sent_mid_turn_joins_after_the_current_batch() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        write_notes(),
        Turn::Events(script_text("Created it; adding the test next.")),
        Turn::Events(script_text("Ok.")),
    ]));
    let (handle, mut rx, mut seen) = message_while_asked(provider.clone(), &dir).await;

    handle
        .send(Command::AnswerPermission {
            request_id: asked(&seen),
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
    seen.extend(until(&mut rx, is_turn_over).await);

    // One turn, and the human's words land between the batch and the model's
    // next step.
    let started = seen.iter().filter(|e| matches!(e, AgentEvent::TurnStarted { .. }));
    assert_eq!(started.count(), 1);
    let batch_done = position(&seen, |e| matches!(e, AgentEvent::ToolCallFinished { .. }));
    let heard = position(
        &seen,
        |e| matches!(e, AgentEvent::UserMessage { text, .. } if text == LATER),
    );
    let answered = position(&seen, |e| matches!(e, AgentEvent::TextDelta { .. }));
    assert!(batch_done < heard && heard < answered, "{batch_done} {heard} {answered}");

    // The model reads it right after the results of the batch it followed.
    {
        let requests = provider.seen.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let last = requests[1].messages.last().unwrap();
        assert!(matches!(last.content[0], ContentBlock::ToolResult { .. }));
        assert_eq!(texts(last), vec![LATER]);
    }

    // And it is not asked a second time as a turn of its own.
    ask(&handle, "thanks").await;
    until(&mut rx, is_turn_over).await;
    let requests = provider.seen.lock().unwrap();
    assert_eq!(requests.len(), 3);
    let mentions = requests[2]
        .messages
        .iter()
        .flat_map(texts)
        .filter(|t| t.contains(LATER))
        .count();
    assert_eq!(mentions, 1);
}

#[tokio::test]
async fn a_message_sent_before_a_cancel_gets_a_turn_of_its_own() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        write_notes(),
        Turn::Events(script_text("Adding the test.")),
    ]));
    let (handle, mut rx, _) = message_while_asked(provider.clone(), &dir).await;

    // Folded into a cancelled turn, it would sit in the history unanswered:
    // the model is not asked anything after a cancel.
    handle.send(Command::Cancel).await.unwrap();
    until(&mut rx, |e| matches!(e, AgentEvent::TurnCancelled { .. })).await;
    until(&mut rx, |e| matches!(e, AgentEvent::TurnFinished { .. })).await;

    let requests = provider.seen.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let last = requests[1].messages.last().unwrap();
    assert_eq!(texts(last), vec![LATER]);
}
