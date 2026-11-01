//! The wire: JSON text messages over a WebSocket inside TLS.
//!
//! A connection opens with a handshake: [`ClientMessage::Pair`] from a device
//! that has only the code out of the QR, or [`ClientMessage::Hello`] from one
//! that was paired before. Nothing else is read until one of them succeeds, and
//! a failed one gets [`ServerMessage::Refused`] and a closed socket.
//!
//! After it, the client sends [`ClientMessage::Request`]s and gets one
//! [`ServerMessage::Response`] for each, matched by `id`. Everything else the
//! server sends is pushed: frames of the chats the client attached to, the list
//! of live chats when it changes, and the moments a chat needs the human.
//!
//! The session's own vocabulary is not translated: [`Call::Send`] carries a
//! core [`Command`] and [`ServerMessage::Frame`] carries core [`AgentEvent`]s,
//! in the serde shapes the desktop window folds. A client folds the same feed.

use serde::{Deserialize, Serialize};
use yaca_core::event::{AgentEvent, SessionState};
use yaca_core::id::{RequestId, SessionId};
use yaca_core::usage::ModelInfo;
use yaca_core::{Command, Effort, Mode, SessionMeta, StopReason};

/// Raised on any change a client of the previous version would misread. A
/// handshake with another number is refused with
/// [`Refusal::ProtocolMismatch`], so an old phone says "update" instead of
/// folding a feed it does not understand.
pub const PROTOCOL_VERSION: u32 = 1;

/// A paired device. Opaque to the client: it stores what [`ServerMessage::Paired`]
/// gave it and sends it back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
    /// The first message of a device with no token yet. `code` is the one-time
    /// code from the pairing QR; `device_name` is what the host lists the
    /// device as ("Pixel 8").
    #[serde(rename_all = "camelCase")]
    Pair {
        code: String,
        device_name: String,
        protocol: u32,
    },
    /// The first message of a device that was paired before.
    #[serde(rename_all = "camelCase")]
    Hello {
        device_id: DeviceId,
        token: String,
        protocol: u32,
    },
    /// Anything after the handshake. `id` is the client's own, echoed in the
    /// response; the server does not require it to be unique or increasing.
    Request { id: u64, call: Call },
}

/// What a request asks for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Call {
    /// The host's projects, most recent first. → [`Reply::Projects`]
    ListProjects,
    /// The recorded chats of a project. → [`Reply::Sessions`]
    ListSessions { project: String },
    /// Every model the host's providers offer, for the model picker. → [`Reply::Models`]
    ListModels,
    /// The chats running on the host right now. → [`Reply::Chats`]
    ListChats,
    /// The feed of a recorded chat that is not running, read from its journal.
    /// → [`Reply::Feed`]
    SessionFeed { project: String, session: SessionId },
    /// Starts a chat: a new one, or a recorded one when `resume` names it.
    /// Opening a chat that is already running hands back the running one.
    /// → [`Reply::Opened`]
    OpenChat {
        project: String,
        #[serde(default)]
        resume: Option<SessionId>,
        /// What a new chat starts with; `None` leaves it to the project's
        /// settings, as the host would for a chat opened there.
        #[serde(default)]
        startup: Option<Startup>,
    },
    /// Starts sending the chat's events. The reply is the whole feed so far;
    /// frames with the same `epoch` follow. Attaching again restarts this with
    /// a new epoch, and frames of an older one are to be dropped.
    /// → [`Reply::Attached`]
    Attach { session: SessionId },
    /// Stops sending the chat's events. The chat goes on running. → [`Reply::Done`]
    Detach { session: SessionId },
    /// Stops the turn and closes the chat on the host, for every client and
    /// the host's own window. The journal stays. → [`Reply::Done`]
    CloseChat { session: SessionId },
    /// A core command for a running chat. Its effect arrives as events of an
    /// attached chat, not in the reply. → [`Reply::Done`]
    Send { session: SessionId, command: Command },
}

/// What a new chat starts with, chosen before it exists. The same four the
/// desktop's home screen picks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Startup {
    pub mode: Mode,
    pub provider: String,
    pub model: String,
    pub effort: Effort,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    /// The answer to a successful [`ClientMessage::Pair`]. The token is shown
    /// this once: the host keeps only its hash.
    #[serde(rename_all = "camelCase")]
    Paired {
        device_id: DeviceId,
        token: String,
        host: HostInfo,
    },
    /// The answer to a successful [`ClientMessage::Hello`].
    Welcome { host: HostInfo },
    /// The handshake failed. The socket closes after this.
    Refused { reason: Refusal },
    Response { id: u64, result: CallResult },
    /// Events of an attached chat, in order, batched per frame with
    /// consecutive deltas of one block merged.
    Frame {
        session: SessionId,
        epoch: u32,
        events: Vec<AgentEvent>,
    },
    /// This connection fell behind the chat's stream and events were lost.
    /// The server has stopped sending the chat; the client attaches again to
    /// get a whole feed.
    Resync { session: SessionId },
    /// The set of chats running on the host changed: one opened or closed,
    /// here, in another client or in the host's window.
    ChatsChanged { chats: Vec<ChatSummary> },
    /// A running chat needs the human, or finished its turn. Sent for every
    /// running chat, attached or not: this is what the phone notifies about.
    Attention {
        session: SessionId,
        project: String,
        title: Option<String>,
        reason: Attention,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInfo {
    /// The machine's name, for the phone's list of hosts.
    pub name: String,
    /// The Yaca version the host runs.
    pub version: String,
    pub protocol: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Refusal {
    /// Unknown device, wrong token, or a device that was revoked. One answer
    /// for all three: telling them apart tells a guesser which part was right.
    Unauthorized,
    /// The pairing code is wrong, used or expired.
    PairingFailed,
    /// The client speaks another version of this protocol.
    ProtocolMismatch { server: u32 },
    /// Too many failed handshakes from this address; try again later.
    TooManyAttempts,
    /// The first message was not a handshake, or did not arrive in time.
    HandshakeExpected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum CallResult {
    Ok { reply: Reply },
    Error { error: CallError },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Reply {
    Projects { projects: Vec<String> },
    Sessions { sessions: Vec<SessionMeta> },
    Models { models: Vec<ModelInfo> },
    Chats { chats: Vec<ChatSummary> },
    Feed { events: Vec<AgentEvent> },
    /// Boxed: a summary is several times the size of every other reply, and
    /// each response would carry that size otherwise.
    Opened { chat: Box<ChatSummary> },
    Attached {
        session: SessionId,
        epoch: u32,
        events: Vec<AgentEvent>,
    },
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallError {
    pub code: ErrorCode,
    /// English, for the human: the client shows it as it is.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// The call names a chat that is not running on the host.
    ChatNotOpen,
    /// The request could not be read (unknown call, missing field).
    BadRequest,
    /// The host tried and failed: a project folder that is gone, a provider
    /// that does not resolve, a journal that cannot be read.
    Host,
}

/// A chat running on the host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSummary {
    pub id: SessionId,
    /// The project as the host's list names it.
    pub project: String,
    pub state: SessionState,
    /// Title and cost; `None` for a chat that is not written down.
    pub meta: Option<SessionMeta>,
}

/// Why a chat wants the human.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Attention {
    /// The agent asks for permission; `target` is the request's short target
    /// (a file name, a command's first word, a domain). `request` lets the
    /// phone answer from the notification itself, without opening the chat.
    Permission { request: RequestId, target: String },
    /// The agent asked a question.
    Question { text: String },
    /// An MCP server asks the human to fill in a form.
    Elicitation,
    /// The turn ended (not cancelled): the agent is done and waits.
    #[serde(rename_all = "camelCase")]
    TurnFinished { stop_reason: StopReason },
}
