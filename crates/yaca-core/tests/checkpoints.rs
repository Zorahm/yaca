//! Checkpoints end to end (§5.8, §13).
//!
//! Real files and a real shell. A snapshot that only convinces a mock is a
//! snapshot of the mock.
//!
//! Driven through the public surface only — `spawn`, `Command`, `AgentEvent` —
//! because that is exactly the surface the interface has.

mod common;

use std::sync::Arc;
use std::time::Duration;

use yaca_core::checkpoint::{CheckpointReason, CheckpointSummary};
use yaca_core::event::AgentEvent;
use yaca_core::id::{CheckpointId, SessionId};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::ProviderEvent;
use yaca_core::{Command, Mode, PermissionAnswer, SessionConfig, SessionHandle};
use serde_json::json;
use tokio::sync::broadcast::Receiver;

use common::{ask, calls, is_turn_over, session, until, until_within, TempProject};

/// A line written through the shell, so the change comes from a real command
/// and not from the core pretending to be one.
///
/// Not `echo x > f` on Windows: Windows PowerShell's `>` writes UTF-16, and
/// the test reads the file back as text. The trailing newline differs by
/// platform, so tests look for the word, not for the exact bytes.
fn write_through_shell(path: &str, line: &str) -> String {
    if cfg!(windows) {
        format!("Set-Content -Path {path} -Value {line}")
    } else {
        format!("echo {line} > {path}")
    }
}

fn runs(id: &str, command: &str) -> Vec<ProviderEvent> {
    calls(&[(
        id,
        "Bash",
        json!({ "command": command, "description": "Ran the command" }),
    )])
}

fn writes(id: &str, path: &str, content: &str) -> Vec<ProviderEvent> {
    calls(&[(
        id,
        "Write",
        json!({ "path": path, "content": content }),
    )])
}

fn edits(id: &str, path: &str, from: &str, to: &str) -> Vec<ProviderEvent> {
    calls(&[(
        id,
        "Edit",
        json!({ "path": path, "old_string": from, "new_string": to }),
    )])
}

fn points(events: &[AgentEvent]) -> Vec<CheckpointSummary> {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::CheckpointCreated { checkpoint } => Some(checkpoint.clone()),
            _ => None,
        })
        .collect()
}

/// Answers every permission with "allow" until the turn ends.
///
/// Commands are asked about in every trust mode (§8), so a turn that runs two
/// of them stops twice, and a helper that answers once would sit waiting for a
/// turn that is waiting for it.
async fn allow_everything(
    handle: &SessionHandle,
    rx: &mut Receiver<AgentEvent>,
) -> Vec<AgentEvent> {
    let mut all = Vec::new();
    loop {
        let batch = until_within(rx, Duration::from_secs(30), |e| {
            matches!(e, AgentEvent::PermissionRequested { .. }) || is_turn_over(e)
        })
        .await;
        let last = batch.last().cloned();
        all.extend(batch);

        match last {
            Some(AgentEvent::PermissionRequested { request }) => handle
                .send(Command::AnswerPermission {
                    request_id: request.id,
                    answer: PermissionAnswer::Allow,
                })
                .await
                .unwrap(),
            _ => return all,
        }
    }
}

async fn roll_back_to(
    handle: &SessionHandle,
    rx: &mut Receiver<AgentEvent>,
    id: &CheckpointId,
) -> u32 {
    handle
        .send(Command::RestoreCheckpoint { id: id.clone() })
        .await
        .unwrap();
    let seen = until(rx, |e| matches!(e, AgentEvent::CheckpointRestored { .. })).await;
    match seen.last() {
        Some(AgentEvent::CheckpointRestored { files, .. }) => *files,
        other => panic!("no rollback: {other:?}"),
    }
}

// --- tests ------------------------------------------------------------------

/// §5.8 names this case outright: the snapshot has to cover git-ignored files,
/// so a `.env` a command overwrote must come back.
#[tokio::test]
async fn a_command_is_snapshotted_and_a_rollback_brings_back_even_env() {
    let dir = TempProject::new();
    dir.write(".gitignore", ".env\n");
    let env = dir.write(".env", "SECRET=old\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(runs("t1", &write_through_shell(".env", "SECRET=new"))),
        Turn::Events(script_text("Rewritten.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "rewrite the env file").await;
    let turn = allow_everything(&handle, &mut rx).await;

    let point = points(&turn)
        .into_iter()
        .next()
        .expect("a permitted command must leave a point behind");
    assert!(std::fs::read_to_string(&env)
        .unwrap()
        .contains("SECRET=new"));

    let files = roll_back_to(&handle, &mut rx, &point.id).await;
    assert_eq!(files, 1);
    assert_eq!(std::fs::read_to_string(&env).unwrap(), "SECRET=old\n");
}

/// The point before the first command is the one the turn already took; the
/// second command runs against a tree that has moved, so it gets its own.
#[tokio::test]
async fn a_command_that_follows_a_change_gets_a_point_of_its_own() {
    let dir = TempProject::new();
    dir.write("a.txt", "keep\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(runs("t1", &write_through_shell("first.txt", "one"))),
        Turn::Events(runs("t2", &write_through_shell("second.txt", "two"))),
        Turn::Events(script_text("Both written.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "write two files").await;
    let turn = allow_everything(&handle, &mut rx).await;

    let reasons: Vec<_> = points(&turn).into_iter().map(|p| p.reason).collect();
    assert!(
        matches!(reasons.first(), Some(CheckpointReason::Message)),
        "the turn opens with the point `Esc Esc` returns to: {reasons:?}"
    );
    assert!(
        reasons
            .iter()
            .any(|r| matches!(r, CheckpointReason::Command { .. })),
        "the second command must not share a point with the first: {reasons:?}"
    );
}

/// §5.8, §16.37: writes land at once, so the point taken before the message
/// is the one that undoes them. There is no apply and no point of its own.
#[tokio::test]
async fn a_written_file_is_undone_by_the_point_before_the_message() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(writes("t1", "src/new.rs", "fn one() {}\n")),
        Turn::Events(script_text("Written.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "add a file").await;
    let seen = until(&mut rx, is_turn_over).await;

    let created = dir.path().join("src/new.rs");
    assert!(created.exists(), "the write reaches the tree at once");

    let point = points(&seen)
        .into_iter()
        .find(|p| p.reason == CheckpointReason::Message)
        .expect("a point is taken before the message");

    let files = roll_back_to(&handle, &mut rx, &point.id).await;
    assert_eq!(files, 1);
    assert!(!created.exists(), "and the file it created is gone again");
}

#[tokio::test]
async fn the_rollback_names_its_files_before_touching_anything() {
    let dir = TempProject::new();
    let kept = dir.write("kept.txt", "one\ntwo\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(runs("t1", &write_through_shell("kept.txt", "wiped"))),
        Turn::Events(script_text("Overwritten.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "overwrite it").await;
    let turn = allow_everything(&handle, &mut rx).await;
    let point = points(&turn).into_iter().next().unwrap();

    handle
        .send(Command::PreviewCheckpoint {
            id: point.id.clone(),
        })
        .await
        .unwrap();
    let seen = until(&mut rx, |e| {
        matches!(e, AgentEvent::CheckpointPreviewed { .. })
    })
    .await;

    let AgentEvent::CheckpointPreviewed { files, .. } = seen.last().unwrap() else {
        unreachable!("until() only returns on a match");
    };
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "kept.txt");
    // §5.8: the list is shown, and then the human confirms. Nothing has moved.
    assert!(std::fs::read_to_string(&kept).unwrap().contains("wiped"));
}

/// After a rollback the agent's memory of a file is a memory of a tree that no
/// longer exists. Same rule as §11: it has to read the file again.
#[tokio::test]
async fn a_rolled_back_file_has_to_be_read_again_before_it_is_edited() {
    let dir = TempProject::new();
    dir.write("a.txt", "original\n");

    let provider = Arc::new(MockProvider::new(vec![
        // Reads it, so read-before-edit is satisfied; then a command moves it.
        Turn::Events(calls(&[(
            "t1",
            "Read",
            json!({ "path": "a.txt", "description": "Read the file" }),
        )])),
        Turn::Events(runs("t2", &write_through_shell("a.txt", "changed"))),
        Turn::Events(script_text("Done.")),
        // After the rollback, editing on the old memory must be refused.
        Turn::Events(edits("t3", "a.txt", "original", "edited")),
        Turn::Events(script_text("Refused.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "read it, then overwrite it").await;
    let turn = allow_everything(&handle, &mut rx).await;
    let point = points(&turn)
        .into_iter()
        .find(|p| matches!(p.reason, CheckpointReason::Command { .. }))
        .expect("the command runs against a tree the read did not change");

    roll_back_to(&handle, &mut rx, &point.id).await;

    // The edit is permitted — Manual asks about edits now (§8) — and then the
    // invariant refuses it anyway: permissions bind the human, invariants the
    // model, and the second does not care what the first answered.
    ask(&handle, "now edit it").await;
    let second = allow_everything(&handle, &mut rx).await;
    let names: Vec<_> = second.iter().map(AgentEvent::kind).collect();
    assert!(
        names.contains(&"toolCallRejected"),
        "the freshness invariant must fire: {names:?}"
    );
}

#[tokio::test]
async fn the_panel_gets_the_whole_list_and_a_rollback_adds_to_it() {
    let dir = TempProject::new();
    dir.write("a.txt", "one\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(runs("t1", &write_through_shell("a.txt", "two"))),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "change it").await;
    let turn = allow_everything(&handle, &mut rx).await;

    let listed = turn
        .iter()
        .rev()
        .find_map(|e| match e {
            AgentEvent::CheckpointsUpdated { checkpoints } => Some(checkpoints.clone()),
            _ => None,
        })
        .expect("the panel is sent whole, not assembled from dividers");
    let point = listed.last().unwrap().clone();

    roll_back_to(&handle, &mut rx, &point.id).await;
    let after = until(&mut rx, |e| {
        matches!(e, AgentEvent::CheckpointsUpdated { .. })
    })
    .await;

    let AgentEvent::CheckpointsUpdated { checkpoints } = after.last().unwrap() else {
        unreachable!("until() only returns on a match");
    };
    // The tree the rollback replaced is a point too, so the list has grown.
    assert!(
        checkpoints.len() > listed.len(),
        "{:?}",
        checkpoints.iter().map(|c| &c.reason).collect::<Vec<_>>()
    );
}

/// A session that writes its journals, so the list of its points is kept too.
fn recorded(
    dir: &TempProject,
    provider: Arc<MockProvider>,
    resume: Option<SessionId>,
) -> SessionHandle {
    let mut config = SessionConfig::new(dir.path());
    config.mode = Mode::Auto;
    config.journal = Some(dir.chats());
    config.resume = resume;
    yaca_core::spawn(config, provider).unwrap()
}

/// What a reload of the window, a closed chat or a restart used to lose: the
/// snapshots were on disk, but the list of them was in the session's memory.
#[tokio::test]
async fn a_point_outlives_the_session_that_took_it() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(writes("t1", "src/new.rs", "fn one() {}
")),
        Turn::Events(script_text("Written.")),
    ]));
    let first = recorded(&dir, provider, None);
    let mut rx = first.subscribe();

    ask(&first, "add a file").await;
    let seen = until(&mut rx, is_turn_over).await;
    let created = dir.path().join("src/new.rs");
    assert!(created.exists());
    let point = points(&seen)
        .into_iter()
        .find(|p| p.reason == CheckpointReason::Message)
        .expect("a point is taken before the message");
    let id = first.id().clone();
    drop(first);

    // The chat is closed. What the window draws when it is opened again has
    // the divider, and the panel has its list.
    let feed = dir.store().feed(&id);
    assert!(
        points(&feed).iter().any(|p| p.id == point.id),
        "the divider of a point that can be rolled back to stays"
    );
    assert!(feed.iter().any(|e| matches!(
        e,
        AgentEvent::CheckpointsUpdated { checkpoints } if checkpoints.iter().any(|p| p.id == point.id)
    )));

    // And the session that is opened holds that point: the rollback works.
    let again = recorded(&dir, Arc::new(MockProvider::new(vec![])), Some(id));
    let mut rx = again.subscribe();
    let files = roll_back_to(&again, &mut rx, &point.id).await;
    assert_eq!(files, 1);
    assert!(!created.exists(), "the file the first session wrote is gone");
}

/// The list belongs to its session: deleting the chat takes its points with it,
/// and a copy of the chat does not pretend to have any.
#[tokio::test]
async fn a_deleted_or_copied_chat_has_no_points_to_offer() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(writes("t1", "src/new.rs", "fn one() {}
")),
        Turn::Events(script_text("Written.")),
    ]));
    let first = recorded(&dir, provider, None);
    let mut rx = first.subscribe();
    ask(&first, "add a file").await;
    until(&mut rx, is_turn_over).await;
    let id = first.id().clone();
    drop(first);

    let store = dir.store();
    let copy = store.duplicate(&id, None).unwrap();
    assert!(
        points(&store.feed(&copy.id)).is_empty(),
        "a copy has no list of its own, so its feed offers no rollback"
    );

    store.delete(&id).unwrap();
    assert!(store.feed(&id).is_empty());
    assert!(!yaca_core::checkpoint::ledger::path(&dir.journals(), &id).exists());
}
