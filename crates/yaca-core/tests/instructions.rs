//! The project's instructions file reaches the model (§10).
//!
//! Real files at the project root, and the core driven through the surface the
//! interface has. What is checked is the request the model was sent: reading
//! the instructions is the core's job, not something the model is asked to
//! remember to do.

mod common;

use std::sync::Arc;

use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;

use common::{ask, is_turn_over, session, until, TempProject};

fn replying(turns: usize) -> Arc<MockProvider> {
    Arc::new(MockProvider::new(
        (0..turns).map(|_| Turn::Events(script_text("ok"))).collect(),
    ))
}

/// The system prompt of every request the session has sent, in order.
fn systems(provider: &MockProvider) -> Vec<String> {
    provider
        .seen
        .lock()
        .unwrap()
        .iter()
        .map(|request| request.system.clone())
        .collect()
}

#[tokio::test]
async fn agents_md_is_in_the_system_prompt_in_every_mode() {
    // Plan reads the project too, and a plan that ignores the project's rules
    // is a plan for some other project.
    for mode in [Mode::Plan, Mode::Manual] {
        let dir = TempProject::new();
        dir.write("AGENTS.md", "Commit messages follow Conventional Commits.\n");

        let provider = replying(1);
        let handle = session(&dir, provider.clone(), mode);
        let mut rx = handle.subscribe();
        ask(&handle, "hello").await;
        until(&mut rx, is_turn_over).await;

        let system = &systems(&provider)[0];
        assert!(
            system.contains("Commit messages follow Conventional Commits."),
            "{mode:?}: {system}"
        );
        assert!(system.contains("AGENTS.md"), "{mode:?}");
    }
}

#[tokio::test]
async fn the_file_is_read_once_per_session() {
    // The system prompt is the cached prefix of every request. Re-reading the
    // file per message would restart the cache on every edit, and a model that
    // binds its reasoning to the exact prefix rejects the history outright.
    let dir = TempProject::new();
    dir.write("AGENTS.md", "First version.\n");

    let provider = replying(2);
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "one").await;
    until(&mut rx, is_turn_over).await;
    dir.write("AGENTS.md", "Second version.\n");
    ask(&handle, "two").await;
    until(&mut rx, is_turn_over).await;

    let systems = systems(&provider);
    assert_eq!(systems.len(), 2);
    assert_eq!(systems[0], systems[1]);
    assert!(systems[0].contains("First version."));
}

#[tokio::test]
async fn without_a_file_the_prompt_says_nothing_about_one() {
    let dir = TempProject::new();

    let provider = replying(1);
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "hello").await;
    until(&mut rx, is_turn_over).await;

    let system = &systems(&provider)[0];
    assert!(!system.contains("AGENTS.md"), "{system}");
    assert!(!system.contains("<project_instructions"), "{system}");
}

#[tokio::test]
async fn the_prompt_names_the_project_root() {
    // It used to be shown relative to itself — that is, as an empty string.
    let dir = TempProject::new();

    let provider = replying(1);
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "hello").await;
    until(&mut rx, is_turn_over).await;

    let system = &systems(&provider)[0];
    let name = dir.path().file_name().unwrap().to_str().unwrap();
    assert!(system.contains(name), "{system}");
    // The verbatim prefix `canonicalize` puts on Windows paths is not
    // something cmd.exe accepts as a directory.
    assert!(!system.contains(r"\\?\"), "{system}");
}
