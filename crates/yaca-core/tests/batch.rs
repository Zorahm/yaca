//! A batch of calls from one model response (§5.4), from start to results.
//!
//! The model calls tools in a batch so that they run together, and this is checked
//! through the event stream: all reading calls must start before the first one
//! finishes. Writing calls are the opposite, strictly one at a time: editing one
//! file at the same time gets past the freshness invariant of §8, where both
//! checks pass but only one of them survives.

mod common;

use std::sync::Arc;

use yaca_core::event::AgentEvent;
use yaca_core::message::ContentBlock;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;
use serde_json::json;

use common::{ask, calls, is_turn_over, session, until, TempProject};

/// Order of the turn's events: what started and what finished, by identifier.
///
/// A turn for one message can span several model responses, and `only` picks
/// out the batch the test is about: preparatory reads do not count.
fn timeline(seen: &[AgentEvent], only: &[&str]) -> Vec<String> {
    seen.iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallStarted { call_id, .. } => Some(("started", call_id)),
            AgentEvent::ToolCallFinished { call_id, .. } => Some(("finished", call_id)),
            _ => None,
        })
        .filter(|(_, call_id)| only.is_empty() || only.contains(&call_id.0.as_str()))
        .map(|(verb, call_id)| format!("{verb} {call_id}"))
        .collect()
}

/// Identifiers of the results, in the order they were sent to the model.
fn results(provider: &MockProvider) -> Vec<String> {
    let sent = provider.seen.lock().unwrap().last().unwrap().clone();
    sent.messages
        .last()
        .expect("a request without messages")
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn reading_calls_run_at_the_same_time() {
    let dir = TempProject::new();
    dir.write("a.rs", "first\n");
    dir.write("b.rs", "second\n");
    dir.write("c.rs", "third\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            ("t1", "Read", json!({ "path": "a.rs" })),
            ("t2", "Read", json!({ "path": "b.rs" })),
            ("t3", "Read", json!({ "path": "c.rs" })),
        ])),
        Turn::Events(script_text("read")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "read three files").await;
    let seen = until(&mut rx, is_turn_over).await;

    let line = timeline(&seen, &[]);
    let first_finish = line
        .iter()
        .position(|s| s.starts_with("finished"))
        .expect("at least one should have finished");
    assert_eq!(
        first_finish, 3,
        "all three started before the first one finished: {line:?}"
    );

    // The order of results is the one the model wrote the calls in, not the one
    // in which the disk managed to answer.
    assert_eq!(results(&provider), vec!["t1", "t2", "t3"]);
}

#[tokio::test]
async fn writing_calls_run_one_at_a_time() {
    let dir = TempProject::new();
    dir.write("a.rs", "first\n");
    dir.write("b.rs", "second\n");

    let provider = Arc::new(MockProvider::new(vec![
        // Read first: without this §8 would not allow editing.
        Turn::Events(calls(&[
            ("r1", "Read", json!({ "path": "a.rs" })),
            ("r2", "Read", json!({ "path": "b.rs" })),
        ])),
        Turn::Events(calls(&[
            (
                "w1",
                "Edit",
                json!({ "path": "a.rs", "old_string": "first", "new_string": "one" }),
            ),
            (
                "w2",
                "Edit",
                json!({ "path": "b.rs", "old_string": "second", "new_string": "two" }),
            ),
        ])),
        Turn::Events(script_text("fixed")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "fix both").await;
    let seen = until(&mut rx, is_turn_over).await;

    assert_eq!(
        timeline(&seen, &["w1", "w2"]),
        vec!["started w1", "finished w1", "started w2", "finished w2"],
        "edits do not mix even in different files"
    );
    assert_eq!(results(&provider), vec!["w1", "w2"]);
}

#[tokio::test]
async fn reads_do_not_overtake_a_writing_neighbour() {
    let dir = TempProject::new();
    dir.write("a.rs", "first\n");
    dir.write("b.rs", "second\n");

    // The order in the response is the order of the turn the model wrote. Letting
    // reads run ahead of a write would mean reading the file before it was
    // fixed, and handing the model yesterday's text.
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("r0", "Read", json!({ "path": "a.rs" }))])),
        Turn::Events(calls(&[
            (
                "w1",
                "Edit",
                json!({ "path": "a.rs", "old_string": "first", "new_string": "one" }),
            ),
            ("r2", "Read", json!({ "path": "a.rs" })),
            ("r3", "Read", json!({ "path": "b.rs" })),
        ])),
        Turn::Events(script_text("done")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "fix it and read it again").await;
    let seen = until(&mut rx, is_turn_over).await;

    let line = timeline(&seen, &["w1", "r2", "r3"]);
    assert_eq!(
        &line[..2],
        &["started w1", "finished w1"],
        "the edit completes before the reads: {line:?}"
    );
    // And the two reads after it run together again.
    assert_eq!(&line[2..4], &["started r2", "started r3"], "{line:?}");
}
