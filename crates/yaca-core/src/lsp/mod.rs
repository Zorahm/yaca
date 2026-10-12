//! The Language Server Protocol, client side.
//!
//! The model orients itself in code by `grep` and `Read`, which find text and
//! not meaning: the definition of the `parse` it is looking at, who calls it,
//! what type this is, what the compiler thinks of the edit just made. A
//! language server knows all of that, and the `Lsp` tool ([`crate::tool::lsp`])
//! is how the model asks it.
//!
//! What is here: definition, references, hover, document and workspace
//! symbols, and the diagnostics a server pushes — on request, and appended to
//! the result of an edit when a server for that file is already up. What is
//! not: rename, code actions, completion, formatting, pull diagnostics,
//! file watching. The README lists them.
//!
//! The layers, bottom up: [`wire`] frames JSON-RPC over a child process,
//! [`client`] is one connection with its handshake and document sync, [`hub`]
//! is the servers of a session and what state each is in, [`config`] says
//! which servers exist and for which files, [`format`] turns answers into text.

mod client;
pub mod config;
mod edits;
pub(crate) mod format;
mod hub;
mod wire;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

pub use config::LspServerSettings;
pub use hub::Lsp;

pub(crate) use client::Client;

/// Why a server could not be reached or did not answer. The text is for the
/// human: it lands in the server's state and in the row of a failed call.
#[derive(Debug, Clone, Error)]
pub enum LspError {
    #[error("{0}")]
    Transport(String),
    /// The server answered with a JSON-RPC error.
    #[error("{message} (код {code})")]
    Rpc { code: i64, message: String },
    #[error("сервер не ответил за {0} с")]
    Timeout(u64),
    #[error("отменено")]
    Cancelled,
    #[error("{0}")]
    Closed(String),
}

/// What state a server is in. The interface draws the list from this and
/// from nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum LspStatus {
    /// Known and not started: nothing has needed it yet.
    Idle,
    Starting,
    Ready,
    /// Could not start, or was up and went away. `reason` says which.
    Failed,
    Disabled,
}

/// One server of the session, as the interface lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LspServerSummary {
    pub name: String,
    /// The command line, as declared.
    pub command: String,
    pub extensions: Vec<String>,
    pub status: LspStatus,
    /// Why it failed, in the human's language.
    pub reason: Option<String>,
    /// How the server introduces itself: `name version`.
    pub label: Option<String>,
}
