//! The Model Context Protocol, client side.
//!
//! A session connects to the servers its configuration declares ([`config`]),
//! and each server's tools become tools of the session ([`tool`]): the same
//! registry, the same hooks, the same question to the human before a call that
//! changes something, the same row in the feed. Nothing about a call tells the
//! rest of the core that the work is done by another process.
//!
//! What is here: tools, resources, the server's instructions, a tool list
//! that changes while the server runs, and roots. What is not: prompts,
//! sampling, elicitation, and OAuth for remote servers. The README lists
//! those.
//!
//! The layers, bottom up: [`transport`] moves JSON-RPC messages, [`client`]
//! is one connection with its handshake, [`hub`] is the servers of a session
//! and what state each is in.

pub mod app;
mod approval;
mod client;
pub mod elicit;
pub mod oauth;
pub mod config;
mod hub;
mod protocol;
mod resources;
mod tool;
pub(crate) mod transport;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

pub use app::{AppServer, AppServerSpec, ServerEnd};
pub use config::{McpServerSettings, McpSource, McpTransport, PluginLaunch, ServerSpec};
pub use elicit::{ElicitField, ElicitKind, ElicitationReply, ElicitationRequest};
pub use hub::Mcp;
pub use tool::PREFIX as TOOL_PREFIX;

/// Names of the tools that are not a server's own but the client's, for
/// reading what servers expose as resources.
pub const RESOURCE_TOOLS: [&str; 2] = [resources::LIST, resources::READ];

/// Why a server could not be reached or did not answer.
///
/// The text is for the human: it lands in the server's state and in the row of
/// a failed call.
#[derive(Debug, Clone, Error)]
pub enum McpError {
    #[error("{0}")]
    Transport(String),
    #[error("{0}")]
    Protocol(String),
    /// The server answered with a JSON-RPC error.
    #[error("{message} (code {code})")]
    Rpc { code: i64, message: String },
    #[error("the server did not answer within {0} s")]
    Timeout(u64),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Closed(String),
    /// The server answered 401: it wants a login. Carries its
    /// `WWW-Authenticate`, which may say where to log in.
    #[error("the server asks to log in")]
    Unauthorized(Option<String>),
}

/// What state a server is in. The interface draws the list from this and from
/// nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum McpStatus {
    Connecting,
    Ready,
    /// Could not start, or was up and went away. `reason` says which.
    Failed,
    /// Declared with `disabled`.
    Disabled,
    /// Declared in the project's `.mcp.json` and not approved by the human
    /// yet: nothing has been started.
    NeedsApproval,
    /// A remote server that answered 401: the human has to log in (OAuth).
    NeedsLogin,
    /// A login is under way in the browser; `login_url` is its page.
    LoggingIn,
}

/// One server of the session, as the interface lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSummary {
    pub name: String,
    pub source: McpSource,
    pub transport: McpTransport,
    /// The command line or the address, as declared.
    pub target: String,
    pub status: McpStatus,
    /// Why it failed, in the human's language.
    pub reason: Option<String>,
    /// How the server introduces itself: `name version`.
    pub label: Option<String>,
    pub tools: Vec<McpToolSummary>,
    /// The server exposes resources, so the two resource tools reach it.
    pub resources: bool,
    /// Its prompt templates, each a slash command.
    pub prompts: Vec<McpPromptSummary>,
    /// While logging in: the page to open in the browser.
    pub login_url: Option<String>,
    /// Yaca holds an OAuth login for this server, which "Log out" forgets.
    pub logged_in: bool,
}

/// A server's prompt template, as the slash menu offers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptSummary {
    /// What is typed after the slash: `mcp__server__prompt`.
    pub command: String,
    pub description: Option<String>,
    /// The arguments, in the order words typed after the name fill them.
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct McpToolSummary {
    /// The tool's own name, as the server calls it.
    pub name: String,
    pub description: Option<String>,
    /// The server marks it as changing nothing, so it runs without a question.
    pub read_only: bool,
}
