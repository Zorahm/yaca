//! Every variant of each nested wire enum, listed once. The builders iterate
//! these lists, so a fixture carries each variant by construction, and the test
//! checks the names of these same lists against `expected.rs`. A variant added
//! to a core enum fails to compile in `names.rs` first, then fails the name
//! check here until it is added to the list.

use std::collections::BTreeMap;

use serde_json::json;
use yaca_core::change::{ChangeKind, LineKind};
use yaca_core::checkpoint::CheckpointReason;
use yaca_core::event::{StopReason, TodoStatus};
use yaca_core::intent::{Effort, Mode};
use yaca_core::lsp::LspStatus;
use yaca_core::mcp::{ElicitKind, ElicitationReply, McpSource, McpStatus, McpTransport};
use yaca_core::permission::{Invariant, PermissionAnswer, PermissionKind, ScopeKey};
use yaca_core::process::ProcessState;
use yaca_core::question::QuestionReply;
use yaca_core::usage::ContextSource;
use yaca_remote::protocol::{Attention, ErrorCode, Refusal, PROTOCOL_VERSION};

pub fn stop_reasons() -> Vec<StopReason> {
    vec![
        StopReason::EndTurn,
        StopReason::MaxTokens,
        StopReason::StopSequence,
        StopReason::ToolUse,
        StopReason::Refusal,
        StopReason::PauseTurn,
        StopReason::Failed,
    ]
}

pub fn permission_kinds() -> Vec<PermissionKind> {
    vec![
        PermissionKind::RunCommand,
        PermissionKind::DeleteFile,
        PermissionKind::DeleteDirectory,
        PermissionKind::MoveFile,
        PermissionKind::EditFile,
        PermissionKind::FetchUrl,
        PermissionKind::McpWrite,
    ]
}

pub fn scope_keys() -> Vec<ScopeKey> {
    vec![
        ScopeKey::Tool { name: "Edit".into() },
        ScopeKey::Directory { path: "src".into() },
        ScopeKey::CommandPrefix { prefix: "cargo".into() },
        ScopeKey::Domain { host: "docs.rs".into() },
        ScopeKey::McpServer { server: "asana".into() },
    ]
}

pub fn permission_answers() -> Vec<PermissionAnswer> {
    vec![
        PermissionAnswer::Allow,
        PermissionAnswer::AllowAlways {
            scope: ScopeKey::Directory { path: "src".into() },
        },
        PermissionAnswer::Deny {
            reason: Some("use the existing helper".into()),
        },
        PermissionAnswer::DenyAlways {
            scope: ScopeKey::CommandPrefix { prefix: "rm".into() },
            reason: None,
        },
    ]
}

pub fn question_replies() -> Vec<QuestionReply> {
    vec![
        QuestionReply::Answered {
            selected: vec![0, 2],
            other: None,
        },
        QuestionReply::Answered {
            selected: vec![],
            other: Some("neither, reuse the cache".into()),
        },
        QuestionReply::Dismissed,
    ]
}

pub fn elicitation_replies() -> Vec<ElicitationReply> {
    vec![
        ElicitationReply::Accept {
            values: BTreeMap::from([
                ("title".to_owned(), json!("Release notes")),
                ("count".to_owned(), json!(3)),
            ]),
        },
        ElicitationReply::Decline,
        ElicitationReply::Cancel,
    ]
}

pub fn process_states() -> Vec<ProcessState> {
    vec![
        ProcessState::Running,
        ProcessState::Exited { code: Some(0) },
        ProcessState::Exited { code: None },
        ProcessState::Stopped,
    ]
}

pub fn change_kinds() -> Vec<ChangeKind> {
    vec![
        ChangeKind::Created,
        ChangeKind::Modified,
        ChangeKind::Deleted,
        ChangeKind::Moved,
    ]
}

pub fn checkpoint_reasons() -> Vec<CheckpointReason> {
    vec![
        CheckpointReason::Message,
        CheckpointReason::Command {
            command: "cargo test -p yaca-remote".into(),
        },
        CheckpointReason::Rollback,
    ]
}

pub fn modes() -> Vec<Mode> {
    vec![Mode::Plan, Mode::Manual, Mode::Auto, Mode::Edits]
}

pub fn efforts() -> Vec<Effort> {
    vec![Effort::Low, Effort::Medium, Effort::High, Effort::Extra, Effort::Max]
}

pub fn line_kinds() -> Vec<LineKind> {
    vec![LineKind::Context, LineKind::Add, LineKind::Del]
}

pub fn mcp_statuses() -> Vec<McpStatus> {
    vec![
        McpStatus::Connecting,
        McpStatus::Ready,
        McpStatus::Failed,
        McpStatus::Disabled,
        McpStatus::NeedsApproval,
        McpStatus::NeedsLogin,
        McpStatus::LoggingIn,
    ]
}

pub fn lsp_statuses() -> Vec<LspStatus> {
    vec![
        LspStatus::Idle,
        LspStatus::Starting,
        LspStatus::Ready,
        LspStatus::Failed,
        LspStatus::Disabled,
    ]
}

pub fn todo_statuses() -> Vec<TodoStatus> {
    vec![TodoStatus::Pending, TodoStatus::InProgress, TodoStatus::Completed]
}

pub fn context_sources() -> Vec<ContextSource> {
    vec![
        ContextSource::DefaultGuess,
        ContextSource::ProviderReported,
        ContextSource::DetectedAllocated,
        ContextSource::DetectedMaximum,
    ]
}

pub fn invariants() -> Vec<Invariant> {
    vec![
        Invariant::GitCommit,
        Invariant::GitPush,
        Invariant::GitUnresolved,
        Invariant::GitConfig,
    ]
}

pub fn elicit_kinds() -> Vec<ElicitKind> {
    vec![
        ElicitKind::Text,
        ElicitKind::Number,
        ElicitKind::Integer,
        ElicitKind::Boolean,
        ElicitKind::Choice,
    ]
}

pub fn mcp_sources() -> Vec<McpSource> {
    vec![
        McpSource::Global,
        McpSource::ProjectFile,
        McpSource::Project,
        McpSource::Plugin,
        McpSource::App,
    ]
}

pub fn mcp_transports() -> Vec<McpTransport> {
    vec![McpTransport::Stdio, McpTransport::Http, McpTransport::App]
}

pub fn error_codes() -> Vec<ErrorCode> {
    vec![ErrorCode::ChatNotOpen, ErrorCode::BadRequest, ErrorCode::Host]
}

pub fn refusals() -> Vec<Refusal> {
    vec![
        Refusal::Unauthorized,
        Refusal::PairingFailed,
        Refusal::ProtocolMismatch {
            server: PROTOCOL_VERSION + 1,
        },
        Refusal::TooManyAttempts,
        Refusal::HandshakeExpected,
    ]
}

pub fn attentions() -> Vec<Attention> {
    vec![
        Attention::Permission {
            request: crate::ids::request(7),
            target: "cargo".into(),
        },
        Attention::Question {
            text: "Which test runner should I use?".into(),
        },
        Attention::Elicitation,
        Attention::TurnFinished {
            stop_reason: StopReason::EndTurn,
        },
    ]
}
