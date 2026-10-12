//! Permissions end to end (§5.7, §8).
//!
//! Driven through the public surface only — `spawn`, `Command`, `AgentEvent` —
//! because that is exactly the surface the interface has. If a question can be
//! answered from here, it can be answered from the feed.

mod common;

use std::sync::Arc;

use yaca_core::event::AgentEvent;
use yaca_core::id::RequestId;
use yaca_core::message::ContentBlock;
use yaca_core::permission::Invariant;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::ProviderEvent;
use yaca_core::{Command, Mode, PermissionAnswer, PermissionKind, ScopeKey};

use common::{ask, calls, is_turn_over, session, until, TempProject};

fn request(events: &[AgentEvent]) -> (RequestId, PermissionKind, u32, u32) {
    events
        .iter()
        .find_map(|e| match e {
            AgentEvent::PermissionRequested { request } => Some((
                request.id.clone(),
                request.kind,
                request.position,
                request.total,
            )),
            _ => None,
        })
        .expect("no permission was requested")
}

fn delete(id: &str, path: &str) -> Vec<ProviderEvent> {
    calls(&[(id, "Delete", serde_json::json!({ "path": path }))])
}

// --- tests ------------------------------------------------------------------

#[tokio::test]
async fn manual_mode_asks_before_deleting() {
    let dir = TempProject::new();
    let victim = dir.write("junk.rs", "dead code\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "junk.rs")),
        Turn::Events(script_text("Removed.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "drop the dead file").await;
    let asking = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;

    let (id, kind, position, total) = request(&asking);
    assert_eq!(kind, PermissionKind::DeleteFile);
    assert_eq!((position, total), (1, 1));

    // Nothing has happened yet: the question exists to be answered first.
    assert!(victim.exists());

    handle
        .send(Command::AnswerPermission {
            request_id: id,
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();

    let rest = until(&mut rx, is_turn_over).await;
    let names: Vec<_> = rest.iter().map(AgentEvent::kind).collect();
    assert!(names.contains(&"permissionResolved"), "{names:?}");
    assert!(names.contains(&"fileChanged"), "{names:?}");

    // One answer is the whole confirmation: the file is gone (§16.37).
    assert!(!victim.exists());
}

#[tokio::test]
async fn denial_travels_to_the_agent_with_its_reason() {
    let dir = TempProject::new();
    let victim = dir.write("node_modules/x.js", "vendored\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "node_modules/x.js")),
        Turn::Events(script_text("Understood, rebuilding instead.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "clean up").await;
    let asking = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    let (id, ..) = request(&asking);

    handle
        .send(Command::AnswerPermission {
            request_id: id,
            answer: PermissionAnswer::Deny {
                reason: Some("не удаляй node_modules, просто пересобери".into()),
            },
        })
        .await
        .unwrap();

    let rest = until(&mut rx, is_turn_over).await;
    assert!(
        rest.iter().any(|e| matches!(
            e,
            AgentEvent::ToolCallDenied {
                reason: Some(_),
                ..
            }
        )),
        "the feed needs its own line for a human refusal (§5.5)"
    );
    assert!(victim.exists());

    // Without the note the agent just tries again (§5.7), so it has to reach
    // the next request.
    let seen = provider.seen.lock().unwrap();
    let last = format!("{:?}", seen.last().unwrap().messages);
    assert!(
        last.contains("пересобери"),
        "reason never reached the model"
    );
}

#[tokio::test]
async fn always_allow_covers_the_rest_of_the_session() {
    let dir = TempProject::new();
    dir.write("src/a.rs", "one\n");
    dir.write("src/b.rs", "two\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "src/a.rs")),
        Turn::Events(delete("t2", "src/b.rs")),
        Turn::Events(script_text("Both gone.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "delete both").await;
    let asking = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    let (id, ..) = request(&asking);

    handle
        .send(Command::AnswerPermission {
            request_id: id,
            answer: PermissionAnswer::AllowAlways {
                scope: ScopeKey::Directory { path: "src".into() },
            },
        })
        .await
        .unwrap();

    let rest = until(&mut rx, is_turn_over).await;
    let asked_again = rest
        .iter()
        .filter(|e| matches!(e, AgentEvent::PermissionRequested { .. }))
        .count();
    assert_eq!(
        asked_again, 0,
        "the granted rule should have covered src/b.rs"
    );

    // Both deletions land: the one that was asked about, and the one the
    // granted rule waved through.
    let changed = rest
        .iter()
        .filter(|e| matches!(e, AgentEvent::FileChanged { .. }))
        .count();
    assert_eq!(changed, 2, "both deletions still have to happen");
}

#[tokio::test]
async fn the_counter_knows_how_many_are_queued() {
    let dir = TempProject::new();
    dir.write("a.rs", "one\n");
    dir.write("b.rs", "two\n");

    // Both deletions arrive in a single assistant message.
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            ("t1", "Delete", serde_json::json!({ "path": "a.rs" })),
            ("t2", "Delete", serde_json::json!({ "path": "b.rs" })),
        ])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "delete both").await;

    let first = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    let (first_id, _, position, total) = request(&first);
    assert_eq!((position, total), (1, 2), "§5.7 shows `1 of 2`");

    handle
        .send(Command::AnswerPermission {
            request_id: first_id,
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();

    let second = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    let (second_id, _, position, total) = request(&second);
    assert_eq!((position, total), (2, 2));

    handle
        .send(Command::AnswerPermission {
            request_id: second_id,
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;
}

#[tokio::test]
async fn auto_mode_does_not_ask_about_deletion() {
    let dir = TempProject::new();
    dir.write("junk.rs", "dead\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "junk.rs")),
        Turn::Events(script_text("Gone.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "delete it").await;
    let events = until(&mut rx, is_turn_over).await;

    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::PermissionRequested { .. })),
        "§8: deletion is allowed outright in Auto"
    );
}

#[tokio::test]
async fn bypass_skips_the_question_entirely() {
    let dir = TempProject::new();
    dir.write("junk.rs", "dead\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "junk.rs")),
        Turn::Events(script_text("Gone.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    handle.send(Command::SetBypass { on: true }).await.unwrap();
    until(
        &mut rx,
        |e| matches!(e, AgentEvent::StateChanged { state } if state.bypass),
    )
    .await;

    ask(&handle, "delete it").await;
    let events = until(&mut rx, is_turn_over).await;

    assert!(!events
        .iter()
        .any(|e| matches!(e, AgentEvent::PermissionRequested { .. })));
}

#[tokio::test]
async fn cancelling_the_turn_closes_the_open_question() {
    let dir = TempProject::new();
    dir.write("junk.rs", "dead\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "junk.rs")),
        Turn::Events(script_text("unreachable")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "delete it").await;
    until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;

    // There is no timeout on a permission (§5.7); `Esc` is the only way out.
    handle.send(Command::Cancel).await.unwrap();
    let rest = until(&mut rx, is_turn_over).await;

    let names: Vec<_> = rest.iter().map(AgentEvent::kind).collect();
    assert!(names.contains(&"permissionResolved"), "{names:?}");
    assert!(names.contains(&"turnCancelled"), "{names:?}");
}

#[tokio::test]
async fn stopping_a_call_tells_the_model_to_wait_and_ends_the_turn() {
    let dir = TempProject::new();
    dir.write(
        "junk.rs", "dead
",
    );

    // Two turns are scripted; the second must never be asked for. A turn that
    // carried on after the human said stop would consume it.
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "junk.rs")),
        Turn::Events(script_text("unreachable")),
        Turn::Events(script_text("Alright.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "delete it").await;
    until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;

    handle.send(Command::Cancel).await.unwrap();
    let rest = until(&mut rx, is_turn_over).await;

    // §5.5: the human's decision, not a failure — and its own sentence.
    assert!(
        rest.iter()
            .any(|e| matches!(e, AgentEvent::ToolCallDenied { stopped: true, .. })),
        "{:?}",
        rest.iter().map(AgentEvent::kind).collect::<Vec<_>>()
    );
    assert_eq!(
        provider.seen.lock().unwrap().len(),
        1,
        "модель спросили второй раз после остановки"
    );

    // The refusal is only visible to the model on the next request, so the
    // human sends one. Its history has to carry the sentence.
    ask(&handle, "ладно, оставь").await;
    until(&mut rx, is_turn_over).await;

    let seen = provider.seen.lock().unwrap();
    let told = seen[1].messages.iter().any(|m| {
        m.content.iter().any(|b| {
            matches!(
                b,
                ContentBlock::ToolResult { content, is_error, .. }
                    if *is_error && content.contains("STOP what you are doing")
            )
        })
    });
    assert!(told, "модели не сказали, что человек остановил вызов");
}

/// The scripted turn behind both edit tests: read the file, then edit it.
fn renaming() -> Arc<MockProvider> {
    Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "Read",
            serde_json::json!({ "path": "a.rs", "description": "Looked at it" }),
        )])),
        Turn::Events(calls(&[(
            "t2",
            "Edit",
            serde_json::json!({ "path": "a.rs", "old_string": "one", "new_string": "two" }),
        )])),
        Turn::Events(script_text("Renamed.")),
    ]))
}

#[tokio::test]
async fn auto_mode_edits_without_asking() {
    let dir = TempProject::new();
    let path = dir.write("a.rs", "one\n");

    let handle = session(&dir, renaming(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "rename it").await;
    let seen = until(&mut rx, is_turn_over).await;

    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, AgentEvent::PermissionRequested { .. })),
        "§8: Auto does not ask about edits"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "two\n");
}

/// §8, §16.37: Manual asks before the edit, and that question is the only
/// confirmation there is — the file is written the moment it is answered.
#[tokio::test]
async fn manual_mode_asks_once_and_then_writes() {
    let dir = TempProject::new();
    let path = dir.write("a.rs", "one\n");

    let handle = session(&dir, renaming(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "rename it").await;
    let asking = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;

    let (id, kind, ..) = request(&asking);
    assert_eq!(kind, PermissionKind::EditFile);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "one\n",
        "nothing is written while the question is open"
    );

    handle
        .send(Command::AnswerPermission {
            request_id: id,
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "two\n",
        "one answer is enough: there is no second button"
    );
}

/// A refusal written into `.yaca/settings.json` by the settings window (§10)
/// is in force from the first call of the session: the tool is refused, the
/// reason reaches the agent, and no question is put to the human.
#[tokio::test]
async fn a_standing_refusal_from_the_project_file_needs_no_question() {
    let dir = TempProject::new();
    let victim = dir.write("junk.rs", "dead code\n");
    dir.write(
        ".yaca/settings.json",
        r#"{
          "permissions": {
            "denies": [
              {
                "scope": { "type": "directory", "path": "." },
                "reason": "удалять здесь нечего, просто перечитай"
              }
            ]
          }
        }"#,
    );

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "junk.rs")),
        Turn::Events(script_text("Понял.")),
    ]));
    // Auto, so that nothing but the refusal could be stopping the call:
    // in Auto a deletion goes through without asking (§8).
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "снеси мёртвый файл").await;
    let events = until(&mut rx, is_turn_over).await;

    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::PermissionRequested { .. })),
        "a standing refusal answers the question instead of asking it"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentEvent::ToolCallDenied {
                reason: Some(_),
                ..
            }
        )),
        "the refusal keeps its reason: {:?}",
        events.iter().map(AgentEvent::kind).collect::<Vec<_>>()
    );
    assert!(
        victim.exists(),
        "the file the refusal protects is still there"
    );
}

/// A refusal outranks a grant covering the same call, and it is consulted
/// before the mode — so the two lists in the settings window mean what they
/// say: "запрещено всегда" wins.
#[tokio::test]
async fn a_refusal_outranks_a_grant_of_the_same_scope() {
    let dir = TempProject::new();
    let victim = dir.write("junk.rs", "dead code\n");
    dir.write(
        ".yaca/settings.json",
        r#"{
          "permissions": {
            "rules": [{ "type": "directory", "path": "." }],
            "denies": [{ "scope": { "type": "directory", "path": "." } }]
          }
        }"#,
    );

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "junk.rs")),
        Turn::Events(script_text("Понял.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "снеси мёртвый файл").await;
    let events = until(&mut rx, is_turn_over).await;

    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolCallDenied { .. })),
        "the grant must not undo the refusal"
    );
    assert!(victim.exists());
}

// --- invariants, read-only commands, and what a deletion loses (§8) ----------

fn bash(id: &str, command: &str) -> Vec<ProviderEvent> {
    calls(&[(id, "Bash", serde_json::json!({ "command": command }))])
}

fn delete_dir(id: &str, path: &str) -> Vec<ProviderEvent> {
    calls(&[(id, "DeleteDir", serde_json::json!({ "path": path }))])
}

fn first_request(events: &[AgentEvent]) -> Option<yaca_core::PermissionRequest> {
    events.iter().find_map(|e| match e {
        AgentEvent::PermissionRequested { request } => Some(request.clone()),
        _ => None,
    })
}

fn saw(events: &[AgentEvent], kind: &str) -> bool {
    events.iter().any(|e| e.kind() == kind)
}

/// A git push is put to the human under bypass: the flag turns permissions
/// off, and a push is an invariant, not a permission (§8).
#[tokio::test]
async fn a_push_is_asked_under_bypass() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(bash("t1", "git push origin main")),
        Turn::Events(script_text("Stopped.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();
    handle.send(Command::SetBypass { on: true }).await.unwrap();

    ask(&handle, "publish").await;
    let asking = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    let request = first_request(&asking).unwrap();
    assert_eq!(request.invariant, Some(Invariant::GitPush));
    assert!(
        request.scopes.is_empty(),
        "no rule may be offered for a push"
    );

    handle
        .send(Command::AnswerPermission {
            request_id: request.id,
            answer: PermissionAnswer::Deny { reason: None },
        })
        .await
        .unwrap();
    let rest = until(&mut rx, is_turn_over).await;
    assert!(
        saw(&rest, "toolCallDenied"),
        "the refused push must not run"
    );
}

/// "Always allow git" covers `git` as a command. It must not pass a commit,
/// even under bypass: the question is put again.
#[tokio::test]
async fn an_always_allow_for_git_does_not_pass_a_commit() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(bash("t1", "mkdir made")),
        Turn::Events(bash("t2", "git commit -m wip")),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "work").await;
    let first = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    // A "git" grant is stored, then bypass is switched on: the commit is still asked.
    let mkdir = first_request(&first).unwrap();
    handle
        .send(Command::AnswerPermission {
            request_id: mkdir.id,
            answer: PermissionAnswer::AllowAlways {
                scope: ScopeKey::CommandPrefix {
                    prefix: "git".into(),
                },
            },
        })
        .await
        .unwrap();

    handle.send(Command::SetBypass { on: true }).await.unwrap();
    let second = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    let commit = first_request(&second).unwrap();
    assert_eq!(commit.invariant, Some(Invariant::GitCommit));
    handle
        .send(Command::AnswerPermission {
            request_id: commit.id,
            answer: PermissionAnswer::Deny { reason: None },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;
}

/// Plan refuses a write before any question is put: a question about an action
/// Plan does not allow would ask for something that cannot happen.
#[tokio::test]
async fn plan_refuses_a_write_command_without_asking() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(bash("t1", "echo hi > made.txt")),
        Turn::Events(script_text("Noted.")),
    ]));
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "plan it").await;
    let rest = until(&mut rx, is_turn_over).await;

    assert!(!saw(&rest, "permissionRequested"), "no question in Plan");
    assert!(saw(&rest, "toolCallRejected"), "the write must be refused");
    assert!(!dir.path().join("made.txt").exists());
}

#[tokio::test]
async fn plan_runs_a_read_only_command_without_asking() {
    let dir = TempProject::new();
    dir.write("notes.txt", "remember\n");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(bash("t1", "cat notes.txt")),
        Turn::Events(script_text("Read it.")),
    ]));
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "read the notes").await;
    let rest = until(&mut rx, is_turn_over).await;

    assert!(!saw(&rest, "permissionRequested"));
    assert!(saw(&rest, "toolCallFinished"), "{rest:?}");
}

#[tokio::test]
async fn auto_runs_a_read_only_command_without_asking() {
    let dir = TempProject::new();
    dir.write("notes.txt", "remember\n");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(bash("t1", "cat notes.txt")),
        Turn::Events(script_text("Read it.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "read the notes").await;
    let rest = until(&mut rx, is_turn_over).await;

    assert!(!saw(&rest, "permissionRequested"));
    assert!(saw(&rest, "toolCallFinished"));
}

/// The directory holds `target`, which no snapshot keeps: Auto must ask, and the
/// question must say what will not come back.
#[tokio::test]
async fn auto_asks_before_deleting_a_directory_a_rollback_cannot_restore() {
    let dir = TempProject::new();
    let victim = dir.write("pkg/target/debug/app.rlib", "built\n");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete_dir("t1", "pkg")),
        Turn::Events(script_text("Kept it.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "clean up").await;
    let asking = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    let request = first_request(&asking).unwrap();
    let unsaved = request.unsaved.clone().expect("the loss must be named");
    assert_eq!(unsaved.dirs, vec!["pkg/target".to_string()]);
    assert_eq!(unsaved.dir_count, 1);

    handle
        .send(Command::AnswerPermission {
            request_id: request.id,
            answer: PermissionAnswer::Deny { reason: None },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;
    assert!(victim.exists(), "a refused deletion removes nothing");
}

/// Under bypass the same deletion goes through unasked: bypass switches the
/// permission off (§8), and the question about the loss is one of those.
#[tokio::test]
async fn bypass_deletes_what_a_rollback_cannot_restore_without_asking() {
    let dir = TempProject::new();
    let victim = dir.write("pkg/target/debug/app.rlib", "built\n");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete_dir("t1", "pkg")),
        Turn::Events(script_text("Gone.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();
    handle.send(Command::SetBypass { on: true }).await.unwrap();

    ask(&handle, "clean up").await;
    let rest = until(&mut rx, is_turn_over).await;

    assert!(!saw(&rest, "permissionRequested"));
    assert!(!victim.exists());
}

/// A directory the snapshot keeps in full is deleted in Auto as before.
#[tokio::test]
async fn auto_deletes_a_fully_saved_directory_without_asking() {
    let dir = TempProject::new();
    let victim = dir.write("docs/a.md", "text\n");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete_dir("t1", "docs")),
        Turn::Events(script_text("Removed.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "tidy").await;
    let rest = until(&mut rx, is_turn_over).await;

    assert!(!saw(&rest, "permissionRequested"));
    assert!(!victim.exists());
}

// --- what a deletion loses, and git's own configuration (§8) ---------------

fn asked_about(events: &[AgentEvent]) -> yaca_core::permission::PermissionRequest {
    events
        .iter()
        .find_map(|e| match e {
            AgentEvent::PermissionRequested { request } => Some(request.clone()),
            _ => None,
        })
        .expect("no permission was requested")
}

#[tokio::test]
async fn a_deletion_that_loses_what_no_snapshot_keeps_is_asked_in_auto() {
    let dir = TempProject::new();
    let victim = dir.write("data/dump.bin", &"x".repeat(8 * 1024 * 1024 + 1));

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "data/dump.bin")),
        Turn::Events(script_text("Removed.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "drop the dump").await;
    let asking = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;

    // Auto deletes a small file unasked; this one is over the snapshot's limit.
    let request = asked_about(&asking);
    assert_eq!(request.unsaved.map(|u| u.files), Some(1));
    assert!(victim.exists());

    handle
        .send(Command::AnswerPermission {
            request_id: request.id,
            answer: PermissionAnswer::Deny { reason: None },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;
    assert!(victim.exists(), "a refused deletion deletes nothing");
}

#[tokio::test]
async fn a_directory_the_project_excludes_in_settings_is_asked_in_auto() {
    let dir = TempProject::new();
    dir.write(
        ".yaca/settings.json",
        r#"{ "shadow": { "exclude": ["fixtures"] } }"#,
    );
    let golden = dir.write("suite/fixtures/golden.json", "{}\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "DeleteDir",
            serde_json::json!({ "path": "suite" }),
        )])),
        Turn::Events(script_text("Removed.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "drop the suite").await;
    let asking = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;

    // Built-in list knows nothing of `fixtures`; the project's own settings
    // are what make the snapshot skip it, and so what makes it unsaved.
    let request = asked_about(&asking);
    let unsaved = request.unsaved.expect("fixtures/ is in no snapshot");
    assert_eq!(unsaved.dirs, vec!["suite/fixtures".to_string()]);
    assert!(golden.exists());

    handle
        .send(Command::AnswerPermission {
            request_id: request.id,
            answer: PermissionAnswer::Deny { reason: None },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;
}

fn write_file(id: &str, path: &str) -> Vec<ProviderEvent> {
    calls(&[(
        id,
        "Write",
        serde_json::json!({ "path": path, "content": "[alias]\n\tx = !sh\n" }),
    )])
}

#[tokio::test]
async fn git_configuration_is_asked_in_auto_and_the_refusal_writes_nothing() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(write_file("t1", ".git/config")),
        Turn::Events(script_text("Stopped.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "set up git").await;
    let asking = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;

    // Auto writes ordinary files unasked; git's configuration is a question.
    let request = asked_about(&asking);
    assert_eq!(request.invariant, Some(Invariant::GitConfig));
    assert!(request.scopes.is_empty(), "an invariant offers no rule");

    handle
        .send(Command::AnswerPermission {
            request_id: request.id,
            answer: PermissionAnswer::Deny { reason: None },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;
    assert!(!dir.path().join(".git/config").exists());
}

#[tokio::test]
async fn git_configuration_is_asked_under_bypass_too() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(write_file("t1", ".gitattributes")),
        Turn::Events(script_text("Stopped.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    handle.send(Command::SetBypass { on: true }).await.unwrap();
    ask(&handle, "set up git").await;
    let asking = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;

    // Bypass turns off permissions (§8); an invariant is not one of them.
    let request = asked_about(&asking);
    assert_eq!(request.invariant, Some(Invariant::GitConfig));
    handle
        .send(Command::AnswerPermission {
            request_id: request.id,
            answer: PermissionAnswer::Deny { reason: None },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;
    assert!(!dir.path().join(".gitattributes").exists());
}
