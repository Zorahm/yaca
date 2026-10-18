//! A window that lost its feed while the session kept running (a reload).
//!
//! The session is the same one throughout: nothing here restarts it, and the
//! question under test is only whether a subscriber arriving late can be put
//! where the stream is — everything before it replayed, everything after it
//! delivered, nothing twice.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::{BlockKind, ProviderEvent};
use yaca_core::usage::TokenCounts;
use yaca_core::{Command, Mode, PermissionAnswer, Replay, SessionConfig, SessionHandle, StopReason};

use common::{ask, calls, is_turn_over, until, until_within, TempProject};

fn recorded(dir: &TempProject, provider: Arc<MockProvider>, mode: Mode) -> Arc<SessionHandle> {
    let mut config = SessionConfig::new(dir.path());
    config.mode = mode;
    config.journal = Some(dir.chats());
    Arc::new(yaca_core::spawn(config, provider).unwrap())
}

/// One answer made of `count` deltas, each a numbered word.
fn long_answer(count: usize) -> Vec<ProviderEvent> {
    let mut script = vec![
        ProviderEvent::MessageStart {
            model: "mock".into(),
            usage: TokenCounts::default(),
        },
        ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::Text,
        },
    ];
    for i in 0..count {
        script.push(ProviderEvent::TextDelta {
            index: 0,
            text: format!("w{i} "),
        });
    }
    script.push(ProviderEvent::BlockStop { index: 0 });
    script.push(ProviderEvent::MessageDelta {
        stop_reason: Some(StopReason::EndTurn),
        usage: TokenCounts::default(),
    });
    script.push(ProviderEvent::MessageStop);
    script
}

fn text_of(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

async fn replayed(handle: &Arc<SessionHandle>, at: Replay) -> Vec<AgentEvent> {
    let handle = handle.clone();
    // The journal is read from disk; the shell does the same off the runtime.
    tokio::task::spawn_blocking(move || handle.replay(&at))
        .await
        .unwrap()
}

/// The point of the whole design: however the attach lands among the deltas of
/// a stream, what was replayed plus what is delivered afterwards is the stream,
/// whole and once.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_attach_in_the_middle_of_a_stream_misses_nothing_and_repeats_nothing() {
    const WORDS: usize = 3_000;
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(long_answer(WORDS))]));
    let handle = recorded(&dir, provider, Mode::Auto);
    let expected: String = (0..WORDS).map(|i| format!("w{i} ")).collect();

    // Attaching is cheap, so attach again and again for as long as the turn
    // runs: some of them are bound to land among the deltas. Reading what came
    // before each is done afterwards, which changes nothing — the journal only
    // grows, and each attach carries the length it had.
    let mut stream = handle.subscribe();
    let over = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = over.clone();
    tokio::spawn(async move {
        while let Ok(event) = stream.recv().await {
            if is_turn_over(&event) {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                break;
            }
        }
    });

    ask(&handle, "talk").await;
    let mut attached = Vec::new();
    while !over.load(std::sync::atomic::Ordering::SeqCst) && attached.len() < 20_000 {
        attached.push(handle.attach());
        tokio::task::yield_now().await;
    }
    // A few of them, spread over the run: all of them would be a lot of file.
    let step = (attached.len() / 40).max(1);

    let mut watchers = Vec::new();
    for yaca_core::Attach { mut events, replay } in attached.into_iter().step_by(step) {
        let before = replayed(&handle, replay).await;
        watchers.push(tokio::spawn(async move {
            let mut after = Vec::new();
            while let Ok(event) = events.recv().await {
                let done = is_turn_over(&event);
                after.push(event);
                if done {
                    break;
                }
            }
            (text_of(&before), text_of(&after))
        }));
    }

    let mut landed_inside = 0;
    for watcher in watchers {
        let (before, after) = tokio::time::timeout(Duration::from_secs(20), watcher)
            .await
            .expect("the turn ends")
            .unwrap();
        // A late attach finds the turn over and gets it all from the journal,
        // and that is as right as getting it in two halves.
        if !before.is_empty() && !after.is_empty() {
            landed_inside += 1;
        }
        assert_eq!(format!("{before}{after}"), expected);
    }
    assert!(
        landed_inside > 0,
        "no attach landed inside the stream, so nothing was tested"
    );
}

/// A reload while the agent waits for a yes: the question is still open and
/// still answerable, and answering it reaches the new subscriber.
#[tokio::test]
async fn a_pending_permission_is_still_pending_after_an_attach() {
    let dir = TempProject::new();
    // A command that changes a file: one that only reads is not asked about.
    let command = if cfg!(windows) {
        "Set-Content -Path out.txt -Value hi"
    } else {
        "echo hi > out.txt"
    };
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "Bash",
            json!({ "command": command, "description": "Say hi" }),
        )])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = recorded(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "run it").await;
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::PermissionRequested { .. })).await;
    let request = match seen.last() {
        Some(AgentEvent::PermissionRequested { request }) => request.clone(),
        other => panic!("no request: {other:?}"),
    };

    let yaca_core::Attach { events, replay } = handle.attach();
    let before = replayed(&handle, replay).await;

    assert!(
        before.iter().any(|e| matches!(
            e,
            AgentEvent::PermissionRequested { request: r } if r.id == request.id
        )),
        "the question is in what the new window folds"
    );
    assert!(
        !before
            .iter()
            .any(|e| matches!(e, AgentEvent::PermissionResolved { .. })),
        "and it is not closed, as a replay of a dead run would close it"
    );
    assert!(
        !before.iter().any(|e| matches!(e, AgentEvent::TurnCancelled { .. })),
        "the turn is running, not cancelled"
    );

    handle
        .send(Command::AnswerPermission {
            request_id: request.id.clone(),
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();

    let mut events = events;
    let after = until_within(&mut events, Duration::from_secs(30), is_turn_over).await;
    assert!(after.iter().any(|e| matches!(
        e,
        AgentEvent::PermissionResolved { request_id } if *request_id == request.id
    )));
    assert!(
        !after
            .iter()
            .any(|e| matches!(e, AgentEvent::PermissionRequested { .. })),
        "the request is not delivered a second time"
    );
}

/// What is never written down is asked of the live session.
#[tokio::test]
async fn the_state_that_is_not_journaled_is_taken_from_the_live_session() {
    let dir = TempProject::new();
    dir.write("a.rs", "one\n");
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("Hi."))]));
    let handle = recorded(&dir, provider, Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "hello").await;
    until(&mut rx, is_turn_over).await;

    let events = replayed(&handle, handle.attach().replay).await;

    let kinds: Vec<_> = events.iter().rev().take(6).map(|e| e.kind()).collect();
    assert_eq!(
        kinds,
        vec![
            "checkpointsUpdated",
            "queueUpdated",
            "processesUpdated",
            "lspUpdated",
            "mcpUpdated",
            "stateChanged",
        ],
        "the tail is the live state, in a fixed order"
    );
    match events.last() {
        Some(AgentEvent::CheckpointsUpdated { checkpoints }) => {
            assert!(
                !checkpoints.is_empty(),
                "the point taken before the message is on the panel's list"
            );
        }
        other => panic!("{other:?}"),
    }
}

/// A one-shot run writes nothing down; attaching to it must not be an error.
#[tokio::test]
async fn a_session_without_a_journal_attaches_with_only_its_live_state() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![]));
    let handle = Arc::new(common::session(&dir, provider, Mode::Manual));

    let events = replayed(&handle, handle.attach().replay).await;

    assert_eq!(events.len(), 6);
    assert!(matches!(events[0], AgentEvent::StateChanged { .. }));
}
