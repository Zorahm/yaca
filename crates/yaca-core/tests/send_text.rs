//! `SendText`: a file's text put in front of the human without the model
//! writing it out again.
//!
//! What is checked is the shape of the stream, because that is what the feed
//! is drawn from: the text arrives as `TextShown`, the call has no row of its
//! own, and the model is told that the text was shown rather than handed it.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;

use common::{ask, calls, is_turn_over, session, tool_result_of, until, TempProject};

/// Every `TextShown` of the turn as `(path, first_line, lines, total, text)`.
fn shown(seen: &[AgentEvent]) -> Vec<(String, u32, u32, u32, String)> {
    seen.iter()
        .filter_map(|event| match event {
            AgentEvent::TextShown {
                path,
                first_line,
                lines,
                total,
                text,
                ..
            } => Some((path.clone(), *first_line, *lines, *total, text.clone())),
            _ => None,
        })
        .collect()
}

async fn run(dir: &TempProject, input: serde_json::Value) -> (Vec<AgentEvent>, Arc<MockProvider>) {
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("s1", "SendText", input)])),
        Turn::Events(script_text("done")),
    ]));
    let handle = session(dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "show file").await;
    let seen = until(&mut rx, is_turn_over).await;
    (seen, provider)
}

#[tokio::test]
async fn a_file_reaches_the_human_and_not_the_model() {
    let dir = TempProject::new();
    dir.write("notes.md", "# Notes\n\nfor human eyes only\n");

    let (seen, provider) = run(&dir, json!({ "path": "notes.md" })).await;

    assert_eq!(
        shown(&seen),
        vec![(
            "notes.md".to_string(),
            1,
            3,
            3,
            "# Notes\n\nfor human eyes only".to_string()
        )]
    );

    let (result, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{result}");
    assert!(result.contains("notes.md"), "{result}");
    // The whole point: the text is not paid for a second time.
    assert!(!result.contains("for human eyes only"), "{result}");
}

#[tokio::test]
async fn the_call_has_no_row_of_its_own() {
    let dir = TempProject::new();
    dir.write("notes.md", "line\n");

    let (seen, _provider) = run(&dir, json!({ "path": "notes.md" })).await;

    // The block is what the human sees; a row beside it would say the same
    // thing twice.
    let rows: Vec<_> = seen.iter().filter_map(AgentEvent::call_id).collect();
    assert!(rows.is_empty(), "{rows:?}");
    assert_eq!(shown(&seen).len(), 1);
}

#[tokio::test]
async fn a_range_shows_only_its_lines_and_says_where_they_are() {
    let dir = TempProject::new();
    dir.write("a.rs", "one\ntwo\nthree\nfour\nfive\n");

    let (seen, provider) = run(
        &dir,
        json!({ "path": "a.rs", "start_line": 2, "end_line": 3 }),
    )
    .await;

    assert_eq!(
        shown(&seen),
        vec![("a.rs".to_string(), 2, 2, 5, "two\nthree".to_string())]
    );
    let (result, _) = tool_result_of(&provider, "s1");
    assert!(result.contains("2–3"), "{result}");
}

#[tokio::test]
async fn a_file_that_cannot_be_shown_is_an_error_and_shows_nothing() {
    let dir = TempProject::new();

    let (seen, provider) = run(&dir, json!({ "path": "no-such.md" })).await;

    assert!(shown(&seen).is_empty());
    let (result, is_error) = tool_result_of(&provider, "s1");
    assert!(is_error, "{result}");
}
