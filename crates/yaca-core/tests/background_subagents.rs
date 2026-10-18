//! `Task` with `background`: the agent goes on while the subagent works.
//!
//! The mock answers requests in the order they arrive, whoever makes them, so
//! each test has to hold the two conversations to one order. A permission
//! question does that: in manual mode a write waits for the human, and the
//! one who asked makes no request until the test answers.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::broadcast::Receiver;
use yaca_core::error::ProviderError;
use yaca_core::event::AgentEvent;
use yaca_core::message::ContentBlock;
use yaca_core::provider::mock::{script_text, script_tool_call, MockProvider, Turn};
use yaca_core::provider::ProviderRequest;
use yaca_core::{Command, Mode, PermissionAnswer, SessionHandle};

use common::{ask, calls, is_turn_over, session, until, TempProject};

/// The agent sends a subagent in the background and, in the same answer,
/// starts a write of its own that waits for the human.
fn send_and_write() -> Turn {
    Turn::Events(calls(&[
        (
            "t1",
            "Task",
            json!({ "prompt": "Create b.txt.", "background": true }),
        ),
        ("w0", "Write", json!({ "path": "a.txt", "content": "a\n" })),
    ]))
}

fn write(id: &str, path: &str) -> Turn {
    Turn::Events(script_tool_call(
        id,
        "Write",
        json!({ "path": path, "content": "b\n" }),
    ))
}

fn text(reply: &str) -> Turn {
    Turn::Events(script_text(reply))
}

fn requests(provider: &MockProvider) -> Vec<ProviderRequest> {
    provider.seen.lock().unwrap().clone()
}

/// Collects events until every one of `hits` has matched some event.
async fn until_all(
    rx: &mut Receiver<AgentEvent>,
    hits: &[&dyn Fn(&AgentEvent) -> bool],
) -> Vec<AgentEvent> {
    let mut seen = Vec::new();
    let all = |seen: &[AgentEvent]| hits.iter().all(|hit| seen.iter().any(hit));
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        while !all(&seen) {
            seen.push(rx.recv().await.unwrap());
        }
    })
    .await;
    assert!(outcome.is_ok(), "timed out; got {} events", seen.len());
    seen
}

/// Waits for the provider to have been asked `n` times.
async fn asked_times(provider: &MockProvider, n: usize) {
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        while provider.seen.lock().unwrap().len() < n {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(outcome.is_ok(), "the provider was not asked {n} times");
}

fn asks_about(id: &'static str) -> impl Fn(&AgentEvent) -> bool {
    move |event| {
        matches!(
            event,
            AgentEvent::PermissionRequested { request } if request.call_id.0 == id
        )
    }
}

fn started(id: &'static str) -> impl Fn(&AgentEvent) -> bool {
    move |event| matches!(event, AgentEvent::ToolCallStarted { call_id, .. } if call_id.0 == id)
}

fn finished(id: &'static str) -> impl Fn(&AgentEvent) -> bool {
    move |event| matches!(event, AgentEvent::ToolCallFinished { call_id, .. } if call_id.0 == id)
}

fn failed(id: &'static str) -> impl Fn(&AgentEvent) -> bool {
    move |event| matches!(event, AgentEvent::ToolCallFailed { call_id, .. } if call_id.0 == id)
}

/// Answers the open question about `id` with a yes.
async fn allow(handle: &SessionHandle, seen: &[AgentEvent], id: &str) {
    let request_id = seen
        .iter()
        .find_map(|event| match event {
            AgentEvent::PermissionRequested { request } if request.call_id.0 == id => {
                Some(request.id.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no question about {id}"));
    handle
        .send(Command::AnswerPermission {
            request_id,
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
}

fn position(seen: &[AgentEvent], hit: impl Fn(&AgentEvent) -> bool) -> usize {
    seen.iter().position(hit).expect("no such event")
}

/// The texts of the last message of a request, apart from its tool results.
fn said_last(request: &ProviderRequest) -> Vec<String> {
    let last = request.messages.last().expect("a request without messages");
    last.content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn result_of(request: &ProviderRequest, id: &str) -> (String, bool) {
    request
        .messages
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
        .unwrap_or_else(|| panic!("no result for {id}"))
}

#[tokio::test]
async fn the_agent_goes_on_and_the_turn_waits_for_the_report() {
    let dir = TempProject::new();
    dir.write("dates.rs", "fn parse_date() {}\n");

    let provider = Arc::new(MockProvider::new(vec![
        send_and_write(),
        // The subagent, while the agent's own write waits for the human.
        write("w1", "b.txt"),
        // The agent, while the subagent's write waits in its turn.
        Turn::Events(script_tool_call("r1", "Read", json!({ "path": "dates.rs" }))),
        text("waiting for the report"),
        // The subagent, once it was allowed to write.
        text("Created b.txt."),
        text("done"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "two files").await;
    let mut seen = until_all(&mut rx, &[&asks_about("w0"), &started("w1")]).await;
    allow(&handle, &seen, "w0").await;

    // The agent read a file and said all it had to say, and the subagent is
    // still at its question: the turn is not over.
    seen.extend(until_all(&mut rx, &[&asks_about("w1"), &finished("r1")]).await);
    asked_times(&provider, 4).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    while let Ok(event) = rx.try_recv() {
        seen.push(event);
    }
    assert!(!seen.iter().any(is_turn_over), "the turn ended without the report");
    assert!(!seen.iter().any(finished("t1")), "the row closed with the subagent at work");

    allow(&handle, &seen, "w1").await;
    seen.extend(until(&mut rx, is_turn_over).await);

    let sent = requests(&provider);
    assert_eq!(sent.len(), 6);
    // The call answered at once, and said that the report comes later.
    let (answer, is_error) = result_of(&sent[2], "t1");
    assert!(!is_error);
    assert!(answer.contains("background"), "{answer}");
    // The report woke the agent as a message of its own.
    let woken = said_last(&sent[5]);
    assert_eq!(woken.len(), 1);
    assert!(woken[0].contains("t1") && woken[0].contains("Created b.txt."), "{woken:?}");

    // The row of the subagent closes when it reports, after the agent's own
    // later rows, and ahead of the turn.
    let closed = position(&seen, finished("t1"));
    assert!(position(&seen, finished("r1")) < closed);
    assert!(closed < position(&seen, is_turn_over));
    let report: String = seen
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallProgress { call_id, chunk } if call_id.0 == "t1" => {
                Some(chunk.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(report, "Created b.txt.");
    assert!(dir.path().join("b.txt").exists());
}

#[tokio::test]
async fn a_report_that_is_in_joins_the_results_of_the_batch() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![
        send_and_write(),
        text("Nothing to create."),
        text("done"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "two files").await;
    let seen = until_all(&mut rx, &[&asks_about("w0"), &finished("t1")]).await;
    allow(&handle, &seen, "w0").await;
    until(&mut rx, is_turn_over).await;

    let sent = requests(&provider);
    assert_eq!(sent.len(), 3);
    // In the message that answers the calls, and after every result in it.
    let last = sent[2].messages.last().unwrap();
    let results = last
        .content
        .iter()
        .take_while(|block| matches!(block, ContentBlock::ToolResult { .. }))
        .count();
    assert_eq!(results, 2);
    let said = said_last(&sent[2]);
    assert!(
        said.iter().any(|text| text.contains("Nothing to create.")),
        "{said:?}"
    );
}

#[tokio::test]
async fn esc_stops_a_background_subagent_with_the_turn() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![send_and_write(), write("w1", "b.txt")]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "two files").await;
    until_all(&mut rx, &[&asks_about("w0"), &started("w1")]).await;
    handle.send(Command::Cancel).await.unwrap();
    let seen = until(&mut rx, is_turn_over).await;

    assert!(matches!(seen.last(), Some(AgentEvent::TurnCancelled { .. })));
    // Its row is closed as stopped, and before the turn's own last event.
    assert!(seen.iter().any(|event| matches!(
        event,
        AgentEvent::ToolCallDenied { call_id, stopped: true, .. } if call_id.0 == "t1"
    )));
    assert_eq!(requests(&provider).len(), 2);
    assert!(!dir.path().join("b.txt").exists());
}

#[tokio::test]
async fn a_background_subagent_that_fails_says_so() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![
        send_and_write(),
        Turn::Fail(ProviderError::Protocol("boom".into())),
        text("got it"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "two files").await;
    let seen = until_all(&mut rx, &[&asks_about("w0"), &failed("t1")]).await;
    allow(&handle, &seen, "w0").await;
    until(&mut rx, is_turn_over).await;

    let sent = requests(&provider);
    assert_eq!(sent.len(), 3);
    let said = said_last(&sent[2]);
    assert!(
        said.iter()
            .any(|text| text.contains("t1") && text.contains("failed") && text.contains("boom")),
        "{said:?}"
    );
}

#[tokio::test]
async fn the_human_wakes_an_agent_that_waits_for_a_report() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![
        send_and_write(),
        write("w1", "b.txt"),
        text("waiting for the report"),
        // Answering the human, with the subagent still at its question.
        text("the subagent is still working"),
        text("Created b.txt."),
        text("done"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "two files").await;
    let mut seen = until_all(&mut rx, &[&asks_about("w0"), &started("w1")]).await;
    allow(&handle, &seen, "w0").await;
    seen.extend(until_all(&mut rx, &[&asks_about("w1")]).await);
    asked_times(&provider, 3).await;

    // §7.4: said to the agent, so heard now and not when the subagent is done.
    ask(&handle, "how are you?").await;
    asked_times(&provider, 4).await;
    let said = said_last(&requests(&provider)[3]);
    assert_eq!(said, vec!["how are you?".to_string()]);

    allow(&handle, &seen, "w1").await;
    let rest = until(&mut rx, is_turn_over).await;
    assert!(matches!(rest.last(), Some(AgentEvent::TurnFinished { .. })));

    let sent = requests(&provider);
    assert_eq!(sent.len(), 6);
    assert!(said_last(&sent[5])[0].contains("Created b.txt."));
}
