//! Chats of one project that do not share a working tree.
//!
//! Real git, real files. The point is what one chat's tree and rollbacks do to
//! the other's, and a mocked repository would only answer for the mock.

mod common;

use std::path::PathBuf;
use std::process::Command as Git;
use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, SessionConfig, SessionHandle};

use common::{ask, calls, until, TempProject};

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = Git::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn repo() -> TempProject {
    let dir = TempProject::new();
    dir.write("a.txt", "one\n");
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);
    dir
}

fn isolated(dir: &TempProject, file: &str) -> SessionHandle {
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "Write",
            json!({ "path": file, "content": "mine\n" }),
        )])),
        Turn::Events(script_text("Written.")),
    ]));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Auto;
    cfg.journal = None;
    cfg.isolate = true;
    yaca_core::spawn(cfg, provider).unwrap()
}

/// The turn is over for the human when the ring goes out, which is after the
/// work has been put on the branch.
async fn finish_turn(
    rx: &mut tokio::sync::broadcast::Receiver<AgentEvent>,
) -> Vec<AgentEvent> {
    until(rx, |e| {
        matches!(e, AgentEvent::StateChanged { state } if !state.busy)
    })
    .await
}

#[tokio::test]
async fn two_chats_write_into_trees_of_their_own() {
    let dir = repo();
    let one = isolated(&dir, "one.txt");
    let two = isolated(&dir, "two.txt");

    let tree_of = |h: &SessionHandle| -> PathBuf {
        PathBuf::from(h.state().isolation.expect("asked for isolation").path)
    };
    let (a, b) = (tree_of(&one), tree_of(&two));
    assert_ne!(a, b);

    let mut rx_one = one.subscribe();
    let mut rx_two = two.subscribe();
    ask(&one, "write one").await;
    ask(&two, "write two").await;
    finish_turn(&mut rx_one).await;
    finish_turn(&mut rx_two).await;

    assert!(a.join("one.txt").exists());
    assert!(!a.join("two.txt").exists());
    assert!(b.join("two.txt").exists());
    assert!(!b.join("one.txt").exists());
    assert!(
        !dir.path().join("one.txt").exists() && !dir.path().join("two.txt").exists(),
        "the project's own tree is nobody's"
    );

    // The work is on the branch, ready for `git merge`.
    let branch = one.state().isolation.unwrap().branch;
    let files = git(dir.path(), &["ls-tree", "-r", "--name-only", &branch]);
    assert!(files.lines().any(|l| l == "one.txt"), "{files}");
}

#[tokio::test]
async fn a_rollback_in_one_chat_leaves_the_other_alone() {
    let dir = repo();
    let one = isolated(&dir, "one.txt");
    let two = isolated(&dir, "two.txt");
    let mut rx_one = one.subscribe();
    let mut rx_two = two.subscribe();

    ask(&one, "write one").await;
    ask(&two, "write two").await;
    let seen = finish_turn(&mut rx_one).await;
    finish_turn(&mut rx_two).await;

    // The point before the first message of chat one is where its tree was
    // empty; going back there must not reach chat two's file.
    let first = seen
        .iter()
        .find_map(|e| match e {
            AgentEvent::CheckpointCreated { checkpoint } => Some(checkpoint.id.clone()),
            _ => None,
        })
        .expect("a point before the message");
    one.send(Command::RestoreCheckpoint { id: first }).await.unwrap();
    until(&mut rx_one, |e| matches!(e, AgentEvent::CheckpointRestored { .. })).await;

    let a = PathBuf::from(one.state().isolation.unwrap().path);
    let b = PathBuf::from(two.state().isolation.unwrap().path);
    assert!(!a.join("one.txt").exists(), "chat one went back");
    assert!(b.join("two.txt").exists(), "chat two did not");
}

#[test]
fn a_project_outside_git_is_refused_at_start() {
    let dir = TempProject::new();
    let mut cfg = SessionConfig::new(dir.path());
    cfg.journal = None;
    cfg.isolate = true;
    let provider = Arc::new(MockProvider::new(vec![]));
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let result = rt.block_on(async { yaca_core::spawn(cfg, provider) });
    assert!(result.is_err(), "no silent fallback to the shared tree");
}
