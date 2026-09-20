//! Commands end to end (§5.7, §6.5).
//!
//! Real processes, not a mock shell. A tool that lies about the operating
//! system is only caught by the operating system.

mod common;

use std::sync::Arc;
use std::time::Duration;

use yaca_core::event::AgentEvent;
use yaca_core::process::{ProcessId, ProcessState};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, PermissionAnswer, ScopeKey};
use serde_json::json;
use tokio::sync::broadcast::Receiver;

use common::{ask, calls, is_turn_over, session, until, until_within, TempProject};

/// The shell differs by platform, so the scripts the tests run do too. The same
/// two live in `tool/bash.rs`: a unit test and an integration test are separate
/// binaries, and Rust gives them no shared place below `tests/`.
fn two_bursts() -> &'static str {
    if cfg!(windows) {
        "echo one& ping -n 3 127.0.0.1 >nul& echo two"
    } else {
        "echo one; sleep 0.4; echo two"
    }
}

fn sleeps() -> &'static str {
    if cfg!(windows) {
        "ping -n 30 127.0.0.1 >nul"
    } else {
        "sleep 30"
    }
}

fn runs(id: &str, command: &str) -> Vec<yaca_core::provider::ProviderEvent> {
    calls(&[(
        id,
        "Bash",
        json!({ "command": command, "description": "Прогнал команду" }),
    )])
}

fn background(id: &str, command: &str) -> Vec<yaca_core::provider::ProviderEvent> {
    calls(&[(
        id,
        "Bash",
        json!({
            "command": command,
            "background": true,
            "description": "Поднял процесс",
        }),
    )])
}

/// Answers the first permission request with `answer`, then collects the rest
/// of the turn. Every test here goes through §5.7: commands are asked about in
/// every trust mode.
async fn answer_and_finish(
    handle: &yaca_core::SessionHandle,
    rx: &mut Receiver<AgentEvent>,
    answer: PermissionAnswer,
) -> Vec<AgentEvent> {
    let asked = until(rx, |e| matches!(e, AgentEvent::PermissionRequested { .. })).await;
    let AgentEvent::PermissionRequested { request } = asked.last().unwrap() else {
        unreachable!("until() only returns on a match");
    };
    handle
        .send(Command::AnswerPermission {
            request_id: request.id.clone(),
            answer,
        })
        .await
        .unwrap();

    let mut all = asked;
    all.extend(until_within(rx, Duration::from_secs(30), is_turn_over).await);
    all
}

fn progress_chunks(events: &[AgentEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ToolCallProgress { chunk, .. } => Some(chunk.clone()),
            _ => None,
        })
        .collect()
}

fn panel(events: &[AgentEvent]) -> Vec<yaca_core::ProcessSummary> {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            AgentEvent::ProcessesUpdated { processes } => Some(processes.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn output_reaches_the_feed_before_the_command_ends() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(runs("c1", two_bursts())),
        Turn::Events(script_text("готово")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "прогони команду").await;
    let events = answer_and_finish(&handle, &mut rx, PermissionAnswer::Allow).await;

    let chunks = progress_chunks(&events);
    let first = chunks
        .iter()
        .position(|c| c.contains("one"))
        .unwrap_or_else(|| panic!("«one» не доехало в ленту: {chunks:?}"));
    let second = chunks
        .iter()
        .position(|c| c.contains("two"))
        .unwrap_or_else(|| panic!("«two» не доехало в ленту: {chunks:?}"));

    // The whole point of the design. Handed over in one piece after the command
    // finished, `one` and `two` would sit in the same chunk — which is exactly
    // what this forbids.
    assert!(first < second, "вывод приехал одним куском: {chunks:?}");

    let names: Vec<_> = events.iter().map(AgentEvent::kind).collect();
    let finished = names
        .iter()
        .position(|n| *n == "toolCallFinished")
        .expect("тул не завершился");
    let last_progress = names
        .iter()
        .rposition(|n| *n == "toolCallProgress")
        .expect("вывода не было");
    assert!(last_progress < finished, "{names:?}");
}

#[tokio::test]
async fn a_command_is_asked_about_even_in_auto() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(runs("c1", "echo hi")),
        Turn::Events(script_text("готово")),
    ]));
    // §8: the Command row of the trust matrix says "спросить" in every column.
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "поздоровайся").await;
    let events = answer_and_finish(&handle, &mut rx, PermissionAnswer::Allow).await;

    let names: Vec<_> = events.iter().map(AgentEvent::kind).collect();
    assert!(names.contains(&"permissionRequested"), "{names:?}");
}

#[tokio::test]
async fn a_refused_command_never_runs() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(runs("c1", "echo x > marker.txt")),
        Turn::Events(script_text("понял")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "создай файл командой").await;
    let events = answer_and_finish(
        &handle,
        &mut rx,
        PermissionAnswer::Deny {
            reason: Some("не надо".into()),
        },
    )
    .await;

    let names: Vec<_> = events.iter().map(AgentEvent::kind).collect();
    assert!(names.contains(&"toolCallDenied"), "{names:?}");
    // The question is asked before anything happens (§5.7), so nothing did.
    assert!(
        !dir.path().join("marker.txt").exists(),
        "отклонённая команда всё-таки выполнилась"
    );
}

#[tokio::test]
async fn a_prefix_rule_covers_the_next_command_with_the_same_first_word() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(runs("c1", "echo первый")),
        Turn::Events(runs("c2", "echo второй")),
        Turn::Events(script_text("готово")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "прогони обе").await;
    let events = answer_and_finish(
        &handle,
        &mut rx,
        PermissionAnswer::AllowAlways {
            scope: ScopeKey::CommandPrefix {
                prefix: "echo".into(),
            },
        },
    )
    .await;

    let asked = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::PermissionRequested { .. }))
        .count();
    assert_eq!(asked, 1, "второй `echo` не должен был спрашивать заново");
    assert!(progress_chunks(&events)
        .iter()
        .any(|c| c.contains("второй")));
}

#[tokio::test]
async fn a_non_zero_exit_is_a_failed_action() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(runs("c1", "echo сломалось& exit 3")),
        Turn::Events(script_text("вижу")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "прогони тесты").await;
    let events = answer_and_finish(&handle, &mut rx, PermissionAnswer::Allow).await;

    let failure = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::ToolCallFailed { message, .. } => Some(message.clone()),
            _ => None,
        })
        .expect("ненулевой код должен давать упавшее действие (§5.4)");
    // §5.5: the feed gets a line. The output already streamed into the row.
    assert!(failure.contains("кодом 3"), "{failure}");
    assert!(progress_chunks(&events)
        .iter()
        .any(|c| c.contains("сломалось")));
}

#[tokio::test]
async fn a_background_command_shows_up_in_the_panel_and_stops_from_it() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(background("c1", sleeps())),
        Turn::Events(script_text("поднял")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "подними сервер").await;
    let events = answer_and_finish(&handle, &mut rx, PermissionAnswer::Allow).await;

    let rows = panel(&events);
    assert_eq!(rows.len(), 1, "процесс не попал в панель §6.5");
    assert_eq!(rows[0].state, ProcessState::Running);
    assert_eq!(rows[0].cwd, ".");
    assert!(rows[0].finished_at.is_none());
    let id = rows[0].id;

    handle.send(Command::StopProcess { id }).await.unwrap();
    let after = until_within(&mut rx, Duration::from_secs(20), |e| {
        matches!(e, AgentEvent::ProcessesUpdated { processes }
            if processes.iter().all(|p| p.state != ProcessState::Running))
    })
    .await;

    let rows = panel(&after);
    assert_eq!(rows[0].state, ProcessState::Stopped);
    assert!(rows[0].finished_at.is_some(), "время работы не закрылось");
}

#[tokio::test]
async fn background_output_waits_for_the_row_to_be_selected() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(background("c1", two_bursts())),
        Turn::Events(script_text("поднял")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "подними сервер").await;
    let events = answer_and_finish(&handle, &mut rx, PermissionAnswer::Allow).await;
    let id = panel(&events)[0].id;

    // Nobody is looking at the row yet, so nothing may reach the feed: a
    // chatty process would push real events out of the broadcast channel.
    let quiet = until_within(&mut rx, Duration::from_secs(20), |e| {
        matches!(e, AgentEvent::ProcessesUpdated { processes }
            if processes.iter().all(|p| p.state != ProcessState::Running))
    })
    .await;
    let leaked: Vec<_> = quiet
        .iter()
        .map(AgentEvent::kind)
        .filter(|n| n.starts_with("processOutput"))
        .collect();
    assert!(
        leaked.is_empty(),
        "вывод утёк без выбора строки: {leaked:?}"
    );

    // Selecting it hands over everything collected while nobody looked.
    handle
        .send(Command::WatchProcess { id: Some(id) })
        .await
        .unwrap();
    let shown = until(&mut rx, |e| {
        matches!(e, AgentEvent::ProcessOutputReset { .. })
    })
    .await;

    let AgentEvent::ProcessOutputReset {
        id: shown_id,
        text,
        dropped,
    } = shown.last().unwrap()
    else {
        unreachable!("until() only returns on a match");
    };
    assert_eq!(*shown_id, id);
    assert_eq!(*dropped, 0);
    assert!(
        text.contains("one") && text.contains("two"),
        "получили: {text}"
    );
}

#[tokio::test]
async fn watching_a_process_that_never_existed_says_nothing() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::text("ничего не делаю"));
    let handle = session(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    // Nothing to show and nothing to crash: an id from a stale panel is a
    // plausible thing for the interface to send after a restart.
    handle
        .send(Command::WatchProcess {
            id: Some(ProcessId(42)),
        })
        .await
        .unwrap();

    ask(&handle, "привет").await;
    let events = until(&mut rx, is_turn_over).await;
    let names: Vec<_> = events.iter().map(AgentEvent::kind).collect();
    assert!(
        !names.iter().any(|n| n.starts_with("processOutput")),
        "{names:?}"
    );
}
