//! Shell directories are independent of file-tool roots, including on restart.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::process::{ProcessId, ProcessState};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, PermissionAnswer};

use common::{ask, calls, is_turn_over, session, until, TempProject};

#[tokio::test]
async fn a_command_can_run_outside_the_project_after_permission() {
    let project = TempProject::new();
    let outside = TempProject::new();
    outside.write("tools folder/input.txt", "external input\n");
    let cwd = std::fs::canonicalize(outside.path().join("tools folder")).unwrap();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "c1",
            "Bash",
            json!({
                "command": "cd .; cat input.txt",
                "cwd": cwd,
                "description": "Read external input",
            }),
        )])),
        Turn::Events(script_text("done")),
    ]));
    let handle = session(&project, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "read the input from the tools directory").await;
    let asked = until(&mut rx, |e| {
        matches!(e, AgentEvent::PermissionRequested { .. })
    })
    .await;
    let request = asked
        .iter()
        .find_map(|e| match e {
            AgentEvent::PermissionRequested { request } => Some(request),
            _ => None,
        })
        .unwrap();
    assert_eq!(request.note.as_deref(), cwd.to_str());
    handle
        .send(Command::AnswerPermission {
            request_id: request.id.clone(),
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();

    let events = until(&mut rx, is_turn_over).await;
    assert!(
        events.iter().any(|e| matches!(e,
            AgentEvent::ToolCallProgress { chunk, .. } if chunk.contains("external input")
        )),
        "{events:?}"
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolCallFinished { .. })));
    assert!(project.path().join("input.txt").symlink_metadata().is_err());
}

#[tokio::test]
async fn background_output_and_restart_work_outside_the_project() {
    let project = TempProject::new();
    let outside = TempProject::new();
    outside.write("tools folder/input.txt", "external background output\n");
    let cwd = std::fs::canonicalize(outside.path().join("tools folder")).unwrap();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "c1",
            "Bash",
            json!({
                "command": "cat input.txt", "cwd": cwd, "background": true,
                "description": "Started external command",
            }),
        )])),
        Turn::Events(calls(&[(
            "c2",
            "BashWait",
            json!({ "id": 1, "timeout": 10, "description": "Waited for output" }),
        )])),
        Turn::Events(calls(&[(
            "c3",
            "BashRestart",
            json!({ "id": 1, "description": "Restarted external command" }),
        )])),
        Turn::Events(calls(&[(
            "c4",
            "BashWait",
            json!({ "id": 1, "timeout": 10, "description": "Waited for restart" }),
        )])),
        Turn::Events(script_text("done")),
    ]));
    let handle = session(&project, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "run and restart the external command").await;
    let events = until(&mut rx, is_turn_over).await;
    assert!(
        !events.iter().any(|e| matches!(
            e,
            AgentEvent::ToolCallFailed { .. } | AgentEvent::ToolCallRejected { .. }
        )),
        "{events:?}"
    );
    let rows = events
        .iter()
        .rev()
        .find_map(|e| match e {
            AgentEvent::ProcessesUpdated { processes } => Some(processes),
            _ => None,
        })
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, ProcessState::Exited { code: Some(0) });
    assert_eq!(
        rows[0].cwd,
        yaca_core::process::display_cwd(project.path(), &cwd)
    );
    assert!(!rows[0].cwd.starts_with("//?/"));

    handle
        .send(Command::WatchProcess {
            id: Some(ProcessId(1)),
        })
        .await
        .unwrap();
    let shown = until(&mut rx, |e| {
        matches!(e, AgentEvent::ProcessOutputReset { .. })
    })
    .await;
    let output = shown
        .iter()
        .find_map(|e| match e {
            AgentEvent::ProcessOutputReset { text, .. } => Some(text),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        output.matches("external background output").count(),
        2,
        "{output}"
    );
}
