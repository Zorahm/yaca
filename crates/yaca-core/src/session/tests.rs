//! Golden tests of the loop.
//!
//! They check exactly the one main thing: which sequence of `AgentEvent`
//! comes out. The interface is a pure function of this stream, so if
//! the stream keeps its shape, the feed does not break; if the shape shifts, the test
//! must fail before a person sees it.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::broadcast::Receiver;

use super::*;
use crate::error::ProviderError;
use crate::event::StopReason;
use crate::provider::mock::{script_text, script_tool_call, MockProvider, Turn};
use crate::testutil::TempDir;

/// The event's name — what the interface tells branches apart by.
fn names(events: &[AgentEvent]) -> Vec<&'static str> {
    events.iter().map(AgentEvent::kind).collect()
}

async fn collect(rx: &mut Receiver<AgentEvent>) -> Vec<AgentEvent> {
    let mut out = Vec::new();
    let deadline = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let done = matches!(
                        ev,
                        AgentEvent::TurnFinished { .. } | AgentEvent::TurnCancelled { .. }
                    );
                    out.push(ev);
                    if done {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    })
    .await;
    assert!(
        deadline.is_ok(),
        "turn did not finish within 5 s: {:?}",
        names(&out)
    );
    out
}

fn session(dir: &TempDir, provider: MockProvider, mode: Mode) -> SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = mode;
    cfg.journal = None;
    cfg.model = "claude-opus-5".into();
    spawn(cfg, Arc::new(provider)).unwrap()
}

async fn ask(handle: &SessionHandle, text: &str) {
    handle
        .send(Command::SendMessage {
            text: text.into(),
            attachments: vec![],
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn text_response_yields_exact_expected_stream() {
    let dir = TempDir::new();
    let handle = session(&dir, MockProvider::text("This is in Rust."), Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "what language is core in?").await;
    let events = collect(&mut rx).await;

    assert_eq!(
        names(&events),
        vec![
            "stateChanged", // the ring started spinning
            // §5.1: the stream draws the human's message card, not the
            // interface, and it draws it first — the snapshot below can take
            // seconds.
            "userMessage",
            // §5.8: the point is taken before the turn touches anything —
            // that is what `Esc Esc` returns to (§13).
            "checkpointCreating",
            "checkpointCreationFinished",
            "checkpointCreated",
            "checkpointsUpdated",
            "turnStarted",
            "textDelta",
            "usageUpdated",
            "turnFinished",
        ]
    );

    match &events[7] {
        AgentEvent::TextDelta { text, .. } => assert_eq!(text, "This is in Rust."),
        other => panic!("not a delta: {other:?}"),
    }
    match &events[9] {
        AgentEvent::TurnFinished { stop_reason, .. } => {
            assert_eq!(*stop_reason, StopReason::EndTurn)
        }
        other => panic!("not a finish: {other:?}"),
    }
}

#[tokio::test]
async fn chat_profile_does_not_create_message_checkpoints() {
    let dir = TempDir::new();
    let mut cfg = SessionConfig::new(dir.path());
    cfg.profile = Profile::Chat;
    let handle = spawn(cfg, Arc::new(MockProvider::text("Hello"))).unwrap();
    let mut rx = handle.subscribe();

    ask(&handle, "hello").await;
    let events = collect(&mut rx).await;
    assert!(!events.iter().any(|event| matches!(
        event,
        AgentEvent::CheckpointCreating { .. } | AgentEvent::CheckpointCreated { .. }
    )));
}

#[tokio::test]
async fn chat_workspace_survives_resume_and_is_removed_on_delete() {
    let project = TempDir::new();
    let journals = TempDir::new();
    let mut cfg = SessionConfig::new(project.path());
    cfg.journal = Some(journals.path().to_path_buf());
    cfg.profile = Profile::Chat;
    let handle = spawn(cfg, Arc::new(MockProvider::text("Hello"))).unwrap();
    let id = handle.id().clone();
    let store = SessionStore::open(journals.path(), project.path());
    let workspace = store.chat_workspace(&id);
    assert!(workspace.is_dir());
    assert!(handle
        .shared
        .ws
        .resolve_new(".yaca/drafts/test.md")
        .unwrap()
        .starts_with(&workspace));
    std::fs::write(workspace.join("document.txt"), "kept").unwrap();
    let duplicate = store.duplicate(&id, None).unwrap();
    let duplicate_workspace = store.chat_workspace(&duplicate.id);
    assert_eq!(
        std::fs::read_to_string(duplicate_workspace.join("document.txt")).unwrap(),
        "kept"
    );
    store.delete(&duplicate.id).unwrap();
    drop(handle);

    let mut cfg = SessionConfig::new(project.path());
    cfg.journal = Some(journals.path().to_path_buf());
    cfg.resume_session(id.clone());
    let resumed = spawn(cfg, Arc::new(MockProvider::text("Hello"))).unwrap();
    assert_eq!(resumed.state().profile, Profile::Chat);
    assert_eq!(
        std::fs::read_to_string(workspace.join("document.txt")).unwrap(),
        "kept"
    );
    drop(resumed);
    store.delete(&id).unwrap();
    assert!(!workspace.exists());
}

#[tokio::test]
async fn chat_copies_external_file_before_reading_it() {
    let project = TempDir::new();
    let journals = TempDir::new();
    let external = TempDir::new();
    let file = external.write("outside.txt", "original content");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "tool1",
            "Read",
            json!({"path": file.to_string_lossy()}),
        )),
        Turn::Events(script_text("Done")),
    ]));
    let mut cfg = SessionConfig::new(project.path());
    cfg.journal = Some(journals.path().to_path_buf());
    cfg.profile = Profile::Chat;
    let handle = spawn(cfg, provider.clone()).unwrap();
    let mut rx = handle.subscribe();
    ask(&handle, "Read the file").await;
    let events = collect(&mut rx).await;
    assert!(events.iter().any(|event| matches!(event, AgentEvent::ToolCallProgress { chunk, .. } if chunk.contains("original content"))));
    let workspace = SessionStore::open(journals.path(), project.path()).chat_workspace(handle.id());
    assert_eq!(
        std::fs::read_to_string(workspace.join("attachments/outside.txt")).unwrap(),
        "original content"
    );
    assert_eq!(std::fs::read_to_string(file).unwrap(), "original content");
    assert!(format!("{:?}", provider.seen.lock().unwrap().last()).contains("outside the chat"));
}

#[tokio::test]
async fn chat_copies_user_attachments() {
    let project = TempDir::new();
    let journals = TempDir::new();
    let external = TempDir::new();
    let file = external.write("report.txt", "report");
    let mut cfg = SessionConfig::new(project.path());
    cfg.journal = Some(journals.path().to_path_buf());
    cfg.profile = Profile::Chat;
    let handle = spawn(cfg, Arc::new(MockProvider::text("Done"))).unwrap();
    let mut rx = handle.subscribe();
    handle
        .send(Command::SendMessage {
            text: "Review this".into(),
            attachments: vec![file.to_string_lossy().into_owned()],
        })
        .await
        .unwrap();
    let events = collect(&mut rx).await;
    let store = SessionStore::open(journals.path(), project.path());
    let workspace = store.chat_workspace(handle.id());
    assert_eq!(
        std::fs::read_to_string(workspace.join("attachments/report.txt")).unwrap(),
        "report"
    );
    assert!(events.iter().any(
        |event| matches!(event, AgentEvent::UserMessage { attachments, .. }
        if attachments.iter().any(|path| std::path::Path::new(path).starts_with(&workspace)))
    ));
    assert!(file.exists());
    let duplicate = store.duplicate(handle.id(), None).unwrap();
    let duplicate_workspace = store.chat_workspace(&duplicate.id);
    assert!(store
        .events(&duplicate.id)
        .iter()
        .any(|event| matches!(event,
        AgentEvent::UserMessage { attachments, .. } if attachments.iter().any(|path|
            std::path::Path::new(path).starts_with(&duplicate_workspace)))));
}

#[tokio::test]
async fn tool_call_completes_roundtrip_and_caption_does_not_go_to_model() {
    let dir = TempDir::new();
    dir.write("src/main.rs", "fn main() {}\n");

    let provider = MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "toolu_1",
            "Read",
            json!({ "path": "src/main.rs", "description": "Read entry point." }),
        )),
        Turn::Events(script_text("Entry point is empty.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "what is in main?").await;
    let events = collect(&mut rx).await;

    assert_eq!(
        names(&events),
        vec![
            "stateChanged",
            "userMessage",
            "checkpointCreating",
            "checkpointCreationFinished",
            "checkpointCreated",
            "checkpointsUpdated",
            "turnStarted",
            "toolCallPreparing",
            "toolCallPreparing",
            "toolCallPreparing",
            "usageUpdated",
            "toolCallStarted",
            "toolCallProgress",
            "toolCallFinished",
            "textDelta",
            "usageUpdated",
            "turnFinished",
        ]
    );

    match &events[11] {
        AgentEvent::ToolCallStarted {
            tool,
            description,
            target,
            input,
            ..
        } => {
            assert_eq!(tool, "Read");
            // A caption without a full stop at the end — the §5.4.1 form.
            assert_eq!(description.as_deref(), Some("Read entry point"));
            assert_eq!(target.as_deref(), Some("src/main.rs"));
            assert!(
                input.get("description").is_none(),
                "caption is feed concern, not an argument to tool"
            );
        }
        other => panic!("not a tool start: {other:?}"),
    }

    match &events[12] {
        AgentEvent::ToolCallProgress { chunk, .. } => assert!(chunk.contains("fn main")),
        other => panic!("not tool output: {other:?}"),
    }
}

/// Usage arrives after every model response, not once per turn: the ring
/// of state has to fill up in the middle of a turn with tools too.
#[tokio::test]
async fn usage_accumulates_on_each_model_response() {
    let dir = TempDir::new();
    dir.write("src/main.rs", "fn main() {}\n");

    let provider = MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "toolu_1",
            "Read",
            json!({ "path": "src/main.rs", "description": "Read entry point" }),
        )),
        Turn::Events(script_text("Empty.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "what is in main?").await;
    let events = collect(&mut rx).await;

    let usage_totals: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::UsageUpdated { usage } => Some(usage.total.output),
            _ => None,
        })
        .collect();
    assert_eq!(
        usage_totals,
        vec![30, 50],
        "session counter must accumulate"
    );
}

#[tokio::test]
async fn provider_reported_cost_replaces_estimate() {
    let dir = TempDir::new();
    let mut events = script_text("Answer");
    events.insert(
        events.len() - 1,
        crate::provider::ProviderEvent::Cost { usd: 0.0123 },
    );
    let handle = session(
        &dir,
        MockProvider::new(vec![Turn::Events(events)]),
        Mode::Plan,
    );
    let mut rx = handle.subscribe();
    ask(&handle, "hello").await;
    let events = collect(&mut rx).await;
    let usage = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::UsageUpdated { usage } => Some(usage),
            _ => None,
        })
        .unwrap();
    assert!((usage.cost_usd - 0.0123).abs() < 1e-12);
    assert_eq!(usage.total.input, 100);
    assert_eq!(usage.total.output, 20);
}

#[tokio::test]
async fn routing_usage_adds_to_cost_without_inflating_context() {
    let dir = TempDir::new();
    let mut events = script_text("Answer");
    events.insert(
        0,
        crate::provider::ProviderEvent::AsideUsage {
            usage: crate::usage::TokenCounts {
                input: 10,
                output: 5,
                ..Default::default()
            },
            pricing: crate::usage::Pricing {
                input: 1.0,
                output: 2.0,
            },
            cost: Some(0.01),
        },
    );
    let handle = session(
        &dir,
        MockProvider::new(vec![Turn::Events(events)]),
        Mode::Plan,
    );
    let mut rx = handle.subscribe();
    ask(&handle, "hello").await;
    let events = collect(&mut rx).await;
    let usage = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::UsageUpdated { usage } => Some(usage),
            _ => None,
        })
        .unwrap();
    assert_eq!(usage.total.input, 110);
    assert_eq!(usage.total.output, 25);
    assert_eq!(usage.context_used, 120);
    assert!((usage.cost_usd - 0.011).abs() < 1e-10);
}

#[tokio::test]
async fn escaping_project_boundary_is_rejection_not_failure() {
    let dir = TempDir::new();

    let provider = MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "toolu_1",
            "Read",
            json!({ "path": "../../secrets.env", "description": "Peeking outside" }),
        )),
        Turn::Events(script_text("Did not work.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "read neighboring project").await;
    let events = collect(&mut rx).await;

    // §5.9: a tripped guard is painted `--vermilion-ink`, not `--rust`,
    // and it is a separate event, not a failed tool.
    assert!(names(&events).contains(&"toolCallRejected"));
    assert!(!names(&events).contains(&"toolCallFailed"));
}

#[tokio::test]
async fn unknown_tool_does_not_crash_turn() {
    let dir = TempDir::new();

    let provider = MockProvider::new(vec![
        Turn::Events(script_tool_call("t1", "Write", json!({ "path": "a" }))),
        Turn::Events(script_text("Got it, no such tool.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "rewrite file").await;
    let events = collect(&mut rx).await;

    assert!(names(&events).contains(&"toolCallFailed"));
    assert!(matches!(
        events.last(),
        Some(AgentEvent::TurnFinished {
            stop_reason: StopReason::EndTurn,
            ..
        })
    ));
}

#[tokio::test]
async fn provider_error_clears_ring_rather_than_hanging() {
    let dir = TempDir::new();

    let provider = MockProvider::new(vec![Turn::Fail(ProviderError::Http {
        status: 401,
        message: "invalid x-api-key".into(),
        retry_after_s: None,
    })]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "hello").await;
    let events = collect(&mut rx).await;

    assert_eq!(
        names(&events),
        vec![
            "stateChanged",
            "userMessage",
            "checkpointCreating",
            "checkpointCreationFinished",
            "checkpointCreated",
            "checkpointsUpdated",
            "turnStarted",
            "error",
            "turnFinished",
        ]
    );
    match &events[7] {
        AgentEvent::Error {
            code, retryable, ..
        } => {
            assert_eq!(code, "http_401");
            assert!(!retryable, "401 is pointless to retry");
        }
        other => panic!("not an error: {other:?}"),
    }
    // Without `TurnFinished` the ring would spin forever — the interface would lie
    // about the state, which §0 does not allow.
    assert!(matches!(
        events.last(),
        Some(AgentEvent::TurnFinished {
            stop_reason: StopReason::Failed,
            ..
        })
    ));
}

#[tokio::test]
async fn rate_limit_retries_and_shows_attempt_count() {
    let dir = TempDir::new();

    let provider = MockProvider::new(vec![
        Turn::Fail(ProviderError::Http {
            status: 429,
            message: "rate limited".into(),
            // The provider said when to come back — we obey it,
            // and the test does not sleep on it.
            retry_after_s: Some(0),
        }),
        Turn::Events(script_text("Reached on second attempt.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "hello").await;
    let events = collect(&mut rx).await;

    assert_eq!(
        names(&events),
        vec![
            "stateChanged",
            "userMessage",
            "checkpointCreating",
            "checkpointCreationFinished",
            "checkpointCreated",
            "checkpointsUpdated",
            "turnStarted",
            "retryScheduled",
            "textDelta",
            "usageUpdated",
            "turnFinished",
        ]
    );
    match &events[7] {
        AgentEvent::RetryScheduled { attempt, of, .. } => {
            // §5.10: "Retry in 4 s · attempt 2 of 5".
            assert_eq!((*attempt, *of), (2, 5));
        }
        other => panic!("not a retry: {other:?}"),
    }
}

#[tokio::test]
async fn cancellation_aborts_turn_and_notifies() {
    let dir = TempDir::new();

    let provider = MockProvider::new(vec![Turn::Events(script_text("long response"))])
        .with_gap(Duration::from_millis(80));
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "tell everything").await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    handle.send(Command::Cancel).await.unwrap();

    let events = collect(&mut rx).await;
    assert!(
        matches!(events.last(), Some(AgentEvent::TurnCancelled { .. })),
        "received {:?}",
        names(&events)
    );
    assert!(!names(&events).contains(&"turnFinished"));
}

#[tokio::test]
async fn plan_mode_does_not_give_writing_tools_to_model() {
    let dir = TempDir::new();
    let provider = Arc::new(MockProvider::text("ok"));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Plan;
    cfg.journal = None;
    let handle = spawn(cfg, provider.clone()).unwrap();
    let mut rx = handle.subscribe();

    ask(&handle, "hello").await;
    collect(&mut rx).await;

    let seen = provider.seen.lock().unwrap();
    let tools: Vec<_> = seen[0].tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        tools,
        vec![
            // Asked in Plan too: a question is a read of the human, not a write.
            "AskUser",
            "Bash",
            "ExitPlanMode",
            "Glob",
            "Grep",
            "Read",
            "ReadImage",
            "Send",
            // Shows a file to the human and writes nothing.
            "SendText",
            // Read-only, so offered in plan as well as in agent.
            // Allowed in plan: naming the conversation changes nothing on disk.
            "SetTitle",
            "Skill",
            // The mode binds a subagent as it binds the agent.
            "Task",
            "TodoWrite",
            "WebFetch",
            "WebSearch"
        ]
    );
    assert!(seen[0].system.contains("Mode: plan"));
}

#[tokio::test]
async fn mode_change_is_observable_and_propagates_to_next_request() {
    let dir = TempDir::new();
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("ok"))]));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Manual;
    cfg.journal = None;
    let handle = spawn(cfg, provider.clone()).unwrap();
    let mut rx = handle.subscribe();

    handle
        .send(Command::SetMode { mode: Mode::Plan })
        .await
        .unwrap();

    match tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap()
    {
        AgentEvent::StateChanged { state } => assert_eq!(state.mode, Mode::Plan),
        other => panic!("not a state change: {other:?}"),
    }
    assert_eq!(handle.state().mode, Mode::Plan);

    ask(&handle, "make a plan").await;
    collect(&mut rx).await;

    let seen = provider.seen.lock().unwrap();
    let tools: Vec<_> = seen[0].tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tools.contains(&"TodoWrite"));
    assert!(tools.contains(&"ExitPlanMode"));
    assert!(!tools.contains(&"Edit"));
}

#[tokio::test]
async fn attached_file_is_considered_read() {
    let dir = TempDir::new();
    dir.write("notes.md", "notes");

    let provider = Arc::new(MockProvider::text("read it"));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.journal = None;
    let handle = spawn(cfg, provider.clone()).unwrap();
    let mut rx = handle.subscribe();

    handle
        .send(Command::SendMessage {
            text: "what is here?".into(),
            attachments: vec!["notes.md".into()],
        })
        .await
        .unwrap();
    collect(&mut rx).await;

    // §7.2: the human attached the file themselves; making the agent reread it would be silly.
    let seen = provider.seen.lock().unwrap();
    let first = format!("{:?}", seen[0].messages[0]);
    assert!(
        first.contains("notes.md"),
        "attachment did not reach message"
    );
}

#[tokio::test]
async fn journal_replays_into_identical_stream() {
    let dir = TempDir::new();
    let mut cfg = SessionConfig::new(dir.path());
    let chats = dir.path().join("chats");
    cfg.journal = Some(chats.clone());
    let handle = spawn(cfg, Arc::new(MockProvider::text("hello"))).unwrap();
    let mut rx = handle.subscribe();

    ask(&handle, "greetings").await;
    let live = collect(&mut rx).await;

    let path = crate::store::location::project_dir(&chats, dir.path())
        .join(format!("{}.jsonl", handle.id()));
    let replayed = log::replay(&path).unwrap();

    // The log writes exactly what the subscriber saw: the session is restored
    // by replaying the file, and any mismatch here is a mismatch in the feed.
    // The trailing `stateChanged` arrives after `turnFinished`, on which
    // collecting the live stream stopped.
    assert_eq!(
        names(&replayed),
        [names(&live), vec!["stateChanged"]].concat()
    );
}

// --- edits in the working tree (§6) -------------------------------------------

fn edit_call(path: &str, old: &str, new: &str) -> Vec<crate::provider::ProviderEvent> {
    script_tool_call(
        "toolu_edit",
        "Edit",
        json!({ "path": path, "old_string": old, "new_string": new }),
    )
}

fn read_call(id: &str, path: &str) -> Vec<crate::provider::ProviderEvent> {
    script_tool_call(
        id,
        "Read",
        json!({ "path": path, "description": "Looked at file" }),
    )
}

#[tokio::test]
async fn edit_persists_to_tree_and_provides_diff_to_feed() {
    let dir = TempDir::new();
    let path = dir.write("src/a.rs", "let x = 1;\n");

    let provider = MockProvider::new(vec![
        // The mock too has to read before an edit: the invariant is not switched off.
        Turn::Events(read_call("toolu_read", "src/a.rs")),
        Turn::Events(edit_call("src/a.rs", "let x = 1;", "let x = 42;")),
        Turn::Events(script_text("Done.")),
    ]);
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "change one to forty two").await;
    let events = collect(&mut rx).await;

    let names_list = names(&events);
    assert!(
        names_list.contains(&"fileChanged"),
        "received {names_list:?}"
    );

    let (fact, diff) = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::FileChanged { fact, diff, .. } => Some((fact, diff)),
            _ => None,
        })
        .expect("no edit event found");
    assert_eq!(fact.path, "src/a.rs");
    assert_eq!(fact.kind, crate::change::ChangeKind::Modified);
    assert_eq!((fact.added, fact.removed), (1, 1));
    assert!(!diff.hunks.is_empty(), "expanded row without diff is empty");

    // §16.37: there is one confirmation and it is already given — the edit is on disk.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "let x = 42;\n");
}

#[tokio::test]
async fn agent_rereads_its_own_edit() {
    let dir = TempDir::new();
    dir.write("a.rs", "old\n");

    let provider = MockProvider::new(vec![
        Turn::Events(read_call("t1", "a.rs")),
        Turn::Events(edit_call("a.rs", "old", "new")),
        Turn::Events(read_call("t3", "a.rs")),
        Turn::Events(script_text("All in place.")),
    ]);
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "replace word").await;
    let events = collect(&mut rx).await;

    // The last read has to return the edit: it is already on disk, but the fingerprint
    // after the write must be fresh, or the read will hit the invariant.
    let last_output = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ToolCallProgress { chunk, .. } => Some(chunk.clone()),
            _ => None,
        })
        .next_back()
        .expect("no tool output found");
    assert!(last_output.contains("new"), "read: {last_output}");
}

#[tokio::test]
async fn edit_to_unread_file_is_rejected_not_crashed() {
    let dir = TempDir::new();
    let path = dir.write("a.rs", "someone's work\n");

    let provider = MockProvider::new(vec![
        Turn::Events(edit_call("a.rs", "someone's work", "my work")),
        Turn::Events(script_text("Understood, reading first.")),
    ]);
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "rewrite file").await;
    let events = collect(&mut rx).await;

    // §5.9: a tripped guard is not a failure, and it is painted differently.
    let names_list = names(&events);
    assert!(
        names_list.contains(&"toolCallRejected"),
        "received {names_list:?}"
    );
    assert!(!names_list.contains(&"fileChanged"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "someone's work\n");
}

#[tokio::test]
async fn naming_the_chat_leaves_no_row_in_the_stream() {
    let dir = TempDir::new();
    let provider = MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "toolu_1",
            "SetTitle",
            json!({ "title": "Parsing dates" }),
        )),
        Turn::Events(script_text("Okay.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "hello").await;
    let events = collect(&mut rx).await;

    let kinds = names(&events);
    assert!(kinds.contains(&"titleSet"), "{kinds:?}");
    assert!(
        !kinds.iter().any(|k| k.starts_with("toolCall")),
        "the call must not reach the stream: {kinds:?}"
    );
    // The model still got its answer: the second response was asked for.
    assert!(kinds.contains(&"textDelta"));
}
