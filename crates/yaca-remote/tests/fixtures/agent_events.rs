//! The events of a conversation: what the human sent, what the model wrote, the
//! tools it ran and how each ended, the turn's end, and the lines the feed shows
//! about the session itself. `all` adds the panel events from `agent_panels`.

use serde_json::json;
use yaca_core::change::ChangeKind;
use yaca_core::event::{AgentEvent, ToolMeta};

use crate::agent_panels;
use crate::ids;
use crate::nested_change;
use crate::nested_panels;
use crate::nested_session;
use crate::variants;

pub fn all() -> Vec<AgentEvent> {
    let mut events = conversation();
    events.extend(agent_panels::all());
    events
}

fn tool_meta(lines: u32, items: Option<u32>, truncated: bool, pictures: Vec<String>) -> ToolMeta {
    ToolMeta {
        lines,
        items,
        truncated,
        duration_ms: 12 + lines * 7,
        pictures,
    }
}

const GREET_TEXT: &str = "pub fn greet() -> &'static str {\n    \"hello\"\n}\n";

fn conversation() -> Vec<AgentEvent> {
    let mut events = vec![
        // The human's words, once plain and once with a file attached.
        AgentEvent::UserMessage {
            turn_id: ids::turn(1),
            text: "Make greet() say hello to the world.".to_owned(),
            attachments: vec![],
        },
        AgentEvent::UserMessage {
            turn_id: ids::turn(2),
            text: "Check this file too.".to_owned(),
            attachments: vec!["src/greet.rs".to_owned()],
        },
        AgentEvent::TurnStarted {
            turn_id: ids::turn(1),
            at: ids::at(0),
        },
        AgentEvent::ThinkingDelta {
            turn_id: ids::turn(1),
            block_id: ids::block(2),
            text: "The greeting is a constant, so it can change in one place.".to_owned(),
        },
        AgentEvent::TextDelta {
            turn_id: ids::turn(1),
            block_id: ids::block(1),
            text: "I will read ".to_owned(),
        },
        AgentEvent::TextDelta {
            turn_id: ids::turn(1),
            block_id: ids::block(1),
            text: "src/greet.rs first.".to_owned(),
        },
        // Every optional field set, then every one unset, then a subagent's call
        // under a Task row (parent set).
        AgentEvent::ToolCallStarted {
            turn_id: ids::turn(1),
            call_id: ids::call(1),
            tool: "read_file".to_owned(),
            description: Some("Read greet.rs".to_owned()),
            target: Some("src/greet.rs".to_owned()),
            input: json!({"path": "src/greet.rs"}),
            parent: None,
        },
        AgentEvent::ToolCallStarted {
            turn_id: ids::turn(1),
            call_id: ids::call(2),
            tool: "edit_file".to_owned(),
            description: Some("Edit greet.rs".to_owned()),
            target: Some("src/greet.rs".to_owned()),
            input: json!({"path": "src/greet.rs", "old": "hello", "new": "hello, world"}),
            parent: Some(ids::call(1)),
        },
        AgentEvent::ToolCallStarted {
            turn_id: ids::turn(1),
            call_id: ids::call(3),
            tool: "grep".to_owned(),
            description: None,
            target: None,
            input: json!({"pattern": "hello"}),
            parent: None,
        },
        AgentEvent::ToolCallProgress {
            call_id: ids::call(1),
            chunk: GREET_TEXT.to_owned(),
        },
        AgentEvent::ToolCallFinished {
            call_id: ids::call(1),
            meta: tool_meta(4, None, false, vec![]),
        },
        AgentEvent::ToolCallFinished {
            call_id: ids::call(3),
            meta: tool_meta(2, Some(3), true, vec!["shots/greet.png".to_owned()]),
        },
        AgentEvent::ToolCallFailed {
            call_id: ids::call(2),
            message: "src/missing.rs: no such file".to_owned(),
        },
        AgentEvent::ToolCallRejected {
            call_id: ids::call(4),
            reason: "path escapes the project".to_owned(),
        },
        AgentEvent::ToolCallDenied {
            call_id: ids::call(5),
            reason: Some("Use the helper module.".to_owned()),
            stopped: false,
        },
        AgentEvent::ToolCallDenied {
            call_id: ids::call(6),
            reason: None,
            stopped: true,
        },
        AgentEvent::TodosUpdated {
            items: nested_panels::todo_items(),
        },
        AgentEvent::TitleSet {
            title: "Greeting rename".to_owned(),
        },
        AgentEvent::TextShown {
            path: "src/greet.rs".to_owned(),
            command: None,
            first_line: 1,
            lines: 3,
            total: 3,
            text: GREET_TEXT.to_owned(),
        },
        AgentEvent::TextShown {
            path: "command 3".to_owned(),
            command: Some("cargo test".to_owned()),
            first_line: 1,
            lines: 2,
            total: 40,
            text: "running 2 tests\ntest greet ... ok\n".to_owned(),
        },
        AgentEvent::PlanProposed {
            plan: "1. Read src/greet.rs\n2. Change the greeting\n3. Run the tests".to_owned(),
        },
        AgentEvent::SubagentUsage {
            call_id: ids::call(1),
            tokens: nested_session::token_counts(2),
        },
        AgentEvent::RouteDecided {
            route: Some("long-context".to_owned()),
            provider: "anthropic".to_owned(),
            model: "model-0".to_owned(),
            matched: vec!["context>100k".to_owned()],
        },
        AgentEvent::RouteDecided {
            route: None,
            provider: "anthropic".to_owned(),
            model: "model-1".to_owned(),
            matched: vec![],
        },
        AgentEvent::UsageUpdated {
            usage: nested_session::usage_snapshot(),
        },
        AgentEvent::TurnCancelled {
            turn_id: ids::turn(3),
            at: ids::at(5_000),
        },
        AgentEvent::Error {
            code: "rate_limited".to_owned(),
            message: "Too many requests; the provider asked to wait.".to_owned(),
            retryable: true,
        },
        AgentEvent::Error {
            code: "invalid_request".to_owned(),
            message: "The model does not accept this input.".to_owned(),
            retryable: false,
        },
        AgentEvent::RetryScheduled {
            attempt: 2,
            of: 5,
            delay_ms: 4_000,
            reason: "rate_limited".to_owned(),
        },
        AgentEvent::ContextCompacted {
            summary: "The human asked for the greeting to say hello to the world.".to_owned(),
            freed: 84_000,
        },
        AgentEvent::SummaryEdited {
            summary: "The human asked for the greeting to say hello, world.".to_owned(),
        },
        AgentEvent::FilesFound {
            query: "gre".to_owned(),
            paths: vec!["src/greet.rs".to_owned()],
        },
        AgentEvent::HookReported {
            plugin: "lint-on-stop".to_owned(),
            event: "Stop".to_owned(),
            text: "2 warnings".to_owned(),
            blocked: false,
        },
        AgentEvent::HookReported {
            plugin: "guard".to_owned(),
            event: "UserPromptSubmit".to_owned(),
            text: "The prompt contains a token.".to_owned(),
            blocked: true,
        },
    ];

    // The same edit under every kind of file change, the moved one with its origin.
    events.extend(
        variants::change_kinds()
            .into_iter()
            .map(|kind| AgentEvent::FileChanged {
                call_id: ids::call(1),
                fact: nested_change::fact("src/greet.rs", kind),
                diff: nested_change::greet_diff(),
            }),
    );
    events.push(AgentEvent::FileChanged {
        call_id: ids::call(7),
        fact: nested_change::fact("src/big.rs", ChangeKind::Modified),
        diff: nested_change::cut_diff(),
    });

    // A turn's end under every stop reason; the state ring for every mode and effort.
    events.extend(
        variants::stop_reasons()
            .into_iter()
            .enumerate()
            .map(|(i, stop_reason)| AgentEvent::TurnFinished {
                turn_id: ids::turn(i as u32 + 1),
                stop_reason,
                at: ids::at(10_000 * (i as u64 + 1)),
            }),
    );
    events.extend(
        nested_session::session_states()
            .into_iter()
            .map(|state| AgentEvent::StateChanged { state }),
    );

    events
}
