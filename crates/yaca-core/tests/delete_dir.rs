//! Deleting a whole directory (§5.5, §5.7, §8).
//!
//! The question names the directory and how many files it takes with it, and
//! two things are never deleted whatever the mode: the project itself, and a
//! directory holding a `.git` or `.yaca`. Snapshots keep neither (§5.8), so
//! "roll back" could not bring them back.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::message::ContentBlock;
use yaca_core::permission::FileCount;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, PermissionAnswer, PermissionKind};

use common::{ask, calls, is_turn_over, session, until, TempProject};

fn delete_dir(path: &str) -> Turn {
    Turn::Events(calls(&[("d1", "DeleteDir", json!({ "path": path }))]))
}

/// What the model was told about the call.
fn answer(provider: &MockProvider) -> (String, bool) {
    let seen = provider.seen.lock().unwrap();
    seen.last()
        .unwrap()
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .find_map(|block| match block {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => Some((content.clone(), *is_error)),
            _ => None,
        })
        .expect("the call was never answered")
}

#[tokio::test]
async fn manual_mode_asks_with_the_number_of_files_inside() {
    let dir = TempProject::new();
    dir.write("old/a.txt", "a\n");
    dir.write("old/sub/b.txt", "b\n");
    dir.write("old/sub/c.txt", "c\n");
    dir.write("keep.txt", "k\n");

    let provider = Arc::new(MockProvider::new(vec![
        delete_dir("old"),
        Turn::Events(script_text("Removed.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "remove the old directory").await;
    let asking = until(&mut rx, |e| matches!(e, AgentEvent::PermissionRequested { .. })).await;
    let request = asking
        .iter()
        .find_map(|e| match e {
            AgentEvent::PermissionRequested { request } => Some(request.clone()),
            _ => None,
        })
        .unwrap();

    // §5.7: the directory's name, its full path, and the files inside.
    assert_eq!(request.kind, PermissionKind::DeleteDirectory);
    assert_eq!(request.target, "old");
    assert_eq!(request.detail, "old");
    assert_eq!(request.files, Some(FileCount { files: 3, more: false }));
    assert!(dir.path().join("old").exists(), "nothing happens before the answer");

    handle
        .send(Command::AnswerPermission {
            request_id: request.id,
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
    let rest = until(&mut rx, is_turn_over).await;

    assert!(!dir.path().join("old").exists());
    assert!(dir.path().join("keep.txt").exists());
    // §5.5: the row counts files, not lines.
    let items = rest.iter().find_map(|e| match e {
        AgentEvent::ToolCallFinished { meta, .. } => meta.items,
        _ => None,
    });
    assert_eq!(items, Some(3));
}

#[tokio::test]
async fn a_directory_holding_a_repository_is_not_deleted() {
    let dir = TempProject::new();
    dir.write("vendor/lib/.git/HEAD", "ref: refs/heads/main\n");
    dir.write("vendor/lib/src.rs", "fn f() {}\n");

    let provider = Arc::new(MockProvider::new(vec![
        delete_dir("vendor"),
        Turn::Events(script_text("Understood.")),
    ]));
    // Auto deletes without asking, which is where the invariant has to hold.
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "clean up vendor").await;
    let seen = until(&mut rx, is_turn_over).await;

    assert!(seen.iter().any(|e| matches!(e, AgentEvent::ToolCallRejected { .. })));
    assert!(dir.path().join("vendor/lib/.git/HEAD").exists());
    assert!(dir.path().join("vendor/lib/src.rs").exists());
    let (text, is_error) = answer(&provider);
    assert!(is_error);
    assert!(text.contains(".git"), "{text}");
}

#[tokio::test]
async fn the_project_itself_is_not_deleted() {
    let dir = TempProject::new();
    dir.write("a.txt", "a\n");

    let provider = Arc::new(MockProvider::new(vec![
        delete_dir("."),
        Turn::Events(script_text("Understood.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "wipe everything").await;
    let seen = until(&mut rx, is_turn_over).await;

    assert!(seen.iter().any(|e| matches!(e, AgentEvent::ToolCallRejected { .. })));
    assert!(dir.path().join("a.txt").exists());
}
