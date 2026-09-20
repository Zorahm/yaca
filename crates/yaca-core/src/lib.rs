//! Ядро Yaca.
//!
//! Безголовое: ни окна, ни вебвью, ни Tauri. Наружу — поток [`AgentEvent`],
//! внутрь — [`Command`]. Интерфейс есть чистая функция от этого потока, и
//! проверить это можно без интерфейса: `yaca-cli` гоняет ровно то же ядро.
//!
//! Сейчас закрыты разговор целиком, стажирование правок, инварианты §8,
//! разрешения и выполнение команд: читающие и пишущие тулы, набор непринятых
//! правок с диффами, применение к рабочему дереву с проверкой конфликта,
//! матрица доверия с запросами, команды со стримингом вывода и фоновые
//! процессы с панелью §6.5.
//!
//! Checkpoints (§5.8) are in as well: a shadow store of its own under
//! `.yaca/shadow`, a snapshot before each message, before the change set
//! reaches disk and before every permitted command, and a rollback that names
//! the files it will touch before it touches them.
//!
//! Storage is in too (§9): three append-only files per session under
//! `.yaca/sessions`, a rebuildable index behind the panel, search over the
//! conversation and over the paths a session changed, and a session that
//! reopens with the history the model actually had. Alongside it,
//! `.yaca/settings.json` (§10), provider keys in the system keychain, and
//! history compaction with a summary the human can rewrite (§5.11).

pub mod change;
pub mod checkpoint;
pub mod command;
pub mod error;
pub mod event;
pub mod id;
pub mod instructions;
pub mod intent;
pub mod legacy;
pub mod message;
pub mod permission;
pub mod process;
pub mod project;
pub mod provider;
pub mod secrets;
pub mod session;
pub mod settings;
pub mod slash;
pub mod store;
pub mod tool;
pub mod usage;
pub mod workspace;

#[cfg(test)]
mod testutil;

pub use change::ChangeKind;
pub use checkpoint::{CheckpointChange, CheckpointReason, CheckpointSummary, Checkpoints};
pub use command::Command;
pub use error::{CoreError, ProviderError, ToolError};
pub use event::{AgentEvent, SessionState, StopReason};
pub use id::CheckpointId;
pub use intent::{Effort, Intent, Mode, TrustMode};
pub use permission::{PermissionAnswer, PermissionKind, PermissionRequest, ScopeKey};
pub use process::{ProcessId, ProcessState, ProcessSummary};
pub use provider::registry::{Protocol, ProviderSpec, Providers};
pub use session::{spawn, SessionConfig, SessionHandle};
pub use settings::Settings;
pub use slash::SlashCommand;
pub use store::{SearchHit, SessionMeta, SessionStore};
pub use workspace::Workspace;
