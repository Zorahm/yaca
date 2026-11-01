//! The messages of the wire itself: what a phone sends (the handshake and the
//! requests) and what the host sends back (the handshake answers, the replies,
//! the errors, the frames, the chats and the attention). Every call, reply,
//! error code, refusal and attention kind is written at least once.

use yaca_core::event::AgentEvent;
use yaca_core::Command;
use yaca_remote::protocol::{
    CallError, CallResult, Call, ChatSummary, ClientMessage, DeviceId, HostInfo, Reply,
    ServerMessage, Startup, PROTOCOL_VERSION,
};
use yaca_core::{Effort, Mode};

use crate::ids::{self, PROJECT};
use crate::nested_session;
use crate::variants;

fn host_info() -> HostInfo {
    HostInfo {
        name: "Desk-PC".to_owned(),
        version: "1.0.0-beta".to_owned(),
        protocol: PROTOCOL_VERSION,
    }
}

fn startup() -> Startup {
    Startup {
        mode: Mode::Auto,
        provider: "anthropic".to_owned(),
        model: "model-0".to_owned(),
        effort: Effort::High,
    }
}

/// The handshake and every request a phone can make. Each call appears once as
/// a request; `OpenChat` also appears with and without `resume` and `startup`.
pub fn client_messages() -> Vec<ClientMessage> {
    let mut messages = vec![
        ClientMessage::Pair {
            code: "482913".to_owned(),
            device_name: "Pixel 8".to_owned(),
            protocol: PROTOCOL_VERSION,
        },
        ClientMessage::Hello {
            device_id: DeviceId("dev-1".to_owned()),
            token: "token-not-real".to_owned(),
            protocol: PROTOCOL_VERSION,
        },
    ];
    messages.extend(calls().into_iter().enumerate().map(|(i, call)| {
        ClientMessage::Request {
            id: i as u64 + 1,
            call,
        }
    }));
    messages
}

pub fn calls() -> Vec<Call> {
    let project = PROJECT.to_owned();
    vec![
        Call::ListProjects,
        Call::ListSessions {
            project: project.clone(),
        },
        Call::ListModels,
        Call::ListChats,
        Call::SessionFeed {
            project: project.clone(),
            session: ids::session(1),
        },
        Call::OpenChat {
            project: project.clone(),
            resume: None,
            startup: None,
        },
        Call::OpenChat {
            project: project.clone(),
            resume: Some(ids::session(1)),
            startup: None,
        },
        Call::OpenChat {
            project: project.clone(),
            resume: None,
            startup: Some(startup()),
        },
        Call::OpenChat {
            project,
            resume: Some(ids::session(2)),
            startup: Some(startup()),
        },
        Call::Attach {
            session: ids::session(1),
        },
        Call::Detach {
            session: ids::session(1),
        },
        Call::CloseChat {
            session: ids::session(1),
        },
        Call::Send {
            session: ids::session(1),
            command: Command::SendMessage {
                text: "Make greet() say hello to the world.".to_owned(),
                attachments: vec![],
            },
        },
        Call::Send {
            session: ids::session(1),
            command: Command::Cancel,
        },
    ]
}

/// Every reply a request can get, each with a realistic body.
pub fn replies() -> Vec<Reply> {
    vec![
        Reply::Projects {
            projects: vec![PROJECT.to_owned(), "yaca-site".to_owned()],
        },
        Reply::Sessions {
            sessions: nested_session::session_metas(),
        },
        Reply::Models {
            models: nested_session::model_infos(),
        },
        Reply::Chats {
            chats: nested_session::chat_summaries(),
        },
        Reply::Feed {
            events: short_turn(),
        },
        Reply::Opened {
            chat: Box::new(nested_session::chat_summaries().remove(0)),
        },
        Reply::Attached {
            session: ids::session(1),
            epoch: 2,
            events: short_turn(),
        },
        Reply::Done,
    ]
}

/// The short turn a frame carries: a message, a turn that reads a file and
/// answers, and its end.
pub fn short_turn() -> Vec<AgentEvent> {
    vec![
        AgentEvent::UserMessage {
            turn_id: ids::turn(1),
            text: "Make greet() say hello to the world.".to_owned(),
            attachments: vec![],
        },
        AgentEvent::TurnStarted {
            turn_id: ids::turn(1),
            at: ids::at(0),
        },
        AgentEvent::TextDelta {
            turn_id: ids::turn(1),
            block_id: ids::block(1),
            text: "I will read the file ".to_owned(),
        },
        AgentEvent::TextDelta {
            turn_id: ids::turn(1),
            block_id: ids::block(1),
            text: "before I change it.".to_owned(),
        },
        AgentEvent::ToolCallStarted {
            turn_id: ids::turn(1),
            call_id: ids::call(1),
            tool: "read_file".to_owned(),
            description: Some("Read greet.rs".to_owned()),
            target: Some("src/greet.rs".to_owned()),
            input: serde_json::json!({"path": "src/greet.rs"}),
            parent: None,
        },
        AgentEvent::ToolCallFinished {
            call_id: ids::call(1),
            meta: yaca_core::event::ToolMeta {
                lines: 4,
                items: None,
                truncated: false,
                duration_ms: 12,
                pictures: vec![],
            },
        },
        AgentEvent::TurnFinished {
            turn_id: ids::turn(1),
            stop_reason: yaca_core::StopReason::EndTurn,
            at: ids::at(4_000),
        },
    ]
}

/// The chats of the host, for the list of live chats when it changes.
fn chats() -> Vec<ChatSummary> {
    nested_session::chat_summaries()
}

/// The answers from the server: the handshake, every reply and every error,
/// refusals, a frame of a turn, a resync, the chats, and the attention kinds.
pub fn server_messages() -> Vec<ServerMessage> {
    let mut messages = vec![
        ServerMessage::Paired {
            device_id: DeviceId("dev-1".to_owned()),
            token: "token-not-real".to_owned(),
            host: host_info(),
        },
        ServerMessage::Welcome { host: host_info() },
    ];
    messages.extend(
        variants::refusals()
            .into_iter()
            .map(|reason| ServerMessage::Refused { reason }),
    );
    messages.extend(replies().into_iter().enumerate().map(|(i, reply)| {
        ServerMessage::Response {
            id: i as u64 + 1,
            result: CallResult::Ok { reply },
        }
    }));
    messages.extend(variants::error_codes().into_iter().enumerate().map(|(i, code)| {
        ServerMessage::Response {
            id: 100 + i as u64,
            result: CallResult::Error {
                error: CallError {
                    code,
                    message: "The chat is not open on this host.".to_owned(),
                },
            },
        }
    }));
    messages.push(ServerMessage::Frame {
        session: ids::session(1),
        epoch: 1,
        events: short_turn(),
    });
    messages.push(ServerMessage::Resync {
        session: ids::session(1),
    });
    messages.push(ServerMessage::ChatsChanged { chats: chats() });
    messages.extend(
        variants::attentions()
            .into_iter()
            .enumerate()
            .map(|(i, reason)| ServerMessage::Attention {
                session: ids::session(i as u32 + 1),
                project: PROJECT.to_owned(),
                title: (i % 2 == 0).then(|| "Greeting rename".to_owned()),
                reason,
            }),
    );
    messages
}
