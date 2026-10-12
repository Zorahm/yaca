//! The runner end to end, without a model.
//!
//! A scripted `MockProvider` stands in for the model, so these tests check the
//! runner itself: the copy-and-check cycle, the answers a headless run must
//! give, the timeout, and the shape of what gets written. They say nothing
//! about how well a real model does; that is what the shipped tasks are for.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use yaca_core::provider::mock::{script_text, script_tool_call, MockProvider, Turn};
use yaca_core::provider::Provider;

use yaca_evals::provider::Plan;
use yaca_evals::result::TaskResult;
use yaca_evals::run::{run_task, RunOptions};
use yaca_evals::task::{load_task, Task};
use yaca_evals::workdir::create_unique;

fn scratch(label: &str) -> PathBuf {
    create_unique(&std::env::temp_dir().join("yaca-evals-it"), label).unwrap()
}

/// A task directory with `repo/` holding `files`, and `task.json` holding `json`.
fn make_task(id: &str, json: &str, files: &[(&str, &str)]) -> Task {
    let dir = scratch("task-root").join(id);
    let repo = dir.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    for (path, body) in files {
        let p = repo.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }
    std::fs::write(dir.join("task.json"), json).unwrap();
    load_task(&dir).unwrap()
}

fn plan(mock: MockProvider) -> Plan {
    Plan {
        provider: Arc::new(mock) as Arc<dyn Provider>,
        provider_id: "mock".into(),
        model: "mock-model".into(),
        models: Vec::new(),
    }
}

/// The model's whole answer to `SOLVED_BY_FILE`: read, rewrite, finish.
fn solving_turns() -> Vec<Turn> {
    let mut turns = read_then_write("w1", "answer.txt", "42\n");
    turns.push(Turn::Events(script_text("Done.")));
    turns
}

/// The model reads a file, then rewrites it. The core refuses to overwrite a
/// file the agent has not read (`Write` is read-before-edit), so a scripted
/// overwrite needs the read first, as a real model would give.
fn read_then_write(id: &str, path: &str, content: &str) -> Vec<Turn> {
    vec![
        Turn::Events(script_tool_call(&format!("{id}r"), "Read", json!({ "path": path }))),
        Turn::Events(script_tool_call(
            &format!("{id}w"),
            "Write",
            json!({ "path": path, "content": content }),
        )),
    ]
}

async fn run(task: &Task, plan: &Plan) -> (TaskResult, Vec<String>) {
    let work_root = scratch("work-root");
    let opts = RunOptions { work_root: &work_root, keep_workdirs: false };
    let record = run_task(task, plan, &opts).await;
    // Every tree a run makes goes under `work_root`, and removes itself unless
    // asked to keep it; an empty root is the proof that cleanup happened.
    assert_eq!(std::fs::read_dir(&work_root).unwrap().count(), 0, "a run left its tree behind");
    (record.result, record.events)
}

const SOLVED_BY_FILE: &str = r#"{
    "prompt": "Write the answer to answer.txt",
    "check": { "command": "exit 0" },
    "expect": [ { "path": "answer.txt", "contains": "42" } ]
}"#;

#[tokio::test]
async fn a_solving_run_passes_and_the_fixture_is_never_touched() {
    let task = make_task("answer", SOLVED_BY_FILE, &[("answer.txt", "old\n")]);
    let mock = MockProvider::new(solving_turns());
    let (result, _) = run(&task, &plan(mock)).await;

    assert!(result.solved, "{:?}", result.failure);
    assert_eq!(result.failure, None);
    assert_eq!(result.stop_reason, "endTurn");
    assert_eq!(result.tool_calls.get("Write"), Some(&1));
    assert_eq!(result.tool_calls.get("Read"), Some(&1));
    assert_eq!(result.tool_calls_total, 2);
    assert_eq!(result.turns, 1);
    assert!(result.tokens.input > 0, "usage from the mock did not reach the result");
    assert_eq!(result.checks.expects.len(), 1);
    assert!(result.checks.expects[0].passed);

    // The fixture on disk is the starting tree, byte for byte.
    assert_eq!(std::fs::read_to_string(task.repo.join("answer.txt")).unwrap(), "old\n");
}

#[tokio::test]
async fn each_run_starts_from_the_fixture_not_from_the_last_run() {
    let task = make_task("isolated", SOLVED_BY_FILE, &[("answer.txt", "old\n")]);
    let solving = plan(MockProvider::new(solving_turns()));
    let (first, _) = run(&task, &solving).await;
    assert!(first.solved);

    // A second run that does nothing must not inherit the first run's 42.
    let idle = plan(MockProvider::new(vec![Turn::Events(script_text("Nothing to do."))]));
    let (second, _) = run(&task, &idle).await;
    assert!(!second.solved, "the second run saw the first run's tree");
}

#[tokio::test]
async fn an_agent_that_changes_nothing_is_not_solved() {
    let task = make_task("idle", SOLVED_BY_FILE, &[("answer.txt", "old\n")]);
    let mock = plan(MockProvider::new(vec![Turn::Events(script_text("I think it is 42."))]));
    let (result, _) = run(&task, &mock).await;

    assert!(!result.solved);
    assert_eq!(result.stop_reason, "endTurn");
    let failure = result.failure.expect("an unsolved run needs a reason");
    assert!(failure.starts_with("expect:"), "{failure}");
}

#[tokio::test]
async fn the_check_command_decides_when_it_fails() {
    // The file is right, but the check says no: the check has the last word.
    let task = make_task(
        "check-says-no",
        r#"{ "prompt": "p", "check": { "command": "exit 4" },
             "expect": [ { "path": "answer.txt", "contains": "42" } ] }"#,
        &[("answer.txt", "old\n")],
    );
    let mock = plan(MockProvider::new(solving_turns()));
    let (result, _) = run(&task, &mock).await;

    assert!(!result.solved);
    assert_eq!(result.failure.as_deref(), Some("check exited 4"));
    assert!(result.checks.expects[0].passed);
    assert_eq!(result.checks.command.as_ref().unwrap().exit_code, Some(4));
}

#[tokio::test]
async fn a_git_commit_is_refused_even_in_bypass() {
    // Bypass lets everything through except what §8 keeps behind an explicit
    // request. A commit nobody asked for must come back as a refusal, and the
    // run must count it.
    let task = make_task(
        "no-commit",
        r#"{ "prompt": "p", "check": { "command": "exit 0" } }"#,
        &[("a.txt", "x\n")],
    );
    let mock = plan(MockProvider::new(vec![
        Turn::Events(script_tool_call("b1", "Bash", json!({ "command": "git commit -m done" }))),
        Turn::Events(script_text("Could not commit.")),
    ]));
    let (result, _) = run(&task, &mock).await;

    assert_eq!(result.permissions_denied, 1);
    assert!(result.solved, "the check itself passes; only the commit was refused");
    assert_eq!(result.tool_calls.get("Bash"), Some(&1));
}

#[tokio::test]
async fn a_question_is_dismissed_so_the_turn_can_finish() {
    let task = make_task("asks", r#"{ "prompt": "p", "check": { "command": "exit 0" } }"#, &[]);
    let mock = plan(MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "q1",
            "AskUser",
            json!({ "question": "Which?", "options": [{ "label": "a" }, { "label": "b" }] }),
        )),
        Turn::Events(script_text("No answer, so I stopped.")),
    ]));
    let (result, _) = run(&task, &mock).await;

    assert_eq!(result.questions_dismissed, 1);
    assert_eq!(result.stop_reason, "endTurn");
}

#[tokio::test]
async fn a_run_over_its_timeout_is_cancelled_and_says_so() {
    let task = make_task(
        "slow",
        r#"{ "prompt": "p", "timeoutSecs": 1, "check": { "command": "exit 1" } }"#,
        &[],
    );
    // Six events at 400 ms each is well past the one-second timeout.
    let slow = MockProvider::new(vec![Turn::Events(script_text("This takes a while."))])
        .with_gap(Duration::from_millis(400));
    let (result, events) = run(&task, &plan(slow)).await;

    assert_eq!(result.stop_reason, "timeout");
    assert!(!result.solved);
    assert!(
        events.iter().any(|l| l.contains("\"type\":\"turnCancelled\"")),
        "the core did not confirm the cancel in the stream"
    );
}

#[tokio::test]
async fn the_result_and_the_event_log_have_the_documented_shape() {
    let task = make_task("shape", SOLVED_BY_FILE, &[("answer.txt", "old\n")]);
    let mock = plan(MockProvider::new(solving_turns()));
    let (result, events) = run(&task, &mock).await;

    let value = serde_json::to_value(&result).unwrap();
    for key in [
        "task", "solved", "provider", "model", "mode", "stopReason", "turns", "toolCalls",
        "toolCallsTotal", "tokens", "costUsd", "wallMs", "permissionsDenied",
        "questionsDismissed", "errors", "checks", "failure",
    ] {
        assert!(value.get(key).is_some(), "missing `{key}`");
    }
    assert_eq!(value["provider"], "mock");
    assert_eq!(value["model"], "mock-model");
    assert_eq!(value["mode"], "agent");

    assert!(events.iter().any(|l| l.contains("\"type\":\"turnFinished\"")));
    for line in &events {
        serde_json::from_str::<serde_json::Value>(line).expect("each event is one JSON object");
    }
}

#[tokio::test]
async fn a_plan_task_runs_in_plan_mode() {
    let task = make_task(
        "planned",
        r#"{ "prompt": "p", "mode": "plan", "check": { "command": "exit 0" } }"#,
        &[],
    );
    let mock = plan(MockProvider::new(vec![Turn::Events(script_text("Here is the plan."))]));
    let (result, _) = run(&task, &mock).await;
    assert_eq!(result.mode, "plan");
    assert!(result.solved);
}

#[test]
fn the_result_reads_back_as_a_failure_when_nothing_ran() {
    // A run that could not even copy its tree still produces a result, and the
    // result is not solved: an empty or broken run must never look green.
    let mut result = TaskResult::new("ghost", "mock", "m", "agent");
    result.failure = Some("cannot prepare the tree".into());
    result.judge();
    assert!(!result.solved);
    assert_eq!(result.stop_reason, "setupFailed");
}
