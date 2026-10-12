//! The `@` file search, end to end (§7.2): a real tree, the command, and the
//! answer the interface gets back.
//!
//! The ignore rules are the same ones `glob` and `grep` follow, and a file the
//! session has read ranks above one it has not, among equal matches.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, SessionHandle};

use common::{ask, calls, is_turn_over, session, until, TempProject};

/// Sends the search and returns the paths the answer carries. Fails if the
/// answer is for another query.
async fn search(handle: &SessionHandle, query: &str) -> Vec<String> {
    let mut rx = handle.subscribe();
    handle
        .send(Command::SearchFiles {
            query: query.into(),
        })
        .await
        .unwrap();
    let events = until(&mut rx, |e| matches!(e, AgentEvent::FilesFound { .. })).await;
    match events.last() {
        Some(AgentEvent::FilesFound { query: echoed, paths }) => {
            assert_eq!(echoed, query, "the answer names the query it answers");
            paths.clone()
        }
        other => panic!("not an answer: {other:?}"),
    }
}

fn project() -> TempProject {
    let dir = TempProject::new();
    dir.write(".gitignore", "target/\n");
    dir.write("src/app.rs", "fn main() {}\n");
    dir.write("src/main.rs", "fn main() {}\n");
    dir.write("README.md", "# readme\n");
    dir.write("target/app.rs", "built\n");
    dir
}

#[tokio::test]
async fn the_answer_holds_the_project_files_that_match_the_query() {
    let dir = project();
    let handle = session(&dir, Arc::new(MockProvider::text("unused")), Mode::Manual);

    // Ignored and hidden files are not offered, the same as to `glob`.
    assert_eq!(search(&handle, "app").await, vec!["src/app.rs"]);
    assert_eq!(search(&handle, "APP.RS").await, vec!["src/app.rs"]);
    assert!(search(&handle, "zzz").await.is_empty());
}

#[tokio::test]
async fn an_empty_query_offers_the_project_files_and_no_more_than_the_limit() {
    let dir = project();
    for i in 0..30 {
        dir.write(&format!("bulk/file{i:02}.txt"), "x\n");
    }
    let handle = session(&dir, Arc::new(MockProvider::text("unused")), Mode::Manual);

    let all = search(&handle, "").await;
    assert_eq!(all.len(), yaca_core::mention::LIMIT);
    assert!(all.iter().all(|p| !p.starts_with("target/") && !p.starts_with('.')));
}

#[tokio::test]
async fn a_file_the_session_has_read_comes_first_among_equal_matches() {
    let dir = TempProject::new();
    dir.write("a/one.rs", "1\n");
    dir.write("b/one.rs", "2\n");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("r1", "Read", json!({ "path": "b/one.rs" }))])),
        Turn::Events(script_text("Read it.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();
    ask(&handle, "read it").await;
    until(&mut rx, is_turn_over).await;

    let found = search(&handle, "one").await;
    assert_eq!(found, vec!["b/one.rs", "a/one.rs"]);
}
