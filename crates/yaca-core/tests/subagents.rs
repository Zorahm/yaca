//! `Task`: a subagent holds a conversation of its own and hands back a report.
//!
//! Driven through the public surface like the rest, and the mock's `seen` is
//! what tells the two conversations apart: the subagent's requests carry its
//! own system prompt and a history that starts at the task.
//!
//! What a subagent does to the project is held to what the agent is held to,
//! and that is checked from the feed: its edit asks first, and shows its diff.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::id::RequestId;
use yaca_core::message::ContentBlock;
use yaca_core::provider::mock::{script_text, script_tool_call, MockProvider, Turn};
use yaca_core::provider::ProviderRequest;
use yaca_core::{Command, Mode, PermissionAnswer, SessionConfig, SessionHandle, SessionStore};

use common::{ask, calls, is_turn_over, session, until, TempProject};

fn task(id: &str, prompt: &str) -> Turn {
    Turn::Events(script_tool_call(
        id,
        "Task",
        json!({ "prompt": prompt, "description": "Выяснил, где разбираются даты" }),
    ))
}

fn read(id: &str, path: &str) -> Turn {
    Turn::Events(script_tool_call(
        id,
        "Read",
        json!({ "path": path, "description": "Прочитал разбор дат" }),
    ))
}

fn text(reply: &str) -> Turn {
    Turn::Events(script_text(reply))
}

fn write(id: &str, path: &str) -> Turn {
    Turn::Events(script_tool_call(
        id,
        "Write",
        json!({ "path": path, "content": "new\n" }),
    ))
}

/// Each row of the feed with the row it is drawn under.
fn rows(seen: &[AgentEvent]) -> Vec<(String, Option<String>)> {
    seen.iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallStarted {
                call_id, parent, ..
            } => Some((call_id.0.clone(), parent.as_ref().map(|p| p.0.clone()))),
            _ => None,
        })
        .collect()
}

fn is_asking(event: &AgentEvent) -> bool {
    matches!(event, AgentEvent::PermissionRequested { .. })
}

/// The open question and the call it is about.
fn asked(seen: &[AgentEvent]) -> (RequestId, String) {
    seen.iter()
        .find_map(|event| match event {
            AgentEvent::PermissionRequested { request } => {
                Some((request.id.clone(), request.call_id.0.clone()))
            }
            _ => None,
        })
        .expect("no permission was requested")
}

async fn allow(handle: &SessionHandle, request_id: RequestId) {
    handle
        .send(Command::AnswerPermission {
            request_id,
            answer: PermissionAnswer::Allow,
        })
        .await
        .unwrap();
}

fn requests(provider: &MockProvider) -> Vec<ProviderRequest> {
    provider.seen.lock().unwrap().clone()
}

/// The result the model was given for a call, and whether it was an error.
fn result_of(request: &ProviderRequest, id: &str) -> (String, bool) {
    request
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .find_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } if tool_use_id == id => Some((content.clone(), *is_error)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no result for {id}"))
}

/// Everything the feed was shown under one row.
fn shown_under(seen: &[AgentEvent], id: &str) -> String {
    seen.iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallProgress { call_id, chunk } if call_id.0 == id => {
                Some(chunk.as_str())
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn the_report_comes_back_and_the_steps_stay_behind() {
    let dir = TempProject::new();
    dir.write("dates.rs", "fn parse_date() {}\n");

    let provider = Arc::new(MockProvider::new(vec![
        task("t1", "Find where dates are parsed."),
        read("r1", "dates.rs"),
        text("Dates are parsed in dates.rs:1."),
        text("готово"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "где разбираются даты?").await;
    let seen = until(&mut rx, is_turn_over).await;
    let sent = requests(&provider);
    assert_eq!(sent.len(), 4);

    // The subagent starts from the task alone, under a prompt of its own, and
    // cannot hand the work on to another one.
    let child = &sent[1];
    assert_ne!(child.system, sent[0].system);
    assert_eq!(child.messages.len(), 1);
    assert_eq!(
        child.messages[0].content,
        vec![ContentBlock::Text {
            text: "Find where dates are parsed.".into()
        }]
    );
    let tools: Vec<_> = child.tools.iter().map(|t| t.name.as_str()).collect();
    for given in ["Read", "Edit", "Bash", "WebFetch"] {
        assert!(tools.contains(&given), "{given} is missing: {tools:?}");
    }
    for own in ["Task", "TodoWrite", "ExitPlanMode"] {
        assert!(!tools.contains(&own), "{own} is the agent's: {tools:?}");
    }

    // The parent gets the report and none of what led to it.
    let parent = &sent[3];
    assert_eq!(
        result_of(parent, "t1"),
        ("Dates are parsed in dates.rs:1.".to_string(), false)
    );
    assert!(parent
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .all(|b| !matches!(b, ContentBlock::ToolUse { name, .. } if name == "Read")));

    // What the subagent did is rows of its own, drawn under the row that
    // sent it; what it reported is that row's output.
    assert_eq!(
        rows(&seen),
        vec![
            ("t1".to_string(), None),
            ("r1".to_string(), Some("t1".to_string())),
        ]
    );
    assert_eq!(shown_under(&seen, "t1"), "Dates are parsed in dates.rs:1.");

    // What the subagent said is not the agent speaking.
    assert!(seen.iter().all(|event| !matches!(
        event,
        AgentEvent::TextDelta { text, .. } if text.contains("dates.rs:1")
    )));
}

#[tokio::test]
async fn what_the_subagent_read_the_agent_has_not() {
    let dir = TempProject::new();
    dir.write("dates.rs", "fn parse_date() {}\n");

    let provider = Arc::new(MockProvider::new(vec![
        task("t1", "Read dates.rs."),
        read("r1", "dates.rs"),
        text("It parses dates."),
        Turn::Events(script_tool_call(
            "w1",
            "Edit",
            json!({ "path": "dates.rs", "old_string": "parse_date", "new_string": "parse" }),
        )),
        text("не вышло"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "переименуй").await;
    let seen = until(&mut rx, is_turn_over).await;

    // §8, read-before-edit: the content went to another conversation, so the
    // agent would be editing a file it has never seen.
    assert!(
        seen.iter().any(|event| matches!(
            event,
            AgentEvent::ToolCallRejected { call_id, .. } if call_id.0 == "w1"
        )),
        "the edit went through on the subagent's reading"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("dates.rs")).unwrap(),
        "fn parse_date() {}\n"
    );
}

#[tokio::test]
async fn a_subagent_costs_money_without_filling_the_window() {
    let dir = TempProject::new();
    dir.write("dates.rs", "fn parse_date() {}\n");

    let provider = Arc::new(MockProvider::new(vec![
        task("t1", "Read dates.rs."),
        read("r1", "dates.rs"),
        text("It parses dates."),
        text("готово"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "посмотри").await;
    let seen = until(&mut rx, is_turn_over).await;

    let usage = seen
        .iter()
        .rev()
        .find_map(|event| match event {
            AgentEvent::UsageUpdated { usage } => Some(*usage),
            _ => None,
        })
        .expect("no usage reported");

    // Four requests of a hundred input tokens each: two were the subagent's.
    assert_eq!(usage.total.input, 400);
    // The ring is the agent's own window, measured by its own last answer.
    assert_eq!(usage.last.input, 100);
    assert_eq!(usage.context_used, 120);
}

#[tokio::test]
async fn independent_tasks_run_at_the_same_time() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            ("t1", "Task", json!({ "prompt": "First." })),
            ("t2", "Task", json!({ "prompt": "Second." })),
        ])),
        text("report"),
        text("report"),
        text("готово"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "два вопроса").await;
    let seen = until(&mut rx, is_turn_over).await;

    let order: Vec<_> = seen
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallStarted { .. } => Some("started"),
            AgentEvent::ToolCallFinished { .. } => Some("finished"),
            _ => None,
        })
        .collect();
    assert_eq!(order, vec!["started", "started", "finished", "finished"]);

    let last = requests(&provider).pop().unwrap();
    assert_eq!(result_of(&last, "t1").0, "report");
    assert_eq!(result_of(&last, "t2").0, "report");
}

#[tokio::test]
async fn esc_stops_the_subagent_with_the_turn() {
    let dir = TempProject::new();

    let provider = Arc::new(
        MockProvider::new(vec![
            task("t1", "Take your time."),
            text("a report nobody waits for"),
        ])
        .with_gap(Duration::from_millis(40)),
    );
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "посмотри").await;
    until(&mut rx, |e| matches!(e, AgentEvent::ToolCallStarted { .. })).await;
    handle.send(Command::Cancel).await.unwrap();
    let seen = until(&mut rx, is_turn_over).await;

    assert!(matches!(
        seen.last(),
        Some(AgentEvent::TurnCancelled { .. })
    ));
    assert!(seen
        .iter()
        .any(|event| matches!(event, AgentEvent::ToolCallDenied { stopped: true, .. })));
    // Nothing was asked of the model after the stop: the agent's request and
    // at most the one the subagent had already opened.
    assert!(requests(&provider).len() <= 2);
}

#[tokio::test]
async fn a_subagent_that_never_reports_runs_out_of_steps() {
    let dir = TempProject::new();
    dir.write("dates.rs", "fn parse_date() {}\n");

    // One request more than the cap: the last one is the subagent ignoring
    // the instruction to stop and report.
    let steps = yaca_core::session::subagent::MAX_STEPS + 1;
    let mut turns = vec![task("t1", "Read dates.rs forever.")];
    turns.extend((0..steps).map(|i| read(&format!("r{i}"), "dates.rs")));
    turns.push(text("не вышло"));

    let provider = Arc::new(MockProvider::new(turns));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "посмотри").await;
    let seen = until(&mut rx, is_turn_over).await;

    assert!(seen.iter().any(|event| matches!(
        event,
        AgentEvent::ToolCallFailed { call_id, .. } if call_id.0 == "t1"
    )));

    let sent = requests(&provider);
    assert_eq!(sent.len(), steps + 2);
    // Before giving up it was told, once, that the steps were spent.
    let told = |request: &ProviderRequest| {
        request.messages.last().is_some_and(|m| {
            m.content
                .iter()
                .any(|b| matches!(b, ContentBlock::Text { text } if text.contains("final report")))
        })
    };
    assert!(told(&sent[steps]));
    assert!(!told(&sent[steps - 1]));
    assert!(result_of(&sent[steps + 1], "t1").1);
}

#[tokio::test]
async fn a_reopened_session_shows_what_the_subagent_did() {
    let dir = TempProject::new();
    dir.write("dates.rs", "fn parse_date() {}\n");

    let provider = Arc::new(MockProvider::new(vec![
        task("t1", "Find where dates are parsed."),
        read("r1", "dates.rs"),
        text("Dates are parsed in dates.rs:1."),
        text("готово"),
    ]));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.journal = true;
    let handle = yaca_core::spawn(cfg, provider).unwrap();
    let mut rx = handle.subscribe();

    ask(&handle, "где разбираются даты?").await;
    until(&mut rx, is_turn_over).await;

    // The journal is all a reopened session has of the feed.
    let replayed = SessionStore::open(dir.path()).feed(handle.id());
    assert_eq!(
        rows(&replayed),
        vec![
            ("t1".to_string(), None),
            ("r1".to_string(), Some("t1".to_string())),
        ]
    );
    assert_eq!(
        shown_under(&replayed, "t1"),
        "Dates are parsed in dates.rs:1."
    );
}

#[tokio::test]
async fn the_subagent_model_comes_from_the_project_settings() {
    let dir = TempProject::new();
    dir.write(
        ".yaca/settings.json",
        r#"{ "subagentModel": "claude-haiku-4-5" }"#,
    );

    let provider = Arc::new(MockProvider::new(vec![
        task("t1", "Look around."),
        text("report"),
        text("готово"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "посмотри").await;
    until(&mut rx, is_turn_over).await;

    let sent = requests(&provider);
    assert_eq!(sent[1].model, "claude-haiku-4-5");
    // Haiku 4.5 takes neither: sending them is a 400 instead of an answer.
    assert_eq!(sent[1].effort, None);
    assert!(!sent[1].thinking);
    assert_ne!(sent[0].model, sent[1].model);
}

#[tokio::test]
async fn a_subagents_edit_asks_first_and_shows_its_diff() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![
        task("t1", "Create notes.txt."),
        write("w1", "notes.txt"),
        text("Created notes.txt."),
        text("готово"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "заведи файл").await;
    let asking = until(&mut rx, is_asking).await;

    // §5.7: decided before anything happens, whoever is asking.
    let (request, call) = asked(&asking);
    assert_eq!(call, "w1");
    assert!(!dir.path().join("notes.txt").exists());

    allow(&handle, request).await;
    let rest = until(&mut rx, is_turn_over).await;

    assert!(rest.iter().any(|event| matches!(
        event,
        AgentEvent::FileChanged { call_id, .. } if call_id.0 == "w1"
    )));
    assert!(rows(&asking).contains(&("w1".to_string(), Some("t1".to_string()))));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("notes.txt")).unwrap(),
        "new\n"
    );
}

#[tokio::test]
async fn a_refusal_reaches_the_subagent_and_the_turn_goes_on() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![
        task("t1", "Create notes.txt."),
        write("w1", "notes.txt"),
        text("The human refused the write."),
        text("понял"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "заведи файл").await;
    let (request, _) = asked(&until(&mut rx, is_asking).await);
    handle
        .send(Command::AnswerPermission {
            request_id: request,
            answer: PermissionAnswer::Deny {
                reason: Some("не сейчас".into()),
            },
        })
        .await
        .unwrap();
    until(&mut rx, is_turn_over).await;

    let sent = requests(&provider);
    // The note is the instruction (§5.7), and it is the subagent that needs it.
    let (refusal, is_error) = result_of(&sent[2], "w1");
    assert!(is_error);
    assert!(refusal.contains("не сейчас"), "{refusal}");
    assert!(!dir.path().join("notes.txt").exists());
    assert_eq!(result_of(&sent[3], "t1").0, "The human refused the write.");
}

#[tokio::test]
async fn two_subagents_ask_one_at_a_time() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            ("t1", "Task", json!({ "prompt": "Create a file." })),
            ("t2", "Task", json!({ "prompt": "Create another." })),
        ])),
        write("w1", "a.txt"),
        write("w2", "b.txt"),
        text("done"),
        text("done"),
        text("готово"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "два файла").await;
    let (first, _) = asked(&until(&mut rx, is_asking).await);

    // §5.7 draws one block. Both subagents have their write in hand by now,
    // and the second waits its turn rather than asking over the first.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut meanwhile = Vec::new();
    while let Ok(event) = rx.try_recv() {
        meanwhile.push(event);
    }
    assert!(!meanwhile.iter().any(is_asking), "two questions at once");

    allow(&handle, first).await;
    let (second, _) = asked(&until(&mut rx, is_asking).await);
    allow(&handle, second).await;
    until(&mut rx, is_turn_over).await;

    assert!(dir.path().join("a.txt").exists());
    assert!(dir.path().join("b.txt").exists());
}

#[tokio::test]
async fn plan_mode_binds_the_subagent_too() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![
        task("t1", "Rename the function."),
        write("w1", "notes.txt"),
        text("I can only report what it would take."),
        text("готово"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "переименуй").await;
    let seen = until(&mut rx, is_turn_over).await;

    // §7.1: the writing tools are not offered at all, and one the model makes
    // up is an unknown tool rather than a question to the human.
    let sent = requests(&provider);
    let tools: Vec<_> = sent[1].tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        tools,
        vec![
            "Bash",
            "Glob",
            "Grep",
            "Read",
            "ReadImage",
            "Skill",
            "WebFetch",
            "WebSearch"
        ]
    );
    assert!(sent[1].system.contains("Mode: plan"));
    assert!(!seen.iter().any(is_asking));
    assert!(result_of(&sent[2], "w1").1);
    assert!(!dir.path().join("notes.txt").exists());
}

#[tokio::test]
async fn a_subagent_is_given_the_memory_index_the_agent_has() {
    let dir = TempProject::new();
    dir.write(
        ".yaca/memory/MEMORY.md",
        "- [release](release.md) — how the release is cut\n",
    );

    let provider = Arc::new(MockProvider::new(vec![
        task("t1", "Look around."),
        text("report"),
        text("готово"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "посмотри").await;
    until(&mut rx, is_turn_over).await;

    // The subagent has the Memory tool, so it must also have the index that
    // says what there is to read; the prompt is the same shape as the agent's.
    let sent = requests(&provider);
    assert!(sent[0].system.contains("<project_memory>"));
    assert!(sent[1].system.contains("how the release is cut"));
    assert!(sent[1].system.contains("<project_memory>"));
}

#[tokio::test]
async fn each_subagent_reports_what_it_has_spent_so_far() {
    let dir = TempProject::new();
    dir.write("dates.rs", "fn parse_date() {}
");

    let provider = Arc::new(MockProvider::new(vec![
        task("t1", "Find where dates are parsed."),
        read("r1", "dates.rs"),
        text("Dates are parsed in dates.rs:1."),
        text("готово"),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "где разбираются даты?").await;
    let seen = until(&mut rx, is_turn_over).await;

    // One event per answer the subagent got, each the total up to then: the
    // panel draws the newest and never has to add them up.
    let spent: Vec<_> = seen
        .iter()
        .filter_map(|event| match event {
            AgentEvent::SubagentUsage { call_id, tokens } => {
                Some((call_id.0.as_str(), tokens.input, tokens.output))
            }
            _ => None,
        })
        .collect();
    assert_eq!(spent, vec![("t1", 100, 30), ("t1", 200, 50)]);
}
