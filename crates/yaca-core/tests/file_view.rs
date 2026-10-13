//! The file panel's view of a changed file, end to end (§6.4).
//!
//! Real files and a real write tool. The mock only plays the model's side:
//! which tool it calls and with what. There is nothing to decide about a hunk —
//! the view is read-only, and the one way back is a checkpoint.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::change::ViewSegment;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, SessionHandle};

use common::{ask, calls, is_turn_over, session, until, TempProject};

const PATH: &str = "a.txt";

/// Thirty numbered lines. The two edits below sit far enough apart to be two
/// hunks, not one.
fn original() -> String {
    (1..=30).map(|i| format!("строка {i}\n")).collect()
}

fn edited() -> String {
    original()
        .replace("строка 2\n", "правка 2\n")
        .replace("строка 25\n", "правка 25\n")
}

/// Reads the file, then rewrites it whole: one write call, two hunks.
fn read_then_write(content: &str) -> Arc<MockProvider> {
    Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("r1", "Read", json!({ "path": PATH }))])),
        Turn::Events(calls(&[(
            "w1",
            "Write",
            json!({ "path": PATH, "content": content }),
        )])),
        Turn::Events(script_text("Готово.")),
    ]))
}

async fn view_of(handle: &SessionHandle, path: &str) -> Option<yaca_core::change::FileView> {
    let mut rx = handle.subscribe();
    handle
        .send(Command::ReadFileView { path: path.into() })
        .await
        .unwrap();
    let answer = until(&mut rx, |e| matches!(e, AgentEvent::FileViewLoaded { .. })).await;
    answer.into_iter().find_map(|e| match e {
        AgentEvent::FileViewLoaded { view, .. } => view,
        _ => None,
    })
}

#[tokio::test]
async fn the_panel_gets_the_whole_file_with_its_unchanged_lines() {
    let dir = TempProject::new();
    dir.write(PATH, &original());
    let handle = session(&dir, read_then_write(&edited()), Mode::Auto);
    let mut rx = handle.subscribe();
    ask(&handle, "поправь два места").await;
    until(&mut rx, is_turn_over).await;

    let view = view_of(&handle, PATH)
        .await
        .expect("the session wrote the file, so it has a view");

    let hunks = view
        .segments
        .iter()
        .filter(|s| matches!(s, ViewSegment::Hunk { .. }))
        .count();
    assert_eq!(hunks, 2, "two far-apart edits are two hunks");

    // Every line of the file is in the view: unchanged lines included, which
    // is what separates it from the feed's three-line context.
    let lines: usize = view
        .segments
        .iter()
        .map(|s| match s {
            ViewSegment::Unchanged { lines } => lines.len(),
            ViewSegment::Hunk { hunk } => hunk.lines.iter().filter(|l| l.new_line.is_some()).count(),
        })
        .sum();
    assert_eq!(lines, 30);
    let fact = view.fact.expect("a changed file has a fact");
    assert_eq!((fact.added, fact.removed), (2, 2));
}

#[tokio::test]
async fn reading_the_view_writes_nothing() {
    let dir = TempProject::new();
    dir.write(PATH, &original());
    let handle = session(&dir, read_then_write(&edited()), Mode::Auto);
    let mut rx = handle.subscribe();
    ask(&handle, "поправь два места").await;
    until(&mut rx, is_turn_over).await;

    view_of(&handle, PATH).await.unwrap();

    assert_eq!(std::fs::read_to_string(dir.path().join(PATH)).unwrap(), edited());
}

#[tokio::test]
async fn a_file_the_session_never_wrote_is_shown_as_it_is() {
    let dir = TempProject::new();
    dir.write(PATH, &original());
    let handle = session(&dir, Arc::new(MockProvider::text("ничего")), Mode::Auto);

    let view = view_of(&handle, PATH)
        .await
        .expect("a file on disk has a view, changed or not");

    assert!(view.fact.is_none(), "nothing changed, so there is no fact");
    match view.segments.as_slice() {
        [ViewSegment::Unchanged { lines }] => assert_eq!(lines.len(), 30),
        other => panic!("one unchanged stretch expected, got {other:?}"),
    }
}

#[tokio::test]
async fn a_changed_file_is_found_by_another_spelling_of_its_path() {
    let dir = TempProject::new();
    dir.write(PATH, &original());
    let handle = session(&dir, read_then_write(&edited()), Mode::Auto);
    let mut rx = handle.subscribe();
    ask(&handle, "поправь два места").await;
    until(&mut rx, is_turn_over).await;

    // A `Read` row carries the path as the model wrote it.
    let view = view_of(&handle, "./a.txt").await.unwrap();

    assert!(view.fact.is_some(), "the change is shown, not the bare file");
}

#[tokio::test]
async fn a_file_that_is_not_there_has_no_view() {
    let dir = TempProject::new();
    let handle = session(&dir, Arc::new(MockProvider::text("ничего")), Mode::Auto);

    assert!(view_of(&handle, "missing.txt").await.is_none());
}

#[tokio::test]
async fn a_file_outside_the_workspace_has_no_view() {
    let dir = TempProject::new();
    let handle = session(&dir, Arc::new(MockProvider::text("ничего")), Mode::Auto);

    assert!(view_of(&handle, "../outside.txt").await.is_none());
}
