//! Name functions for the nested wire enums that the events and requests carry.
//! Same rule as `names.rs`: every variant is listed, no wildcard arm, so a new
//! variant breaks the build until the builders in `variants.rs` produce it.

use yaca_core::change::ViewSegment;
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
use yaca_remote::protocol::ErrorCode;

pub(crate) fn stop_reason_name(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::EndTurn => "endTurn",
        StopReason::MaxTokens => "maxTokens",
        StopReason::StopSequence => "stopSequence",
        StopReason::ToolUse => "toolUse",
        StopReason::Refusal => "refusal",
        StopReason::PauseTurn => "pauseTurn",
        StopReason::Failed => "failed",
    }
}

pub(crate) fn permission_kind_name(kind: &PermissionKind) -> &'static str {
    match kind {
        PermissionKind::RunCommand => "runCommand",
        PermissionKind::DeleteFile => "deleteFile",
        PermissionKind::DeleteDirectory => "deleteDirectory",
        PermissionKind::MoveFile => "moveFile",
        PermissionKind::EditFile => "editFile",
        PermissionKind::FetchUrl => "fetchUrl",
        PermissionKind::McpWrite => "mcpWrite",
    }
}

pub(crate) fn scope_key_name(scope: &ScopeKey) -> &'static str {
    match scope {
        ScopeKey::Tool { .. } => "tool",
        ScopeKey::Directory { .. } => "directory",
        ScopeKey::CommandPrefix { .. } => "commandPrefix",
        ScopeKey::Domain { .. } => "domain",
        ScopeKey::McpServer { .. } => "mcpServer",
    }
}

pub(crate) fn permission_answer_name(answer: &PermissionAnswer) -> &'static str {
    match answer {
        PermissionAnswer::Allow => "allow",
        PermissionAnswer::AllowAlways { .. } => "allowAlways",
        PermissionAnswer::Deny { .. } => "deny",
        PermissionAnswer::DenyAlways { .. } => "denyAlways",
    }
}

pub(crate) fn question_reply_name(reply: &QuestionReply) -> &'static str {
    match reply {
        QuestionReply::Answered { .. } => "answered",
        QuestionReply::Dismissed => "dismissed",
    }
}

pub(crate) fn elicitation_reply_name(reply: &ElicitationReply) -> &'static str {
    match reply {
        ElicitationReply::Accept { .. } => "accept",
        ElicitationReply::Decline => "decline",
        ElicitationReply::Cancel => "cancel",
    }
}

pub(crate) fn process_state_name(state: &ProcessState) -> &'static str {
    match state {
        ProcessState::Running => "running",
        ProcessState::Exited { .. } => "exited",
        ProcessState::Stopped => "stopped",
    }
}

pub(crate) fn change_kind_name(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Created => "created",
        ChangeKind::Modified => "modified",
        ChangeKind::Deleted => "deleted",
        ChangeKind::Moved => "moved",
    }
}

pub(crate) fn checkpoint_reason_name(reason: &CheckpointReason) -> &'static str {
    match reason {
        CheckpointReason::Message => "message",
        CheckpointReason::Command { .. } => "command",
        CheckpointReason::Rollback => "rollback",
    }
}

pub(crate) fn mode_name(mode: &Mode) -> &'static str {
    match mode {
        Mode::Plan => "plan",
        Mode::Manual => "manual",
        Mode::Auto => "auto",
        Mode::Edits => "edits",
    }
}

pub(crate) fn effort_name(effort: &Effort) -> &'static str {
    match effort {
        Effort::Auto => "auto",
        Effort::None => "none",
        Effort::Minimal => "minimal",
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High => "high",
        Effort::Extra => "extra",
        Effort::Max => "max",
    }
}

pub(crate) fn line_kind_name(kind: &LineKind) -> &'static str {
    match kind {
        LineKind::Context => "context",
        LineKind::Add => "add",
        LineKind::Del => "del",
    }
}

pub(crate) fn mcp_status_name(status: &McpStatus) -> &'static str {
    match status {
        McpStatus::Connecting => "connecting",
        McpStatus::Ready => "ready",
        McpStatus::Failed => "failed",
        McpStatus::Disabled => "disabled",
        McpStatus::NeedsApproval => "needsApproval",
        McpStatus::NeedsLogin => "needsLogin",
        McpStatus::LoggingIn => "loggingIn",
    }
}

pub(crate) fn lsp_status_name(status: &LspStatus) -> &'static str {
    match status {
        LspStatus::Idle => "idle",
        LspStatus::Starting => "starting",
        LspStatus::Ready => "ready",
        LspStatus::Failed => "failed",
        LspStatus::Disabled => "disabled",
        LspStatus::Missing => "missing",
        LspStatus::Installing => "installing",
    }
}

pub(crate) fn todo_status_name(status: &TodoStatus) -> &'static str {
    match status {
        TodoStatus::Pending => "pending",
        TodoStatus::InProgress => "inProgress",
        TodoStatus::Completed => "completed",
    }
}

pub(crate) fn view_segment_name(segment: &ViewSegment) -> &'static str {
    match segment {
        ViewSegment::Unchanged { .. } => "unchanged",
        ViewSegment::Hunk { .. } => "hunk",
    }
}

pub(crate) fn context_source_name(source: &ContextSource) -> &'static str {
    match source {
        ContextSource::DefaultGuess => "default_guess",
        ContextSource::ProviderReported => "provider_reported",
        ContextSource::DetectedAllocated => "detected_allocated",
        ContextSource::DetectedMaximum => "detected_maximum",
        ContextSource::Hand => "hand",
    }
}

pub(crate) fn invariant_name(invariant: &Invariant) -> &'static str {
    match invariant {
        Invariant::GitCommit => "gitCommit",
        Invariant::GitPush => "gitPush",
        Invariant::GitUnresolved => "gitUnresolved",
        Invariant::GitConfig => "gitConfig",
    }
}

pub(crate) fn elicit_kind_name(kind: &ElicitKind) -> &'static str {
    match kind {
        ElicitKind::Text => "text",
        ElicitKind::Number => "number",
        ElicitKind::Integer => "integer",
        ElicitKind::Boolean => "boolean",
        ElicitKind::Choice => "choice",
    }
}

pub(crate) fn mcp_source_name(source: &McpSource) -> &'static str {
    match source {
        McpSource::Global => "global",
        McpSource::ProjectFile => "projectFile",
        McpSource::Project => "project",
        McpSource::Plugin => "plugin",
        McpSource::App => "app",
    }
}

pub(crate) fn mcp_transport_name(transport: &McpTransport) -> &'static str {
    match transport {
        McpTransport::Stdio => "stdio",
        McpTransport::Http => "http",
        McpTransport::App => "app",
    }
}

pub(crate) fn error_code_name(code: &ErrorCode) -> &'static str {
    match code {
        ErrorCode::ChatNotOpen => "chatNotOpen",
        ErrorCode::BadRequest => "badRequest",
        ErrorCode::Host => "host",
    }
}
