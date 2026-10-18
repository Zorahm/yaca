//! Sessions, storage and compaction end to end (§9, §5.11).
//!
//! Real files in a chats folder of the test's own, and the core driven through
//! the surface the interface has: `spawn`, `Command`, `AgentEvent`, plus the plain
//! [`SessionStore`] the session panel is built on.

mod common;

use std::sync::Arc;

use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::message::{ContentBlock, Role};
use yaca_core::provider::mock::{script_text, script_tool_call, MockProvider, Turn};
use yaca_core::provider::ProviderRequest;
use yaca_core::session::compact::SUMMARY_HEADER;
use yaca_core::{Command, Mode, SessionConfig, SessionHandle};

use common::{ask, is_turn_over, until, TempProject};

/// Like the shared helper, but writing the journals — which is the whole point
/// here.
fn recorded(
    dir: &TempProject,
    provider: Arc<MockProvider>,
    resume: Option<SessionId>,
) -> SessionHandle {
    let mut config = SessionConfig::new(dir.path());
    config.mode = Mode::Manual;
    config.journal = Some(dir.chats());
    config.resume = resume;
    yaca_core::spawn(config, provider).unwrap()
}

fn replies(count: usize) -> Arc<MockProvider> {
    Arc::new(MockProvider::new(
        (0..count)
            .map(|i| Turn::Events(script_text(&format!("answer{i}"))))
            .collect(),
    ))
}

fn text_of(message: &yaca_core::message::Message) -> String {
    message
        .content
        .iter()
        .filter_map(|b| match b {
            // The environment note rides on the first message of a session;
            // these tests are about what the human's line became.
            ContentBlock::Text { text } if !text.starts_with("<environment_context>") => {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn last_request(provider: &MockProvider) -> ProviderRequest {
    provider.seen.lock().unwrap().last().unwrap().clone()
}

// --- storage ----------------------------------------------------------------

#[tokio::test]
async fn a_session_is_in_the_panel_before_it_has_finished() {
    let dir = TempProject::new();
    let handle = recorded(&dir, replies(1), None);
    let mut rx = handle.subscribe();

    ask(&handle, "fix the date parser\nhere is the trace").await;
    until(&mut rx, is_turn_over).await;

    let store = dir.store();
    let listed = store.list();
    assert_eq!(listed.len(), 1);
    // §3: the header is generated from the first message.
    assert_eq!(listed[0].title, "fix the date parser");
    assert_eq!(listed[0].messages, 1);
    assert!(listed[0].cost_usd > 0.0, "the cost is shown in the panel");
    assert_eq!(&listed[0].id, handle.id());
}

#[tokio::test]
async fn a_session_reopens_with_the_conversation_the_model_had() {
    let dir = TempProject::new();

    let first = recorded(&dir, replies(1), None);
    let mut rx = first.subscribe();
    ask(&first, "why does the parser crash").await;
    until(&mut rx, is_turn_over).await;
    let id = first.id().clone();
    let spent = first.meta().unwrap().cost_usd;
    drop(first);

    let provider = replies(1);
    let again = recorded(&dir, provider.clone(), Some(id.clone()));
    let mut rx = again.subscribe();
    ask(&again, "now fix it").await;
    until(&mut rx, is_turn_over).await;

    // The model is handed what it said last time, not a blank conversation.
    let sent = last_request(&provider);
    assert_eq!(sent.messages.len(), 3);
    assert_eq!(text_of(&sent.messages[0]), "why does the parser crash");
    assert_eq!(sent.messages[1].role, Role::Assistant);
    assert_eq!(text_of(&sent.messages[1]), "answer0");

    // And it is the same session: one line in the panel, one running cost.
    let meta = again.meta().unwrap();
    assert_eq!(meta.id, id);
    assert_eq!(meta.messages, 2);
    assert!(meta.cost_usd > spent, "{} against {spent}", meta.cost_usd);
    assert_eq!(dir.store().list().len(), 1);
}

#[tokio::test]
async fn the_models_title_is_still_the_title_when_the_turn_ends() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "toolu_1",
            "SetTitle",
            serde_json::json!({ "title": "Project overview" }),
        )),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = recorded(&dir, provider, None);
    let mut rx = handle.subscribe();

    ask(&handle, "Hi. Look at the project.").await;
    let seen = until(&mut rx, is_turn_over).await;
    assert!(seen
        .iter()
        .any(|e| matches!(e, AgentEvent::TitleSet { title } if title == "Project overview")));

    // The window re-reads the title from the index when the turn ends (§3). An
    // event that reached the window and not the index would show the name and
    // then take it back.
    assert_eq!(handle.meta().unwrap().title, "Project overview");
    let store = dir.store();
    assert_eq!(store.list()[0].title, "Project overview");

    // And it is in the journal, so a reopened feed and a rebuilt index have it.
    assert!(store
        .events(handle.id())
        .iter()
        .any(|e| matches!(e, AgentEvent::TitleSet { .. })));
}

#[tokio::test]
async fn a_name_typed_while_the_session_is_open_survives_the_next_turn() {
    let dir = TempProject::new();
    let handle = recorded(&dir, replies(2), None);
    let mut rx = handle.subscribe();
    ask(&handle, "fix the date parser").await;
    until(&mut rx, is_turn_over).await;

    handle.rename("My title").unwrap();
    // On disk at once: the panel reads the file, not the session.
    let store = dir.store();
    assert_eq!(store.list()[0].title, "My title");

    // The session writes its index whole when a turn ends. A name that went
    // into the file past the session would be overwritten here.
    ask(&handle, "now the tests").await;
    until(&mut rx, is_turn_over).await;

    let listed = store.list();
    assert_eq!(listed[0].title, "My title");
    assert!(listed[0].title_custom);
    assert_eq!(handle.meta().unwrap().title, "My title");
}

#[tokio::test]
async fn a_reopened_session_gets_its_feed_back() {
    let dir = TempProject::new();

    let first = recorded(&dir, replies(1), None);
    let mut rx = first.subscribe();
    ask(&first, "why does the parser crash").await;
    until(&mut rx, is_turn_over).await;
    let id = first.id().clone();
    drop(first);

    let store = dir.store();
    let journal = store.events(&id);
    let feed = store.feed(&id);

    // §5.1: the human's card comes back with the rest. A reopened session
    // showing only the answers would be the agent talking to itself.
    assert!(
        feed.iter().any(|e| matches!(
            e, AgentEvent::UserMessage { text, .. } if text == "why does the parser crash"
        )),
        "{feed:#?}"
    );
    assert!(feed.iter().any(|e| matches!(e, AgentEvent::TextDelta { .. })));

    // The journal writes down everything; the replay leaves behind what was
    // and no longer is — see `store::replay`.
    assert!(
        journal.iter().any(|e| matches!(e, AgentEvent::StateChanged { .. })),
        "the journal writes the state too"
    );
    assert!(
        journal.iter().any(|e| matches!(e, AgentEvent::CheckpointCreated { .. })),
        "and the taken checkpoints"
    );
    assert!(!feed.iter().any(|e| matches!(
        e,
        AgentEvent::StateChanged { .. } | AgentEvent::ProcessesUpdated { .. }
    )));

    // The points are not left behind: the session keeps the list of them, so
    // the reopened chat holds them and its dividers can be rolled back to.
    let taken: Vec<_> = journal
        .iter()
        .filter_map(|e| match e {
            AgentEvent::CheckpointCreated { checkpoint } => Some(checkpoint.id.clone()),
            _ => None,
        })
        .collect();
    let shown: Vec<_> = feed
        .iter()
        .filter_map(|e| match e {
            AgentEvent::CheckpointCreated { checkpoint } => Some(checkpoint.id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(shown, taken, "the dividers of the points the session holds");
    let lists = feed
        .iter()
        .filter(|e| matches!(e, AgentEvent::CheckpointsUpdated { .. }))
        .count();
    assert_eq!(lists, 1, "and the panel's list, once, as it is now");

    // A session let go after its turn ended needs nothing closed for it.
    let started = feed
        .iter()
        .filter(|e| matches!(e, AgentEvent::TurnStarted { .. }))
        .count();
    let ended = feed
        .iter()
        .filter(|e| matches!(
            e,
            AgentEvent::TurnFinished { .. } | AgentEvent::TurnCancelled { .. }
        ))
        .count();
    assert_eq!(started, 1);
    assert_eq!(ended, 1);
}

#[tokio::test]
async fn a_duplicate_to_a_point_can_be_continued_from_there() {
    let dir = TempProject::new();
    let handle = recorded(&dir, replies(3), None);
    let mut rx = handle.subscribe();

    let mut turns = Vec::new();
    for text in ["one", "two", "three"] {
        ask(&handle, text).await;
        let seen = until(&mut rx, is_turn_over).await;
        turns.push(
            seen.iter()
                .find_map(|e| match e {
                    AgentEvent::TurnStarted { turn_id, .. } => Some(turn_id.clone()),
                    _ => None,
                })
                .expect("the turn should have started"),
        );
    }
    let id = handle.id().clone();
    drop(handle);

    let store = dir.store();
    let copy = store.duplicate(&id, Some(&turns[1])).unwrap();

    let provider = replies(1);
    let forked = recorded(&dir, provider.clone(), Some(copy.id.clone()));
    let mut rx = forked.subscribe();
    ask(&forked, "and like this").await;
    until(&mut rx, is_turn_over).await;

    // Two exchanges kept, the third left behind, and the new message on top.
    let sent = last_request(&provider);
    assert_eq!(sent.messages.len(), 5);
    assert_eq!(text_of(&sent.messages[0]), "one");
    assert_eq!(text_of(&sent.messages[2]), "two");
    assert_eq!(text_of(&sent.messages[4]), "and like this");
    assert!(sent.messages.iter().all(|m| text_of(m) != "three"));

    // The original is untouched, and both are in the panel.
    assert_eq!(store.meta(&id).unwrap().messages, 3);
    assert_eq!(store.list().len(), 2);
}

#[tokio::test]
async fn search_finds_a_finished_session_by_what_was_said() {
    let dir = TempProject::new();

    let one = recorded(&dir, replies(1), None);
    let mut rx = one.subscribe();
    ask(&one, "why does the date parser crash").await;
    until(&mut rx, is_turn_over).await;
    drop(one);

    let two = recorded(&dir, replies(1), None);
    let mut rx = two.subscribe();
    ask(&two, "something else entirely").await;
    until(&mut rx, is_turn_over).await;
    let other = two.id().clone();
    drop(two);

    let store = dir.store();
    let hits = store.search("crash", 10);
    assert_eq!(hits.len(), 1);
    assert_ne!(hits[0].session.id, other);

    // And the export is the conversation, not the event stream.
    let md = store.export_markdown(&hits[0].session.id).unwrap();
    assert!(md.contains("why does the date parser crash"));
    assert!(md.contains("answer0"));
}

// --- compaction (§5.11) -----------------------------------------------------

/// The kind of message that makes a conversation worth compacting: long
/// enough that a summary of it is shorter than it was.
fn long(prefix: &str) -> String {
    format!(
        "{prefix}: {}",
        "the date parser breaks at the time zone boundary.".repeat(20)
    )
}

/// Four exchanges: enough that two can be summarized and two kept.
async fn four_exchanges(dir: &TempProject) -> (SessionHandle, Arc<MockProvider>) {
    // Four answers, then one more for the summary itself.
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(script_text("answer0")),
        Turn::Events(script_text("answer1")),
        Turn::Events(script_text("answer2")),
        Turn::Events(script_text("answer3")),
        Turn::Events(script_text("We fixed the date parser; the parser is already patched.")),
    ]));
    let handle = recorded(dir, provider.clone(), None);
    let mut rx = handle.subscribe();

    for text in ["one", "two"] {
        ask(&handle, &long(text)).await;
        until(&mut rx, is_turn_over).await;
    }
    for text in ["three", "four"] {
        ask(&handle, text).await;
        until(&mut rx, is_turn_over).await;
    }
    (handle, provider)
}

#[tokio::test]
async fn compacting_replaces_the_history_with_a_summary() {
    let dir = TempProject::new();
    let (handle, provider) = four_exchanges(&dir).await;
    let mut rx = handle.subscribe();

    handle.send(Command::Compact).await.unwrap();
    let seen = until(&mut rx, |e| {
        matches!(e, AgentEvent::ContextCompacted { .. })
    })
    .await;

    let AgentEvent::ContextCompacted { summary, freed } = seen.last().unwrap() else {
        unreachable!("until() only returns on a match");
    };
    assert_eq!(summary, "We fixed the date parser; the parser is already patched.");
    assert!(*freed > 0, "§5.11 shows what was freed");

    // The summarizer is asked about a transcript, with no tools in the way.
    let asked = last_request(&provider);
    assert!(asked.tools.is_empty());
    assert!(!asked.thinking);
    assert_eq!(asked.messages.len(), 1);
    assert!(text_of(&asked.messages[0]).contains("Human: one:"));

    // The next turn goes out with the summary in place of what it replaced.
    let provider = replies(1);
    let again = recorded(&dir, provider.clone(), Some(handle.id().clone()));
    let mut rx = again.subscribe();
    ask(&again, "continue").await;
    until(&mut rx, is_turn_over).await;

    let sent = last_request(&provider);
    let head = text_of(&sent.messages[0]);
    assert!(head.starts_with(SUMMARY_HEADER), "{head}");
    assert!(head.contains("the parser is already patched"));
    // Two exchanges are kept word for word; "one" and "two" are gone.
    assert!(sent
        .messages
        .iter()
        .all(|m| !text_of(m).starts_with("one:")));
    assert!(sent.messages.iter().any(|m| text_of(m) == "four"));
}

#[tokio::test]
async fn the_summary_can_be_rewritten_before_continuing() {
    let dir = TempProject::new();
    let (handle, _) = four_exchanges(&dir).await;
    let mut rx = handle.subscribe();

    handle.send(Command::Compact).await.unwrap();
    until(&mut rx, |e| {
        matches!(e, AgentEvent::ContextCompacted { .. })
    })
    .await;

    handle
        .send(Command::EditSummary {
            text: "The parser is patched. Time zones are not touched yet.".into(),
        })
        .await
        .unwrap();
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::SummaryEdited { .. })).await;
    let AgentEvent::SummaryEdited { summary } = seen.last().unwrap() else {
        unreachable!("until() only returns on a match");
    };
    assert!(summary.contains("Time zones"));

    // §5.11: edited before continuing, so the next request carries the edit —
    // and so does the session when it is reopened.
    let provider = replies(1);
    let again = recorded(&dir, provider.clone(), Some(handle.id().clone()));
    let mut rx = again.subscribe();
    ask(&again, "continue").await;
    until(&mut rx, is_turn_over).await;

    let head = text_of(&last_request(&provider).messages[0]);
    assert!(head.contains("Time zones are not touched yet"), "{head}");
    assert!(!head.contains("We fixed the date parser"));
}

#[tokio::test]
async fn compacting_a_short_conversation_says_so_instead_of_doing_nothing() {
    let dir = TempProject::new();
    let handle = recorded(&dir, replies(1), None);
    let mut rx = handle.subscribe();

    ask(&handle, "one").await;
    until(&mut rx, is_turn_over).await;

    handle.send(Command::Compact).await.unwrap();
    let seen = until(&mut rx, |e| matches!(e, AgentEvent::Error { .. })).await;

    // A button that silently does nothing is an interface lying about state.
    let AgentEvent::Error { code, .. } = seen.last().unwrap() else {
        unreachable!("until() only returns on a match");
    };
    assert_eq!(code, "nothing_to_compact");
}
