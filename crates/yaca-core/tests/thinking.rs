//! Replayed thinking under a request prefix that moved.
//!
//! A thinking block is bound to the prefix that produced it: the system prompt,
//! the tool set and every earlier message. Claude Fable 5.1 rejects a request
//! that replays a block under a different prefix, and later models are expected
//! to do the same for every account. These tests pin down when the core stops
//! sending old reasoning — and, just as much, when it keeps it, because a
//! session that forgets its reasoning on every reopen pays for it in quality.

mod common;

use std::sync::Arc;

use yaca_core::id::{SessionId, TurnId};
use yaca_core::message::{ContentBlock, Message};
use yaca_core::provider::mock::{script_text, script_thinking, MockProvider, Turn};
use yaca_core::provider::ProviderRequest;
use yaca_core::store::history::{MessageLog, Record};
use yaca_core::{AgentEvent, Command, Mode, SessionConfig, SessionHandle};

use common::{ask, is_turn_over, until, TempProject};

fn recorded(
    dir: &TempProject,
    provider: Arc<MockProvider>,
    mode: Mode,
    resume: Option<SessionId>,
) -> SessionHandle {
    let mut config = SessionConfig::new(dir.path());
    config.mode = mode;
    config.journal = true;
    config.resume = resume;
    yaca_core::spawn(config, provider).unwrap()
}

/// One reply per label, each thinking `<label> thought` before answering
/// `<label> answer`.
fn thinking(labels: &[&str]) -> Arc<MockProvider> {
    Arc::new(MockProvider::new(labels.iter().map(|l| reply(l)).collect()))
}

fn reply(label: &str) -> Turn {
    Turn::Events(script_thinking(
        &format!("{label} thought"),
        &format!("{label} answer"),
    ))
}

async fn turn(handle: &SessionHandle, text: &str) {
    let mut rx = handle.subscribe();
    ask(handle, text).await;
    until(&mut rx, is_turn_over).await;
}

async fn set_mode(handle: &SessionHandle, mode: Mode) {
    let mut rx = handle.subscribe();
    handle.send(Command::SetMode { mode }).await.unwrap();
    until(&mut rx, |e| matches!(e, AgentEvent::StateChanged { .. })).await;
}

fn requests(provider: &MockProvider) -> Vec<ProviderRequest> {
    provider.seen.lock().unwrap().clone()
}

/// The reasoning a request replays, in order.
fn thoughts(request: &ProviderRequest) -> Vec<String> {
    request
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|b| match b {
            ContentBlock::Thinking { thinking, .. } => Some(thinking.clone()),
            ContentBlock::RedactedThinking { data } => Some(data.clone()),
            _ => None,
        })
        .collect()
}

fn texts(request: &ProviderRequest) -> Vec<String> {
    request
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

// --- resume -----------------------------------------------------------------

/// A session recorded under one `AGENTS.md`, left, and the file edited.
async fn recorded_then_edited(dir: &TempProject) -> SessionId {
    dir.write("AGENTS.md", "Use tabs.\n");
    let first = recorded(dir, thinking(&["first"]), Mode::Manual, None);
    turn(&first, "first question").await;
    let id = first.id().clone();
    drop(first);
    dir.write("AGENTS.md", "Use spaces.\n");
    id
}

#[tokio::test]
async fn a_resumed_session_keeps_its_thinking_when_nothing_moved() {
    let dir = TempProject::new();
    dir.write("AGENTS.md", "Use tabs.\n");
    let first = recorded(&dir, thinking(&["first"]), Mode::Manual, None);
    turn(&first, "first question").await;
    let id = first.id().clone();
    drop(first);

    let provider = thinking(&["second"]);
    let again = recorded(&dir, provider.clone(), Mode::Manual, Some(id));
    turn(&again, "second question").await;

    // Same prompt, same tools: the signature is still good, and dropping the
    // reasoning would make the model re-derive what it already worked out.
    let sent = &requests(&provider)[0];
    assert_eq!(thoughts(sent), ["first thought"]);
    assert!(sent.messages.iter().flat_map(|m| &m.content).any(|b| matches!(
        b,
        ContentBlock::Thinking { signature: Some(s), .. } if s == "sig"
    )));
}

#[tokio::test]
async fn a_resumed_session_drops_thinking_made_under_the_old_instructions() {
    let dir = TempProject::new();
    let id = recorded_then_edited(&dir).await;

    let provider = thinking(&["second", "third"]);
    let again = recorded(&dir, provider.clone(), Mode::Manual, Some(id));
    turn(&again, "second question").await;
    turn(&again, "third question").await;

    let sent = requests(&provider);
    // The system prompt carries AGENTS.md, so the old block is bound to a
    // prefix that no longer exists. The conversation itself survives.
    assert!(thoughts(&sent[0]).is_empty(), "{:?}", thoughts(&sent[0]));
    assert!(texts(&sent[0]).contains(&"first answer".to_string()));
    // Reasoning produced after the change is bound to the new prefix and keeps
    // going out: only a leading run of blocks was removed.
    assert_eq!(thoughts(&sent[1]), ["second thought"]);
}

#[tokio::test]
async fn dropped_thinking_stays_dropped_when_the_session_is_reopened_again() {
    let dir = TempProject::new();
    let id = recorded_then_edited(&dir).await;

    let again = recorded(&dir, thinking(&["second"]), Mode::Manual, Some(id.clone()));
    turn(&again, "second question").await;
    drop(again);

    // Nothing moved since the second run, so its reasoning is kept — and the
    // block from the first run must not come back from the journal, or the
    // second run's block would sit behind a stale one.
    let provider = thinking(&["third"]);
    let third = recorded(&dir, provider.clone(), Mode::Manual, Some(id));
    turn(&third, "third question").await;

    assert_eq!(thoughts(&requests(&provider)[0]), ["second thought"]);
}

#[tokio::test]
async fn a_session_recorded_before_prefixes_were_tracked_drops_its_thinking() {
    let dir = TempProject::new();
    let id = SessionId::from("ses_old");
    let turn_id = TurnId::from("t1");
    {
        // A journal written by an older build: messages, no prefix records.
        // What it was sent under is unknown, and a 400 ends the session where
        // lost reasoning does not.
        let log = MessageLog::open(dir.path(), &id).unwrap();
        log.append(&Record::Prompt {
            turn: turn_id.clone(),
            text: "old question".into(),
        });
        log.append(&Record::Message {
            turn: turn_id.clone(),
            message: Message::user_text("old question"),
        });
        log.append(&Record::Message {
            turn: turn_id,
            message: Message::assistant(vec![
                ContentBlock::Thinking {
                    thinking: "old thought".into(),
                    signature: Some("sig".into()),
                },
                ContentBlock::Text {
                    text: "old answer".into(),
                },
            ]),
        });
    }

    let provider = thinking(&["new"]);
    let again = recorded(&dir, provider.clone(), Mode::Manual, Some(id));
    turn(&again, "new question").await;

    let sent = &requests(&provider)[0];
    assert!(thoughts(sent).is_empty(), "{:?}", thoughts(sent));
    assert!(texts(sent).contains(&"old answer".to_string()));
}

// --- mode -------------------------------------------------------------------

#[tokio::test]
async fn leaving_plan_mode_drops_the_thinking_done_in_plan() {
    let dir = TempProject::new();
    let provider = thinking(&["plan", "build", "more"]);
    let handle = recorded(&dir, provider.clone(), Mode::Plan, None);

    turn(&handle, "plan it").await;
    // The system prompt and the tool set both follow the mode.
    set_mode(&handle, Mode::Manual).await;
    turn(&handle, "build it").await;
    turn(&handle, "and more").await;

    let sent = requests(&provider);
    assert!(thoughts(&sent[1]).is_empty(), "{:?}", thoughts(&sent[1]));
    assert!(texts(&sent[1]).contains(&"plan answer".to_string()));
    assert_eq!(thoughts(&sent[2]), ["build thought"]);
}

#[tokio::test]
async fn switching_between_agent_modes_keeps_thinking() {
    let dir = TempProject::new();
    let provider = thinking(&["manual", "auto"]);
    let handle = recorded(&dir, provider.clone(), Mode::Manual, None);

    turn(&handle, "first").await;
    // Manual and Auto share a prompt and a tool set: only permissions differ,
    // and those never reach the request.
    set_mode(&handle, Mode::Auto).await;
    turn(&handle, "second").await;

    assert_eq!(thoughts(&requests(&provider)[1]), ["manual thought"]);
}

// --- compaction (§5.11) -----------------------------------------------------

/// Four thinking exchanges and a summary: two get summarized, two are kept.
async fn compacted(dir: &TempProject, after: Vec<Turn>) -> (SessionHandle, Arc<MockProvider>) {
    let mut turns: Vec<Turn> = ["one", "two", "three", "four"].iter().map(|l| reply(l)).collect();
    turns.push(Turn::Events(script_text("Fixed the date parser.")));
    turns.extend(after);
    let provider = Arc::new(MockProvider::new(turns));
    let handle = recorded(dir, provider.clone(), Mode::Manual, None);

    for text in ["one", "two", "three", "four"] {
        turn(&handle, text).await;
    }
    let mut rx = handle.subscribe();
    handle.send(Command::Compact).await.unwrap();
    until(&mut rx, |e| matches!(e, AgentEvent::ContextCompacted { .. })).await;
    (handle, provider)
}

#[tokio::test]
async fn compaction_keeps_the_tail_without_its_thinking() {
    let dir = TempProject::new();
    let (handle, provider) = compacted(&dir, vec![reply("five")]).await;

    turn(&handle, "five").await;

    // The kept turns reasoned with the whole history before them. After the
    // summary that history is gone, so their blocks are bound to nothing.
    let sent = requests(&provider).last().unwrap().clone();
    assert!(thoughts(&sent).is_empty(), "{:?}", thoughts(&sent));
    assert!(texts(&sent).contains(&"four answer".to_string()));
}

#[tokio::test]
async fn an_edited_summary_drops_the_thinking_after_it() {
    let dir = TempProject::new();
    let (handle, provider) = compacted(&dir, vec![reply("five"), reply("six")]).await;
    turn(&handle, "five").await;

    let mut rx = handle.subscribe();
    handle
        .send(Command::EditSummary {
            text: "Fixed the parser; time zones untouched.".into(),
        })
        .await
        .unwrap();
    until(&mut rx, |e| matches!(e, AgentEvent::SummaryEdited { .. })).await;
    turn(&handle, "six").await;

    let sent = requests(&provider).last().unwrap().clone();
    assert!(thoughts(&sent).is_empty(), "{:?}", thoughts(&sent));
    assert!(texts(&sent).contains(&"five answer".to_string()));
}
