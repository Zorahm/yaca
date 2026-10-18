//! Several roots in one session, end to end (multi-root workspaces).
//!
//! Driven through the public surface only, as the interface drives it. The
//! model's paths are the ones the environment note shows: absolute, without a
//! verbatim prefix, forward slashes.

mod common;

use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, PermissionAnswer, PermissionKind, SessionConfig};

use common::{ask, calls, is_turn_over, session_with_roots, tool_result_of, until, TempProject};

/// How the model names a root: no verbatim prefix, forward slashes.
fn model_root(root: &Path) -> String {
    let text = root.display().to_string();
    text.strip_prefix(r"\\?\").unwrap_or(&text).replace('\\', "/")
}

/// How the model names a file under `root`.
fn model_path(root: &Path, rel: &str) -> String {
    format!("{}/{rel}", model_root(root))
}

fn permission_requests(events: &[AgentEvent]) -> Vec<yaca_core::PermissionRequest> {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::PermissionRequested { request } => Some(request.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn an_edit_in_the_second_root_asks_what_a_rollback_will_not_return() {
    let project = TempProject::new();
    project.write("a.txt", "primary\n");
    let other = TempProject::new();
    let note = other.write("notes.txt", "one\n");
    let target = model_path(other.path(), "notes.txt");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("r1", "Read", json!({ "path": target }))])),
        Turn::Events(calls(&[(
            "e1",
            "Edit",
            json!({ "path": target, "old_string": "one", "new_string": "two" }),
        )])),
        Turn::Events(script_text("Changed the note.")),
    ]));
    // Auto: an edit is not asked about there, so the question that appears is
    // the one about what a rollback does not cover.
    let handle = session_with_roots(&project, &[&other], provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "fix the note").await;
    let asking = until(&mut rx, |e| matches!(e, AgentEvent::PermissionRequested { .. })).await;
    let requests = permission_requests(&asking);
    assert_eq!(requests.len(), 1, "{requests:?}");
    let request = &requests[0];
    assert_eq!(request.kind, PermissionKind::EditFile);
    let lost = request.unsaved.as_ref().expect("the edit is not covered by rollback");
    assert_eq!(lost.roots, vec![model_root(other.path())]);
    assert_eq!(std::fs::read_to_string(&note).unwrap(), "one\n", "nothing happens before the answer");

    handle
        .send(Command::AnswerPermission {
            request_id: request.id.clone(),
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;

    assert_eq!(std::fs::read_to_string(&note).unwrap(), "two\n", "the edit lands");
}

#[tokio::test]
async fn an_edit_in_the_project_is_not_asked_about_in_auto_mode() {
    // The same mode and the same tool, in the primary root: nothing changes
    // there, and no unsaved question appears.
    let project = TempProject::new();
    let file = project.write("a.txt", "one\n");
    let other = TempProject::new();
    other.write("unused.txt", "x\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("r1", "Read", json!({ "path": "a.txt" }))])),
        Turn::Events(calls(&[(
            "e1",
            "Edit",
            json!({ "path": "a.txt", "old_string": "one", "new_string": "two" }),
        )])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = session_with_roots(&project, &[&other], provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "fix it").await;
    let events = until(&mut rx, is_turn_over).await;
    assert!(permission_requests(&events).is_empty(), "no question for the project");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "two\n");
}

#[tokio::test]
async fn a_write_outside_both_roots_is_refused_and_writes_nothing() {
    let project = TempProject::new();
    let other = TempProject::new();
    let outside = TempProject::new();
    let target = model_path(outside.path(), "stolen.txt");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("w1", "Write", json!({ "path": target, "content": "x\n" }))])),
        Turn::Events(script_text("Could not write there.")),
    ]));
    let handle = session_with_roots(&project, &[&other], provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "write it").await;
    let events = until(&mut rx, is_turn_over).await;

    let (text, is_error) = tool_result_of(&provider, "w1");
    assert!(is_error, "{text}");
    assert!(text.contains("вне проекта"), "{text}");
    assert!(permission_requests(&events).is_empty(), "a refused path is not a question");
    assert!(!outside.path().join("stolen.txt").exists());
}

#[tokio::test]
async fn a_resumed_session_must_come_back_with_the_same_roots() {
    let project = TempProject::new();
    let other = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![]));

    let mut first = SessionConfig::new(project.path());
    first.journal = Some(project.chats());
    first.additional_roots = vec![other.path().to_path_buf()];
    let id = yaca_core::spawn(first, provider.clone()).unwrap().id().clone();

    let resume = |roots: Vec<std::path::PathBuf>| {
        let mut cfg = SessionConfig::new(project.path());
        cfg.journal = Some(project.chats());
        cfg.resume = Some(id.clone());
        cfg.additional_roots = roots;
        yaca_core::spawn(cfg, provider.clone())
    };

    // Without the roots, or with a different set, the resume is refused.
    assert!(resume(vec![]).is_err(), "a resume that drops the root must fail");
    assert!(resume(vec![TempProject::new().path().to_path_buf()]).is_err());
    // The same set goes through.
    assert!(resume(vec![other.path().to_path_buf()]).is_ok());
}
