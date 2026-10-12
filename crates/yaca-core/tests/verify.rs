//! Check after edit (`verify.rs`), end to end: a project's own check runs after
//! a batch that changed files, and its outcome reaches both the feed and the
//! result the model reads.
//!
//! The commands here are real processes in a real temporary project, because
//! the thing under test is how they are started, stopped and reported.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::message::ContentBlock;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;

use common::{ask, calls, is_turn_over, session, until_within, TempProject};

const LIMIT: Duration = Duration::from_secs(30);

/// Writes `.yaca/settings.json` with the given `verify` value.
fn configure(dir: &TempProject, verify: serde_json::Value) {
    dir.write(
        ".yaca/settings.json",
        &json!({ "verify": verify }).to_string(),
    );
}

/// A command that prints `text` and exits with `code`, in whichever shell the
/// platform runs commands through.
fn echo_and_exit(text: &str, code: i32) -> String {
    format!("echo {text}; exit {code}")
}

/// A command that adds one line to `runs.txt` every time it runs. Counting the
/// lines counts the runs, whatever else the check does.
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

/// The text of the result the model read for `id` in its last request.
fn result_of(provider: &MockProvider, id: &str) -> String {
    let sent = provider.seen.lock().unwrap().last().unwrap().clone();
    sent.messages
        .last()
        .expect("запрос без сообщений")
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } if tool_use_id == id => Some(content.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no result for {id}"))
}

/// What the feed was given for one call, in the order it arrived.
fn feed_of(seen: &[AgentEvent], id: &str) -> String {
    seen.iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallProgress { call_id, chunk } if call_id.0 == id => {
                Some(chunk.as_str())
            }
            _ => None,
        })
        .collect()
}

/// One batch of a single new file, then the model's closing words.
fn write_one_file(id: &str, path: &str) -> Vec<Turn> {
    vec![
        Turn::Events(calls(&[(
            id,
            "Write",
            json!({ "path": path, "content": "fn main() {}\n" }),
        )])),
        Turn::Events(script_text("готово")),
    ]
}

#[tokio::test]
async fn a_passing_check_is_reported_to_the_model_and_the_feed() {
    let dir = TempProject::new();
    configure(&dir, json!({ "command": "echo checked" }));
    let provider = Arc::new(MockProvider::new(write_one_file("w1", "a.rs")));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "создай файл").await;
    let seen = until_within(&mut rx, LIMIT, is_turn_over).await;

    let model = result_of(&provider, "w1");
    assert!(model.contains("[verify] `echo checked` passed."), "{model}");
    assert!(feed_of(&seen, "w1").contains("без ошибок"), "{seen:?}");
}

#[tokio::test]
async fn a_failing_check_hands_its_output_to_the_model() {
    let dir = TempProject::new();
    configure(&dir, json!({ "command": echo_and_exit("error-here", 1) }));
    let provider = Arc::new(MockProvider::new(write_one_file("w1", "a.rs")));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "создай файл").await;
    let seen = until_within(&mut rx, LIMIT, is_turn_over).await;

    let model = result_of(&provider, "w1");
    assert!(model.contains("failed with exit code 1"), "{model}");
    assert!(model.contains("error-here"), "{model}");

    let feed = feed_of(&seen, "w1");
    assert!(feed.contains("завершилась кодом 1"), "{feed}");
    assert!(feed.contains("error-here"), "{feed}");
}

#[tokio::test]
async fn a_check_that_overruns_is_stopped_and_reported_as_timed_out() {
    let dir = TempProject::new();
    configure(&dir, json!({ "command": sleeps(), "timeoutSecs": 1 }));
    let provider = Arc::new(MockProvider::new(write_one_file("w1", "a.rs")));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    let started = std::time::Instant::now();
    ask(&handle, "создай файл").await;
    let seen = until_within(&mut rx, LIMIT, is_turn_over).await;

    // The sleep is twenty seconds; the check gave up at one and took its
    // process tree with it, so the turn did not wait for the sleep.
    assert!(started.elapsed() < Duration::from_secs(15));

    let model = result_of(&provider, "w1");
    assert!(model.contains("did not finish within 1 s"), "{model}");
    assert!(model.contains("did not pass"), "{model}");
    assert!(
        !model.contains("passed."),
        "a timeout must not read as a pass: {model}"
    );

    let feed = feed_of(&seen, "w1");
    assert!(feed.contains("не уложилась в 1 с"), "{feed}");
}

#[tokio::test]
async fn a_batch_of_changes_is_checked_once_and_the_outcome_goes_on_the_last_one() {
    let dir = TempProject::new();
    configure(&dir, json!({ "command": count_runs() }));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            ("w1", "Write", json!({ "path": "a.rs", "content": "a\n" })),
            ("w2", "Write", json!({ "path": "b.rs", "content": "b\n" })),
        ])),
        Turn::Events(script_text("готово")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "создай два файла").await;
    until_within(&mut rx, LIMIT, is_turn_over).await;

    let runs = std::fs::read_to_string(dir.path().join("runs.txt")).unwrap();
    assert_eq!(runs.lines().count(), 1, "two changes, one check: {runs:?}");

    // The outcome is said once, on the change that came last.
    assert!(!result_of(&provider, "w1").contains("[verify]"));
    assert!(result_of(&provider, "w2").contains("[verify]"));
}

#[tokio::test]
async fn a_batch_that_changed_nothing_is_not_checked() {
    let dir = TempProject::new();
    dir.write("a.rs", "fn main() {}\n");
    configure(&dir, json!({ "command": count_runs() }));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("r1", "Read", json!({ "path": "a.rs" }))])),
        Turn::Events(script_text("прочитал")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "посмотри файл").await;
    until_within(&mut rx, LIMIT, is_turn_over).await;

    assert!(
        !dir.path().join("runs.txt").exists(),
        "a read is not a change and earns no check"
    );
    assert!(!result_of(&provider, "r1").contains("[verify]"));
}

/// Without the setting, an edit is exactly what it was before this module
/// existed: no check runs, nothing is appended, nothing is said in the feed.
#[tokio::test]
async fn without_the_setting_an_edit_is_not_checked() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(write_one_file("w1", "a.rs")));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "создай файл").await;
    let seen = until_within(&mut rx, LIMIT, is_turn_over).await;

    let model = result_of(&provider, "w1");
    assert!(!model.contains("[verify]"), "{model}");
    assert!(!feed_of(&seen, "w1").contains("Проверка"), "{seen:?}");
    assert!(dir.path().join("a.rs").exists());
}
