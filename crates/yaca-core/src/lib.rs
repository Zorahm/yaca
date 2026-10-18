//! The Yaca core.
//!
//! Headless: no window, no webview, no Tauri. Out goes the stream of [`AgentEvent`],
//! in comes [`Command`]. The interface is a pure function of that stream, and
//! this can be checked without the interface: `yaca-cli` runs exactly the same core.
//!
//! Currently covered: whole conversations, staging of edits, the §8 invariants,
//! permissions and command execution: reading and writing tools, the set of unaccepted
//! edits with their diffs, applying them to the working tree with a conflict check,
//! the trust matrix with its requests, commands with streamed output, and background
//! processes with the §6.5 panel.
//!
//! Checkpoints (§5.8) are in as well: a shadow store of its own under
//! `.yaca/shadow`, a snapshot before each message, before the change set
//! reaches disk and before every permitted command, and a rollback that names
//! the files it will touch before it touches them.
//!
//! Storage is in too (§9): three append-only files per session in the
//! application's chats folder (`store::location`), a rebuildable index behind the panel, search over the
//! conversation and over the paths a session changed, and a session that
//! reopens with the history the model actually had. Alongside it,
//! `.yaca/settings.json` (§10), provider keys in the system keychain, and
//! history compaction with a summary the human can rewrite (§5.11).

pub mod agents;
pub mod change;
pub mod checkpoint;
pub mod command;
pub mod error;
pub mod event;
pub mod id;
pub mod instructions;
pub mod intent;
pub mod isolation;
pub mod legacy;
pub mod lsp;
pub mod mcp;
pub mod mention;
pub mod message;
pub mod permission;
pub mod pending;
pub mod picture;
pub mod plugin;
pub mod process;
pub mod project;
pub mod provider;
pub mod question;
pub mod router;
pub mod secrets;
pub mod session;
pub mod settings;
pub mod skills;
pub mod slash;
pub mod store;
pub mod tool;
pub mod usage;
pub mod workspace;

mod hooks;
mod uri;
mod verify;

#[cfg(test)]
mod testutil;

pub use change::ChangeKind;
pub use checkpoint::{CheckpointChange, CheckpointReason, CheckpointSummary, Checkpoints};
pub use command::Command;
pub use error::{CoreError, ProviderError, ToolError};
pub use event::{AgentEvent, SessionState, StopReason};
pub use id::CheckpointId;
pub use intent::{Effort, Intent, Mode, TrustMode};
pub use isolation::Isolation;
pub use lsp::{LspServerSettings, LspServerSummary, LspStatus};
pub use mcp::{McpServerSettings, McpServerSummary, McpStatus};
pub use permission::{PermissionAnswer, PermissionKind, PermissionRequest, ScopeKey};
pub use process::shell::ShellChoice;
pub use process::{ProcessId, ProcessState, ProcessSummary};
pub use provider::registry::{Protocol, ProviderSpec, Providers};
pub use session::{spawn, Attach, Replay, SessionConfig, SessionHandle};
pub use settings::Settings;
pub use slash::SlashCommand;
pub use store::{SearchHit, SessionMeta, SessionStore};
pub use workspace::Workspace;
