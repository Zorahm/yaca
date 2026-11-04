//! Background commands across active work, idle turns, and background subagents.
//! File gates keep process completion deterministic instead of racing shell startup.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::broadcast::Receiver;
use yaca_core::event::{AgentEvent, StopReason};
use yaca_core::message::ContentBlock;
use yaca_core::process::ProcessState;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::ProviderEvent;
use yaca_core::{Command, Mode, PermissionAnswer, SessionHandle};

use common::{ask, calls, is_turn_over, session, tool_call, until_within, TempProject};

const LIMIT: Duration = Duration::from_secs(30);

fn gated_command() -> &'static str {
    if cfg!(windows) {
        "while (!(Test-Path release)) { Start-Sleep -Milliseconds 25 }; echo background_result; exit 0"
    } else {
        "while [ ! -f release ]; do sleep 0.05; done; echo background_result; exit 0"
    }
}

fn start_background() -> Turn {
    Turn::Events(calls(&[(
        "bg",
        "Bash",
        json!({ "command": gated_command(), "background": true, "description": "Started background job" }),
    )]))
}

async fn allow(handle: &SessionHandle, events: &[AgentEvent], call: &str) {
    let request = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::PermissionRequested { request } if request.call_id.0 == call => {
                Some(request)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no permission request for {call}"));
    handle
        .send(Command::AnswerPermission {
            request_id: request.id.clone(),
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
}

async fn finish_allowing(handle: &SessionHandle, rx: &mut Receiver<AgentEvent>) -> Vec<AgentEvent> {
    let mut all = Vec::new();
    loop {
        let batch = until_within(rx, LIMIT, |event| {
            matches!(event, AgentEvent::PermissionRequested { .. }) || is_turn_over(event)
        })
        .await;
        let over = batch.last().is_some_and(is_turn_over);
        if let Some(AgentEvent::PermissionRequested { request }) = batch.last() {
            allow(handle, &batch, &request.call_id.0).await;
        }
        all.extend(batch);
        if over {
            assert!(
                !all.iter().any(|event| matches!(
                    event,
                    AgentEvent::ToolCallFailed { .. } | AgentEvent::Error { .. }
                )),
                "{all:?}"
            );
            return all;
        }
    }
}

async fn idle(handle: &SessionHandle, rx: &mut Receiver<AgentEvent>) {
    if handle.state().busy {
        until_within(
            rx,
            LIMIT,
            |event| matches!(event, AgentEvent::StateChanged { state } if !state.busy),
        )
        .await;
    }
}

fn request_text(provider: &MockProvider, index: usize) -> String {
    provider.seen.lock().unwrap()[index]
        .messages
        .last()
        .unwrap()
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn asked_times(provider: &MockProvider, count: usize) {
    tokio::time::timeout(LIMIT, async {
        while provider.seen.lock().unwrap().len() < count {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("model did not receive the expected request");
}

fn exited(event: &AgentEvent) -> bool {
    matches!(event, AgentEvent::ProcessesUpdated { processes }
        if processes.iter().any(|process| process.state == ProcessState::Exited { code: Some(0) }))
}

#[tokio::test]
async fn the_model_narrates_edits_answers_the_user_and_is_woken_after_going_idle() {
    let dir = TempProject::new();
    dir.write("input.txt", "work while the command runs\n");
    let mut narrated_read =
        script_text("The command is running; I will inspect the input meanwhile.");
    narrated_read.truncate(narrated_read.len() - 2);
    narrated_read.extend(tool_call(1, "read", "Read", json!({ "path": "input.txt" })));
    narrated_read.push(ProviderEvent::MessageDelta {
        stop_reason: Some(StopReason::ToolUse),
        usage: Default::default(),
    });
    narrated_read.push(ProviderEvent::MessageStop);
    let provider = Arc::new(MockProvider::new(vec![
        start_background(),
        Turn::Events(narrated_read),
        Turn::Events(calls(&[(
            "write",
            "Write",
            json!({ "path": "work.txt", "content": "independent work\n" }),
        )])),
        Turn::Events(script_text(
            "My work is done; the background command is still running.",
        )),
        Turn::Events(script_text("I can answer another question while it runs.")),
        Turn::Events(script_text("The background command has now completed.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "Start the job and keep working.").await;
    let first = finish_allowing(&handle, &mut rx).await;
    idle(&handle, &mut rx).await;
    assert_eq!(
        std::fs::read_to_string(dir.path().join("work.txt")).unwrap(),
        "independent work\n"
    );
    assert!(first
        .iter()
        .any(|event| matches!(event, AgentEvent::TextDelta { text, .. }
        if text.contains("inspect the input meanwhile"))));
    assert!(
        !first.iter().any(exited),
        "the file gate must still hold the process"
    );
    assert_eq!(provider.seen.lock().unwrap().len(), 4);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        provider.seen.lock().unwrap().len(),
        4,
        "idle must not poll the model"
    );

    ask(&handle, "Can you answer while the command runs?").await;
    finish_allowing(&handle, &mut rx).await;
    idle(&handle, &mut rx).await;
    assert_eq!(provider.seen.lock().unwrap().len(), 5);
    assert!(!dir.path().join("release").exists());

    dir.write("release", "ready");
    let wake = finish_allowing(&handle, &mut rx).await;
    idle(&handle, &mut rx).await;
    assert_eq!(provider.seen.lock().unwrap().len(), 6);
    assert!(wake.iter().any(exited));
    assert!(wake
        .iter()
        .any(|event| matches!(event, AgentEvent::TurnStarted { .. })));
    assert!(!wake
        .iter()
        .any(|event| matches!(event, AgentEvent::UserMessage { .. })));
    let note = request_text(&provider, 5);
    assert!(
        note.contains("exited successfully") && note.contains("background_result"),
        "{note}"
    );
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        provider.seen.lock().unwrap().len(),
        6,
        "completion must be delivered once"
    );
}

#[tokio::test]
async fn completion_during_another_tool_is_delivered_at_the_next_model_step() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        start_background(),
        Turn::Events(calls(&[(
            "write",
            "Write",
            json!({ "path": "work.txt", "content": "done\n" }),
        )])),
        Turn::Events(script_text(
            "I received both the write result and background result.",
        )),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "Start the job and write a file.").await;
    let start = until_within(&mut rx, LIMIT, |event| {
        matches!(event,
            AgentEvent::PermissionRequested { request } if request.call_id.0 == "bg"
        )
    })
    .await;
    allow(&handle, &start, "bg").await;
    let writing = until_within(&mut rx, LIMIT, |event| {
        matches!(event,
            AgentEvent::PermissionRequested { request } if request.call_id.0 == "write"
        )
    })
    .await;
    dir.write("release", "ready");
    until_within(&mut rx, LIMIT, exited).await;
    assert_eq!(
        provider.seen.lock().unwrap().len(),
        2,
        "an active tool must not be interrupted by a second model request"
    );
    allow(&handle, &writing, "write").await;
    let rest = finish_allowing(&handle, &mut rx).await;
    assert!(!rest
        .iter()
        .any(|event| matches!(event, AgentEvent::TurnStarted { .. })));
    assert_eq!(provider.seen.lock().unwrap().len(), 3);
    let note = request_text(&provider, 2);
    assert!(note.contains("background_result"), "{note}");
    assert!(provider.seen.lock().unwrap()[2].messages.last().unwrap().content.iter().any(|block| {
        matches!(block, ContentBlock::ToolResult { tool_use_id, is_error: false, .. } if tool_use_id == "write")
    }));
}

#[tokio::test]
async fn a_command_wakes_the_model_while_a_background_task_is_still_working() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        start_background(),
        Turn::Events(calls(&[
            (
                "task",
                "Task",
                json!({ "prompt": "Create child.txt.", "background": true }),
            ),
            (
                "parent",
                "Write",
                json!({ "path": "parent.txt", "content": "parent\n" }),
            ),
        ])),
        Turn::Events(calls(&[(
            "child",
            "Write",
            json!({ "path": "child.txt", "content": "child\n" }),
        )])),
        Turn::Events(script_text("Waiting for the task and the command.")),
        Turn::Events(script_text(
            "The command finished; the task is still running.",
        )),
        Turn::Events(script_text("Created child.txt.")),
        Turn::Events(script_text("Both are finished.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "Start a command and delegate another task.").await;
    let start = until_within(&mut rx, LIMIT, |event| {
        matches!(event,
            AgentEvent::PermissionRequested { request } if request.call_id.0 == "bg"
        )
    })
    .await;
    allow(&handle, &start, "bg").await;
    let mut waiting = Vec::new();
    while !(waiting.iter().any(|event| {
        matches!(event,
            AgentEvent::PermissionRequested { request } if request.call_id.0 == "parent"
        )
    }) && waiting.iter().any(|event| {
        matches!(event,
            AgentEvent::ToolCallStarted { call_id, .. } if call_id.0 == "child"
        )
    })) {
        waiting.extend(
            until_within(&mut rx, LIMIT, |event| {
                matches!(
                    event,
                    AgentEvent::PermissionRequested { .. } | AgentEvent::ToolCallStarted { .. }
                )
            })
            .await,
        );
    }
    allow(&handle, &waiting, "parent").await;
    // The permission broker presents one question at a time, even across agents.
    waiting.extend(
        until_within(&mut rx, LIMIT, |event| {
            matches!(event,
                AgentEvent::PermissionRequested { request } if request.call_id.0 == "child"
            )
        })
        .await,
    );
    asked_times(&provider, 4).await;
    dir.write("release", "ready");
    asked_times(&provider, 5).await;
    let note = request_text(&provider, 4);
    assert!(note.contains("background_result"), "{note}");
    assert!(!dir.path().join("child.txt").exists());
    let mut delivered = Vec::new();
    while let Ok(event) = rx.try_recv() {
        delivered.push(event);
    }
    assert!(
        !delivered.iter().any(is_turn_over),
        "the pending task must keep the turn open"
    );
    assert!(!delivered.iter().any(|event| matches!(event,
        AgentEvent::ToolCallFinished { call_id, .. } if call_id.0 == "task"
    )));

    allow(&handle, &waiting, "child").await;
    finish_allowing(&handle, &mut rx).await;
    assert_eq!(provider.seen.lock().unwrap().len(), 7);
    assert!(request_text(&provider, 6).contains("Created child.txt."));
    assert!(dir.path().join("parent.txt").exists() && dir.path().join("child.txt").exists());
}

#[tokio::test]
async fn explicit_wait_reads_completion_without_a_second_automatic_wakeup() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        start_background(),
        Turn::Events(calls(&[(
            "wait",
            "BashWait",
            json!({ "id": 1, "timeout": 20, "description": "Waited for the job" }),
        )])),
        Turn::Events(script_text("I read the completion through BashWait.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "Start and wait for the job.").await;
    let start = until_within(&mut rx, LIMIT, |event| {
        matches!(event,
            AgentEvent::PermissionRequested { request } if request.call_id.0 == "bg"
        )
    })
    .await;
    allow(&handle, &start, "bg").await;
    until_within(&mut rx, LIMIT, |event| {
        matches!(event,
            AgentEvent::ToolCallStarted { call_id, .. } if call_id.0 == "wait"
        )
    })
    .await;
    dir.write("release", "ready");
    finish_allowing(&handle, &mut rx).await;
    idle(&handle, &mut rx).await;
    let (output, error) = common::tool_result_of(&provider, "wait");
    assert!(
        !error && output.contains("background_result") && output.contains("completed successfully"),
        "{output}"
    );
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        provider.seen.lock().unwrap().len(),
        3,
        "explicitly read completion must not wake the model again"
    );
}

#[tokio::test]
async fn a_command_moved_to_background_by_timeout_can_wake_an_idle_model() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "bg",
            "Bash",
            json!({
                "command": gated_command(), "timeout_ms": 100, "description": "Started long job",
            }),
        )])),
        Turn::Events(script_text("The job moved to background; I can stop here.")),
        Turn::Events(script_text("The background job completed.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "Run the job with a short foreground timeout.").await;
    finish_allowing(&handle, &mut rx).await;
    idle(&handle, &mut rx).await;
    assert_eq!(provider.seen.lock().unwrap().len(), 2);
    let (output, error) = common::tool_result_of(&provider, "bg");
    assert!(!error && output.contains("NOT stopped"), "{output}");
    dir.write("release", "ready");
    finish_allowing(&handle, &mut rx).await;
    let note = request_text(&provider, 2);
    assert!(
        note.contains("background_result") && note.contains("whole output is in the file"),
        "{note}"
    );
}
