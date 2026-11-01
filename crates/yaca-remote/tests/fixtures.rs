//! Writes the wire fixtures that the Android client decodes in its own tests.
//!
//! The phone's Kotlin types are written by hand from the Rust ones, so they can
//! drift apart without a compile error on either side. This test writes one
//! example of every wire variant into `android/core/src/test/resources/fixtures`,
//! one compact JSON value per line, and the Kotlin test decodes and re-encodes
//! every line. Running it rewrites those files, the way ts-rs regenerates
//! `src/core/bindings`: the files are committed, so a change in a Rust type
//! shows up as a diff in them. Everything written is fixed (ids, instants,
//! values), so a run that changes nothing leaves the files as they are.
//!
//! Exhaustiveness is checked twice. The name functions in `names.rs` and
//! `names_nested.rs` match every variant with no wildcard arm, so a new variant
//! stops this test from compiling until a fixture is built for it. Then the test
//! checks that the names of the written values are exactly the variant list in
//! `expected.rs`, and that each name is the tag serde writes.

#[path = "fixtures/agent_events.rs"]
mod agent_events;
#[path = "fixtures/agent_panels.rs"]
mod agent_panels;
#[path = "fixtures/commands.rs"]
mod commands;
#[path = "fixtures/expected.rs"]
mod expected;
#[path = "fixtures/ids.rs"]
mod ids;
#[path = "fixtures/messages.rs"]
mod messages;
#[path = "fixtures/names.rs"]
mod names;
#[path = "fixtures/names_nested.rs"]
mod names_nested;
#[path = "fixtures/nested_change.rs"]
mod nested_change;
#[path = "fixtures/nested_panels.rs"]
mod nested_panels;
#[path = "fixtures/nested_session.rs"]
mod nested_session;
#[path = "fixtures/variants.rs"]
mod variants;

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::fs;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;
use yaca_remote::protocol::{
    Attention, Call, CallResult, ClientMessage, ErrorCode, Refusal, Reply, ServerMessage,
};

use crate::names_nested as nested;

#[test]
fn write_wire_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../android/core/src/test/resources/fixtures");
    fs::create_dir_all(&dir).expect("create the fixtures directory");

    check_names(
        "StopReason",
        &variants::stop_reasons(),
        nested::stop_reason_name,
        None,
        expected::STOP_REASONS,
    );
    check_names(
        "PermissionKind",
        &variants::permission_kinds(),
        nested::permission_kind_name,
        None,
        expected::PERMISSION_KINDS,
    );
    check_names(
        "ScopeKey",
        &variants::scope_keys(),
        nested::scope_key_name,
        Some("type"),
        expected::SCOPE_KEYS,
    );
    check_names(
        "PermissionAnswer",
        &variants::permission_answers(),
        nested::permission_answer_name,
        Some("type"),
        expected::PERMISSION_ANSWERS,
    );
    check_names(
        "QuestionReply",
        &variants::question_replies(),
        nested::question_reply_name,
        Some("type"),
        expected::QUESTION_REPLIES,
    );
    check_names(
        "ElicitationReply",
        &variants::elicitation_replies(),
        nested::elicitation_reply_name,
        Some("type"),
        expected::ELICITATION_REPLIES,
    );
    check_names(
        "ProcessState",
        &variants::process_states(),
        nested::process_state_name,
        Some("type"),
        expected::PROCESS_STATES,
    );
    check_names(
        "ChangeKind",
        &variants::change_kinds(),
        nested::change_kind_name,
        None,
        expected::CHANGE_KINDS,
    );
    check_names(
        "CheckpointReason",
        &variants::checkpoint_reasons(),
        nested::checkpoint_reason_name,
        Some("type"),
        expected::CHECKPOINT_REASONS,
    );
    check_names(
        "Mode",
        &variants::modes(),
        nested::mode_name,
        None,
        expected::MODES,
    );
    check_names(
        "Effort",
        &variants::efforts(),
        nested::effort_name,
        None,
        expected::EFFORTS,
    );
    check_names(
        "LineKind",
        &variants::line_kinds(),
        nested::line_kind_name,
        None,
        expected::LINE_KINDS,
    );
    check_names(
        "McpStatus",
        &variants::mcp_statuses(),
        nested::mcp_status_name,
        None,
        expected::MCP_STATUSES,
    );
    check_names(
        "LspStatus",
        &variants::lsp_statuses(),
        nested::lsp_status_name,
        None,
        expected::LSP_STATUSES,
    );
    check_names(
        "TodoStatus",
        &variants::todo_statuses(),
        nested::todo_status_name,
        None,
        expected::TODO_STATUSES,
    );
    check_names(
        "ViewSegment",
        &nested_change::view_segments(),
        nested::view_segment_name,
        Some("kind"),
        expected::VIEW_SEGMENTS,
    );
    check_names(
        "ContextSource",
        &variants::context_sources(),
        nested::context_source_name,
        None,
        expected::CONTEXT_SOURCES,
    );
    check_names(
        "Invariant",
        &variants::invariants(),
        nested::invariant_name,
        None,
        expected::INVARIANTS,
    );
    check_names(
        "ElicitKind",
        &variants::elicit_kinds(),
        nested::elicit_kind_name,
        None,
        expected::ELICIT_KINDS,
    );
    check_names(
        "McpSource",
        &variants::mcp_sources(),
        nested::mcp_source_name,
        None,
        expected::MCP_SOURCES,
    );
    check_names(
        "McpTransport",
        &variants::mcp_transports(),
        nested::mcp_transport_name,
        None,
        expected::MCP_TRANSPORTS,
    );

    let events = agent_events::all();
    let commands = commands::all();
    let client = messages::client_messages();
    let server = messages::server_messages();

    let calls: Vec<Call> = client
        .iter()
        .filter_map(|message| match message {
            ClientMessage::Request { call, .. } => Some(call.clone()),
            _ => None,
        })
        .collect();
    let results: Vec<CallResult> = server
        .iter()
        .filter_map(|message| match message {
            ServerMessage::Response { result, .. } => Some(result.clone()),
            _ => None,
        })
        .collect();
    let replies: Vec<Reply> = results
        .iter()
        .filter_map(|result| match result {
            CallResult::Ok { reply } => Some(reply.clone()),
            CallResult::Error { .. } => None,
        })
        .collect();
    let error_codes: Vec<ErrorCode> = results
        .iter()
        .filter_map(|result| match result {
            CallResult::Error { error } => Some(error.code),
            CallResult::Ok { .. } => None,
        })
        .collect();
    let refusals: Vec<Refusal> = server
        .iter()
        .filter_map(|message| match message {
            ServerMessage::Refused { reason } => Some(reason.clone()),
            _ => None,
        })
        .collect();
    let attentions: Vec<Attention> = server
        .iter()
        .filter_map(|message| match message {
            ServerMessage::Attention { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect();

    check_names(
        "AgentEvent",
        &events,
        names::agent_event_name,
        Some("type"),
        expected::AGENT_EVENTS,
    );
    check_names(
        "Command",
        &commands,
        names::command_name,
        Some("type"),
        expected::COMMANDS,
    );
    check_names(
        "ClientMessage",
        &client,
        names::client_message_name,
        Some("type"),
        expected::CLIENT_MESSAGES,
    );
    check_names("Call", &calls, names::call_name, Some("kind"), expected::CALLS);
    check_names(
        "ServerMessage",
        &server,
        names::server_message_name,
        Some("type"),
        expected::SERVER_MESSAGES,
    );
    check_names("Reply", &replies, names::reply_name, Some("kind"), expected::REPLIES);
    check_names(
        "CallResult",
        &results,
        names::call_result_name,
        Some("status"),
        expected::CALL_RESULTS,
    );
    check_names(
        "ErrorCode",
        &error_codes,
        nested::error_code_name,
        None,
        expected::ERROR_CODES,
    );
    check_names(
        "Refusal",
        &refusals,
        names::refusal_name,
        Some("kind"),
        expected::REFUSALS,
    );
    check_names(
        "Attention",
        &attentions,
        names::attention_name,
        Some("kind"),
        expected::ATTENTIONS,
    );

    write_jsonl(&dir, "agent_events.jsonl", &events);
    write_jsonl(&dir, "commands.jsonl", &commands);
    write_jsonl(&dir, "client_messages.jsonl", &client);
    write_jsonl(&dir, "server_messages.jsonl", &server);
}

/// The names of `values`, checked two ways: each name must be the tag serde
/// writes (under `tag` for an object, or the value itself for a plain string),
/// and the set of names must be exactly `listed`.
fn check_names<T: Serialize>(
    what: &str,
    values: &[T],
    name: fn(&T) -> &'static str,
    tag: Option<&str>,
    listed: &[&'static str],
) {
    let seen: BTreeSet<&'static str> = values
        .iter()
        .map(|value| {
            let wire = serde_json::to_value(value).expect("serialize a value");
            let wire_name = match tag {
                Some(key) => wire.get(key).and_then(serde_json::Value::as_str),
                None => wire.as_str(),
            };
            let expected = name(value);
            assert_eq!(
                wire_name,
                Some(expected),
                "{what}: the name function disagrees with the wire"
            );
            expected
        })
        .collect();
    let listed: BTreeSet<&'static str> = listed.iter().copied().collect();
    assert_eq!(
        seen, listed,
        "{what}: the written values do not match the variant list"
    );
}

/// One compact JSON value per line, each read back and compared with the value
/// it came from before it is written.
fn write_jsonl<T>(dir: &Path, file: &str, values: &[T])
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let mut text = String::new();
    for value in values {
        let line = serde_json::to_string(value).expect("serialize a fixture");
        let back: T = serde_json::from_str(&line).expect("read a fixture line back");
        assert_eq!(
            &back, value,
            "{file}: a fixture line does not read back as the value it came from"
        );
        text.push_str(&line);
        text.push('\n');
    }
    fs::write(dir.join(file), text).expect("write a fixture file");
}
