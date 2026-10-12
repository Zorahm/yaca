//! Hooks around tool calls (`hooks.rs`), end to end: a hook in
//! `.yaca/settings.json` decides or annotates a call, and the result is what the
//! feed and the model see.
//!
//! The hook commands are real processes in a real temporary project, because
//! what is under test is how they are started, stopped, timed out and reported.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;

use common::{ask, calls, is_turn_over, session, tool_result_of, until_within, TempProject};

const LIMIT: Duration = Duration::from_secs(30);

/// Writes `.yaca/settings.json` with the given `hooks` value.
fn configure(dir: &TempProject, hooks: serde_json::Value) {
    dir.write(
        ".yaca/settings.json",
        &json!({ "hooks": hooks }).to_string(),
    );
}

/// A command that prints `text` and exits with `code`, in whichever shell the
/// platform runs commands through.
fn echo_and_exit(text: &str, code: i32) -> String {
    format!("echo {text}; exit {code}")
}

/// A command that adds a line to `runs.txt` each time it runs. Its presence is
/// the proof that the hook ran.
fn count_runs() -> &'static str {
    if cfg!(windows) {
        "Add-Content -Path runs.txt -Value run"
    } else {
        "echo run >> runs.txt"
    }
}

fn sleeps() -> &'static str {
    if cfg!(windows) {
        "Start-Sleep -Seconds 20"
    } else {
        "sleep 20"
    }
}

/// One write of `a.rs`, then the model's closing words.
fn write_a_file(id: &str) -> Vec<Turn> {
    vec![
        Turn::Events(calls(&[(
            id,
            "Write",
            json!({ "path": "a.rs", "content": "fn main() {}\n" }),
        )])),
        Turn::Events(script_text("готово")),
    ]
}

/// What the feed was given for one call, as text.
fn feed_of(seen: &[AgentEvent], id: &str) -> String {
    seen.iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallProgress { call_id, chunk } if call_id.0 == id => {
                Some(chunk.as_str())
            }
            AgentEvent::ToolCallRejected { call_id, reason } if call_id.0 == id => {
                Some(reason.as_str())
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_refusing_pre_hook_stops_the_call_before_anyone_is_asked() {
    let dir = TempProject::new();
    configure(
        &dir,
        json!({ "preToolUse": [{ "match": "Write", "command": echo_and_exit("no-writes-here", 2) }] }),
    );
    // Manual, so that a Write would have to be asked about. The hook must answer
    // first: the question never reaches the human.
    let provider = Arc::new(MockProvider::new(write_a_file("w1")));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "создай файл").await;
    let seen = until_within(&mut rx, LIMIT, is_turn_over).await;

    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, AgentEvent::PermissionRequested { .. })),
        "a call a hook refuses must not reach the permission question"
    );
    assert!(!dir.path().join("a.rs").exists(), "the refused write ran");

    let (text, is_error) = tool_result_of(&provider, "w1");
    assert!(is_error, "{text}");
    assert!(text.contains("refused this call"), "{text}");
    assert!(text.contains("no-writes-here"), "{text}");
    assert!(text.contains("was not run"), "{text}");

    let feed = feed_of(&seen, "w1");
    assert!(feed.contains("не пропустил вызов"), "{feed}");
    assert!(feed.contains("no-writes-here"), "{feed}");
}

#[tokio::test]
async fn a_pre_hook_that_passes_lets_the_call_run() {
    let dir = TempProject::new();
    configure(
        &dir,
        json!({ "preToolUse": [{ "match": "*", "command": "exit 0" }] }),
    );
    let provider = Arc::new(MockProvider::new(write_a_file("w1")));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "создай файл").await;
    until_within(&mut rx, LIMIT, is_turn_over).await;

    assert!(dir.path().join("a.rs").exists());
    let (_, is_error) = tool_result_of(&provider, "w1");
    assert!(!is_error);
}

#[tokio::test]
async fn a_pre_hook_that_never_answers_refuses_the_call_fail_closed() {
    let dir = TempProject::new();
    configure(
        &dir,
        json!({ "preToolUse": [{ "match": "Write", "command": sleeps(), "timeoutSecs": 1 }] }),
    );
    let provider = Arc::new(MockProvider::new(write_a_file("w1")));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    let started = std::time::Instant::now();
    ask(&handle, "создай файл").await;
    let seen = until_within(&mut rx, LIMIT, is_turn_over).await;

    // The hook was given one second; the turn did not wait out its twenty.
    assert!(started.elapsed() < Duration::from_secs(15));
    assert!(
        !dir.path().join("a.rs").exists(),
        "an unanswered hook let the call through"
    );

    let (text, is_error) = tool_result_of(&provider, "w1");
    assert!(is_error, "{text}");
    assert!(text.contains("did not finish within 1 s"), "{text}");
    assert!(feed_of(&seen, "w1").contains("не ответил за 1 с"));
}

#[tokio::test]
async fn a_hook_for_one_tool_is_not_run_for_another() {
    let dir = TempProject::new();
    configure(
        &dir,
        json!({ "preToolUse": [{ "match": "Bash", "command": count_runs() }] }),
    );
    let provider = Arc::new(MockProvider::new(write_a_file("w1")));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "создай файл").await;
    until_within(&mut rx, LIMIT, is_turn_over).await;

    assert!(
        !dir.path().join("runs.txt").exists(),
        "a Bash hook ran for a Write"
    );
    assert!(dir.path().join("a.rs").exists());
}

#[tokio::test]
async fn a_post_hook_output_is_added_to_the_result_the_model_reads() {
    let dir = TempProject::new();
    configure(
        &dir,
        json!({ "postToolUse": [{ "match": "Write", "command": "echo formatted-ok" }] }),
    );
    let provider = Arc::new(MockProvider::new(write_a_file("w1")));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "создай файл").await;
    let seen = until_within(&mut rx, LIMIT, is_turn_over).await;

    let (text, is_error) = tool_result_of(&provider, "w1");
    assert!(!is_error);
    assert!(text.contains("formatted-ok"), "{text}");
    // The human sees what the model was told, in the call's row.
    assert!(feed_of(&seen, "w1").contains("formatted-ok"));
}

#[tokio::test]
async fn a_failing_post_hook_is_a_note_and_the_call_still_succeeded() {
    let dir = TempProject::new();
    configure(
        &dir,
        json!({ "postToolUse": [{ "match": "Write", "command": echo_and_exit("lint-broke", 3) }] }),
    );
    let provider = Arc::new(MockProvider::new(write_a_file("w1")));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "создай файл").await;
    let seen = until_within(&mut rx, LIMIT, is_turn_over).await;

    // The file was written, so the call is not an error: the hook reports on
    // the write, it does not undo or fail it.
    assert!(dir.path().join("a.rs").exists());
    let (text, is_error) = tool_result_of(&provider, "w1");
    assert!(!is_error, "{text}");
    assert!(text.contains("lint-broke"), "{text}");
    assert!(text.contains("exited with code 3"), "{text}");
    assert!(feed_of(&seen, "w1").contains("кодом 3"));
}
