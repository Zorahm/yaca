//! Compacting on its own, in the middle of a turn (§5.11, §6.1.1).
//!
//! The system prompt tells the model that the length of a conversation is
//! never a reason to stop, because long conversations are compacted when the
//! context fills up. An agent working through a task spends one turn on dozens
//! of calls, so "when the context fills up" has to include the middle of that
//! turn — and a request the provider refuses as too long has to be compacted
//! and sent again rather than end the turn.

mod common;

use std::collections::HashSet;
use std::sync::Arc;

use serde_json::json;
use yaca_core::error::ProviderError;
use yaca_core::event::{AgentEvent, StopReason};
use yaca_core::message::{ContentBlock, Message, Role};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::ProviderEvent;
use yaca_core::session::compact::SUMMARY_HEADER;
use yaca_core::usage::{ModelInfo, Pricing};
use yaca_core::{Mode, SessionConfig, SessionHandle};

use common::{ask, calls, is_turn_over, until, TempProject};

/// A window of 10 000 tokens with 1 000 kept for the answer: small enough to
/// fill in a test.
fn tiny_session(dir: &TempProject, provider: Arc<MockProvider>) -> SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Manual;
    cfg.journal = None;
    cfg.model = "tiny".into();
    cfg.models = vec![ModelInfo {
        id: "tiny".into(),
        provider_id: String::new(),
        display_name: "tiny".into(),
        context_max: 10_000,
        max_output: 1_000,
        pricing: Pricing {
            input: 0.0,
            output: 0.0,
        },
        supports_effort: false,
        supports_thinking: false,
        context_source: yaca_core::usage::ContextSource::ProviderReported,
        supports_images: true,
    }];
    yaca_core::spawn(cfg, provider).unwrap()
}

/// One step of a long turn: a read, with the window as full as `used` says.
fn read_step(id: &str, used: u32) -> Turn {
    // The limit differs per step: the same read four times with the same
    // answer is what the repeat check stops, and these tests are about the
    // window rather than about that.
    let mut events = calls(&[(id, "Read", json!({ "path": "a.rs", "limit": 1_000 + id.bytes().last().map_or(0, u32::from) }))]);
    if let Some(ProviderEvent::MessageStart { usage, .. }) = events.first_mut() {
        usage.input = used;
    }
    Turn::Events(events)
}

fn text_of(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn unanswered(messages: &[Message]) -> Vec<String> {
    let answered: HashSet<&str> = messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|b| match b {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
            _ => None,
        })
        .collect();
    messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|b| match b {
            ContentBlock::ToolUse { id, .. } if !answered.contains(id.as_str()) => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn position(events: &[AgentEvent], hit: impl Fn(&AgentEvent) -> bool) -> usize {
    events.iter().position(hit).expect("the event never came")
}

fn finished_with(events: &[AgentEvent]) -> Option<StopReason> {
    events.iter().find_map(|e| match e {
        AgentEvent::TurnFinished { stop_reason, .. } => Some(*stop_reason),
        _ => None,
    })
}

#[tokio::test]
async fn a_long_turn_is_compacted_between_its_steps() {
    let dir = TempProject::new();
    dir.write("a.rs", "fn main() {}\n");
    let provider = Arc::new(MockProvider::new(vec![
        read_step("c0", 1_000),
        read_step("c1", 2_000),
        read_step("c2", 3_000),
        // Past 90% of what the window leaves for input.
        read_step("c3", 8_500),
        Turn::Events(script_text("Read a.rs four times; nothing changed yet.")),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = tiny_session(&dir, provider.clone());
    let mut rx = handle.subscribe();

    ask(&handle, "read a.rs until you are sure").await;
    let seen = until(&mut rx, is_turn_over).await;

    // Still one turn, and it goes on after the compaction.
    let started = seen.iter().filter(|e| matches!(e, AgentEvent::TurnStarted { .. }));
    assert_eq!(started.count(), 1);
    assert_eq!(finished_with(&seen), Some(StopReason::EndTurn));
    let last_step = position(
        &seen,
        |e| matches!(e, AgentEvent::ToolCallFinished { call_id, .. } if call_id.0 == "c3"),
    );
    let compacted = position(&seen, |e| matches!(e, AgentEvent::ContextCompacted { .. }));
    assert!(last_step < compacted, "{last_step} {compacted}");

    let requests = provider.seen.lock().unwrap();
    assert_eq!(requests.len(), 6);
    // The summarizer reads the start of the turn, the request included.
    assert!(requests[4].tools.is_empty());
    assert!(text_of(&requests[4].messages[0]).contains("Human: read a.rs until you are sure"));

    // The turn continues from the summary and its last steps, word for word.
    let sent = &requests[5].messages;
    assert!(text_of(&sent[0]).starts_with(SUMMARY_HEADER));
    assert_eq!(sent[1].role, Role::Assistant);
    assert!(sent
        .iter()
        .flat_map(|m| &m.content)
        .any(|b| matches!(b, ContentBlock::ToolUse { id, .. } if id == "c3")));
    assert_eq!(unanswered(sent), Vec::<String>::new());
    assert!(sent.len() < requests[3].messages.len());
}

#[tokio::test]
async fn a_request_refused_as_too_long_is_compacted_and_sent_again() {
    let dir = TempProject::new();
    dir.write("a.rs", "fn main() {}\n");
    let provider = Arc::new(MockProvider::new(vec![
        read_step("c0", 100),
        read_step("c1", 100),
        read_step("c2", 100),
        read_step("c3", 100),
        // The estimate said it fits; the provider disagreed.
        Turn::Fail(ProviderError::Http {
            status: 400,
            message: "prompt is too long: 12000 tokens > 10000 maximum".into(),
            retry_after_s: None,
        }),
        Turn::Events(script_text("Read a.rs four times.")),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = tiny_session(&dir, provider.clone());
    let mut rx = handle.subscribe();

    ask(&handle, "read a.rs until you are sure").await;
    let seen = until(&mut rx, is_turn_over).await;

    // Cured, so nothing to report: the error never reaches the feed.
    assert!(!seen.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
    assert!(seen.iter().any(|e| matches!(e, AgentEvent::ContextCompacted { .. })));
    assert_eq!(finished_with(&seen), Some(StopReason::EndTurn));

    let requests = provider.seen.lock().unwrap();
    assert_eq!(requests.len(), 7);
    assert!(text_of(&requests[6].messages[0]).starts_with(SUMMARY_HEADER));
}

#[tokio::test]
async fn an_overflow_with_nothing_to_compact_ends_the_turn_with_the_error() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![Turn::Fail(ProviderError::Http {
        status: 400,
        message: "prompt is too long: 12000 tokens > 10000 maximum".into(),
        retry_after_s: None,
    })]));
    let handle = tiny_session(&dir, provider.clone());
    let mut rx = handle.subscribe();

    ask(&handle, "huge first message").await;
    let seen = until(&mut rx, is_turn_over).await;

    // No summary to make, so the provider's words are what the human gets.
    assert!(seen
        .iter()
        .any(|e| matches!(e, AgentEvent::Error { code, .. } if code == "http_400")));
    assert_eq!(finished_with(&seen), Some(StopReason::Failed));
    assert_eq!(provider.seen.lock().unwrap().len(), 1);
}

/// The window taken apart (§7.1): the split travels with the figure it
/// splits, and never disagrees with it.
#[tokio::test]
async fn the_usage_says_what_the_window_is_filled_with() {
    let dir = TempProject::new();
    dir.write("a.rs", "fn main() {}\n");
    dir.write("AGENTS.md", "Keep every function under forty lines.\n");
    let provider = Arc::new(MockProvider::new(vec![
        read_step("c0", 4_000),
        Turn::Events(script_text("Read it.")),
    ]));
    let handle = tiny_session(&dir, provider);
    let mut rx = handle.subscribe();

    ask(&handle, "read a.rs").await;
    let seen = until(&mut rx, is_turn_over).await;

    let usage = seen
        .iter()
        .find_map(|e| match e {
            AgentEvent::UsageUpdated { usage } => Some(*usage),
            _ => None,
        })
        .expect("no usage reported");
    let b = usage.breakdown;

    assert!(usage.context_used >= 4_000);
    assert_eq!(
        b.system_prompt + b.instructions + b.memory + b.skills + b.tools + b.mcp_tools
            + b.mcp_notes + b.messages,
        usage.context_used,
        "{b:?}"
    );
    assert!(b.system_prompt > 0 && b.tools > 0 && b.messages > 0, "{b:?}");
    assert!(b.instructions > 0, "AGENTS.md is in the prompt: {b:?}");
    // No MCP servers in this session, and nothing made up for them.
    assert_eq!((b.mcp_tools, b.mcp_notes), (0, 0));
    // 10 000 less the 1 000 kept for the answer, compacted at 90% of the rest.
    assert_eq!(b.reserved, 10_000 - 8_100);
}
