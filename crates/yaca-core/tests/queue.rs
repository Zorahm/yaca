//! A message sent while the agent works (§7.4).
//!
//! It joins the conversation after the current group of actions, not after
//! the whole turn. A long turn is exactly when the human wants to steer, and
//! "stop, not that way" that waits for the turn to end arrives once the work
//! it was about is done.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::{AgentEvent, QueuedMessage};
use yaca_core::id::RequestId;
use yaca_core::message::{ContentBlock, Message};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, PermissionAnswer};

use common::{ask, calls, is_turn_over, session, until, TempProject};

const LATER: &str = "and add a test for it";
const SECOND: &str = "and run it afterwards";
const SENTINEL: &str = "what happened to the notes?";

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

/// The queue as the last `queueUpdated` left it.
fn queue_of(events: &[AgentEvent]) -> Vec<QueuedMessage> {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            AgentEvent::QueueUpdated { queued } => Some(queued.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// How long the queue was, in each `queueUpdated` in order.
fn announced_lengths(events: &[AgentEvent]) -> Vec<usize> {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::QueueUpdated { queued } => Some(queued.len()),
            _ => None,
        })
        .collect()
}

fn said(events: &[AgentEvent], text: &str) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, AgentEvent::UserMessage { text: t, .. } if t == text))
        .count()
}

/// Every text any request the model was sent contained, in order.
fn sent_texts(provider: &MockProvider) -> Vec<String> {
    provider
        .seen
        .lock()
        .unwrap()
        .iter()
        .flat_map(|r| r.messages.iter())
        .flat_map(texts)
        .map(str::to_owned)
        .collect()
}

/// Waits for the queue to announce a list of `len` messages.
async fn until_queued(
    rx: &mut tokio::sync::broadcast::Receiver<AgentEvent>,
    len: usize,
) -> Vec<AgentEvent> {
    until(rx, move |e| {
        matches!(e, AgentEvent::QueueUpdated { queued } if queued.len() == len)
    })
    .await
}

#[tokio::test]
async fn a_queued_message_cancelled_mid_turn_never_reaches_the_model() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        write_notes(),
        Turn::Events(script_text("Created it.")),
    ]));
    let (handle, mut rx, mut seen) = message_while_asked(provider.clone(), &dir).await;
    seen.extend(until_queued(&mut rx, 1).await);
    let queued = queue_of(&seen);
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].text, LATER);

    handle
        .send(Command::CancelQueued {
            id: queued[0].id.clone(),
        })
        .await
        .unwrap();
    handle
        .send(Command::AnswerPermission {
            request_id: asked(&seen),
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
    seen.extend(until(&mut rx, is_turn_over).await);

    // The chip came up once and went away once, and nothing else was said
    // about the message: it was never taken in.
    assert_eq!(announced_lengths(&seen), vec![1, 0]);
    assert_eq!(said(&seen, LATER), 0);
    // The model was asked twice, and the text of the cancelled message is in
    // neither request.
    assert_eq!(provider.seen.lock().unwrap().len(), 2);
    assert!(!sent_texts(&provider).iter().any(|t| t.contains(LATER)));
}

#[tokio::test]
async fn a_cancel_after_the_message_was_taken_in_changes_nothing() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        write_notes(),
        Turn::Events(script_text("Adding the test.")),
    ]));
    let (handle, mut rx, mut seen) = message_while_asked(provider.clone(), &dir).await;
    seen.extend(until_queued(&mut rx, 1).await);
    let id = queue_of(&seen)[0].id.clone();

    handle
        .send(Command::AnswerPermission {
            request_id: asked(&seen),
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
    // Taken in after the batch, before the model's next step.
    seen.extend(
        until(&mut rx, |e| matches!(e, AgentEvent::UserMessage { text, .. } if text == LATER))
            .await,
    );

    // Too late: the message is in the conversation. The cancel is a no-op, and
    // the core says so by sending the list it holds, which is now empty. The
    // turn may well finish before that list is sent, so both are waited for,
    // in whichever order they come.
    handle.send(Command::CancelQueued { id }).await.unwrap();
    seen.extend(until(&mut rx, |e| matches!(e, AgentEvent::QueueUpdated { .. })).await);
    if !seen.iter().any(is_turn_over) {
        seen.extend(until(&mut rx, is_turn_over).await);
    }

    assert_eq!(announced_lengths(&seen), vec![1, 0, 0]);
    assert!(!seen.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
    let requests = provider.seen.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let mentions = requests[1]
        .messages
        .iter()
        .flat_map(texts)
        .filter(|t| t.contains(LATER))
        .count();
    assert_eq!(mentions, 1, "the message reaches the model exactly once");
}

#[tokio::test]
async fn a_message_sent_after_a_cancelled_one_still_merges_after_the_batch() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        write_notes(),
        Turn::Events(script_text("Created it.")),
    ]));
    let (handle, mut rx, mut seen) = message_while_asked(provider.clone(), &dir).await;
    seen.extend(until_queued(&mut rx, 1).await);
    let first = queue_of(&seen)[0].id.clone();

    ask(&handle, SECOND).await;
    seen.extend(until_queued(&mut rx, 2).await);
    handle
        .send(Command::CancelQueued { id: first })
        .await
        .unwrap();
    handle
        .send(Command::AnswerPermission {
            request_id: asked(&seen),
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
    seen.extend(until(&mut rx, is_turn_over).await);

    assert_eq!(announced_lengths(&seen), vec![1, 2, 1, 0]);
    assert_eq!(said(&seen, LATER), 0);
    let batch_done = position(&seen, |e| matches!(e, AgentEvent::ToolCallFinished { .. }));
    let heard = position(
        &seen,
        |e| matches!(e, AgentEvent::UserMessage { text, .. } if text == SECOND),
    );
    assert!(batch_done < heard, "{batch_done} {heard}");
    let requests = provider.seen.lock().unwrap();
    assert_eq!(texts(requests[1].messages.last().unwrap()), vec![SECOND]);
}

#[tokio::test]
async fn a_cancelled_message_never_starts_a_turn_of_its_own() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        write_notes(),
        Turn::Events(script_text("Sentinel reply.")),
    ]));
    let (handle, mut rx, mut seen) = message_while_asked(provider.clone(), &dir).await;
    seen.extend(until_queued(&mut rx, 1).await);
    let id = queue_of(&seen)[0].id.clone();

    handle
        .send(Command::CancelQueued { id })
        .await
        .unwrap();
    seen.extend(until_queued(&mut rx, 0).await);

    // Stop the turn with the message gone. Its job is still in the channel,
    // and the worker has to step over it. A message sent after it shows that:
    // had the cancelled one run, its words would come first. A stopped turn
    // ends with `TurnCancelled` alone.
    handle.send(Command::Cancel).await.unwrap();
    until(&mut rx, |e| matches!(e, AgentEvent::TurnCancelled { .. })).await;

    ask(&handle, SENTINEL).await;
    let after = until(&mut rx, is_turn_over).await;
    assert_eq!(said(&after, LATER), 0);
    assert_eq!(said(&after, SENTINEL), 1);
    assert!(!sent_texts(&provider).iter().any(|t| t.contains(LATER)));
}
