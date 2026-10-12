//! The tasks that ship in `evals/tasks`.
//!
//! The fast test checks that every shipped task loads and can be verified. The
//! ignored test checks that every starting tree fails its check: a task that
//! already passes measures nothing. It compiles each fixture, so it is slow
//! and runs on demand:
//!
//! ```text
//! cargo test -p yaca-evals --test shipped_tasks -- --ignored
//! ```

use std::path::PathBuf;
use std::time::Duration;

use yaca_evals::check::run_command;
use yaca_evals::task::load_tasks;
use yaca_evals::workdir::{copy_tree, create_unique};

fn shipped_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../evals/tasks"))
}

#[test]
fn every_shipped_task_loads_and_can_be_verified() {
    let tasks = load_tasks(&shipped_dir(), &[]).expect("the shipped tasks must validate");
    assert_eq!(tasks.len(), 10, "the suite is meant to hold ten tasks");
    for task in &tasks {
        assert!(
            task.check.is_some() || !task.expect.is_empty(),
            "task `{}` has nothing to verify it",
            task.id
        );
    }
}

#[tokio::test]
#[ignore = "compiles every fixture; run with --ignored"]
async fn every_starting_tree_fails_its_check() {
    let tasks = load_tasks(&shipped_dir(), &[]).unwrap();
    let work = create_unique(&std::env::temp_dir().join("yaca-evals-shipped"), "start").unwrap();
    let mut solved_already = Vec::new();

    for task in &tasks {
        let Some(check) = &task.check else { continue };
        let tree = work.join(&task.id);
        copy_tree(&task.repo, &tree).unwrap();
        let report = run_command(&tree, check, Duration::from_secs(900)).await;
        if report.passed {
            solved_already.push(task.id.clone());
        }
    }
    let _ = std::fs::remove_dir_all(&work);
    assert!(solved_already.is_empty(), "already passing before any work: {solved_already:?}");
}
