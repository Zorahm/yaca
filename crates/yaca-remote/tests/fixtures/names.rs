//! Name functions for the wire enums that carry their own tag: the name each
//! variant has in JSON. Every match lists every variant with no wildcard arm, so
//! a variant added in Rust stops this file from compiling until a fixture for it
//! exists. The test then checks each name against what serde actually writes.

use yaca_core::event::AgentEvent;
use yaca_core::Command;
use yaca_remote::protocol::{
    Attention, Call, CallResult, ClientMessage, Refusal, Reply, ServerMessage,
};

pub fn agent_event_name(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::UserMessage { .. } => "userMessage",
        AgentEvent::TurnStarted { .. } => "turnStarted",
        AgentEvent::TextDelta { .. } => "textDelta",
        AgentEvent::ThinkingDelta { .. } => "thinkingDelta",
        AgentEvent::ToolCallStarted { .. } => "toolCallStarted",
        AgentEvent::ToolCallProgress { .. } => "toolCallProgress",
        AgentEvent::ToolCallFinished { .. } => "toolCallFinished",
        AgentEvent::ToolCallFailed { .. } => "toolCallFailed",
        AgentEvent::ToolCallRejected { .. } => "toolCallRejected",
        AgentEvent::ToolCallDenied { .. } => "toolCallDenied",
        AgentEvent::PermissionRequested { .. } => "permissionRequested",
        AgentEvent::PermissionResolved { .. } => "permissionResolved",
        AgentEvent::QuestionAsked { .. } => "questionAsked",
        AgentEvent::QuestionResolved { .. } => "questionResolved",
        AgentEvent::ElicitationRequested { .. } => "elicitationRequested",
        AgentEvent::ElicitationResolved { .. } => "elicitationResolved",
        AgentEvent::TodosUpdated { .. } => "todosUpdated",
        AgentEvent::TitleSet { .. } => "titleSet",
        AgentEvent::TextShown { .. } => "textShown",
        AgentEvent::PlanProposed { .. } => "planProposed",
        AgentEvent::FileChanged { .. } => "fileChanged",
        AgentEvent::FileViewLoaded { .. } => "fileViewLoaded",
        AgentEvent::QueueUpdated { .. } => "queueUpdated",
        AgentEvent::FilesFound { .. } => "filesFound",
        AgentEvent::ProcessesUpdated { .. } => "processesUpdated",
        AgentEvent::McpUpdated { .. } => "mcpUpdated",
        AgentEvent::LspUpdated { .. } => "lspUpdated",
        AgentEvent::ProcessOutputReset { .. } => "processOutputReset",
        AgentEvent::ProcessOutput { .. } => "processOutput",
        AgentEvent::CheckpointCreated { .. } => "checkpointCreated",
        AgentEvent::CheckpointsUpdated { .. } => "checkpointsUpdated",
        AgentEvent::CheckpointPreviewed { .. } => "checkpointPreviewed",
        AgentEvent::CheckpointRestored { .. } => "checkpointRestored",
        AgentEvent::ContextCompacted { .. } => "contextCompacted",
        AgentEvent::SummaryEdited { .. } => "summaryEdited",
        AgentEvent::SubagentUsage { .. } => "subagentUsage",
        AgentEvent::RouteDecided { .. } => "routeDecided",
        AgentEvent::UsageUpdated { .. } => "usageUpdated",
        AgentEvent::TurnFinished { .. } => "turnFinished",
        AgentEvent::TurnCancelled { .. } => "turnCancelled",
        AgentEvent::Error { .. } => "error",
        AgentEvent::RetryScheduled { .. } => "retryScheduled",
        AgentEvent::StateChanged { .. } => "stateChanged",
        AgentEvent::HookReported { .. } => "hookReported",
    }
}

pub fn command_name(command: &Command) -> &'static str {
    match command {
        Command::SendMessage { .. } => "sendMessage",
        Command::Cancel => "cancel",
        Command::SetMode { .. } => "setMode",
        Command::SetModel { .. } => "setModel",
        Command::SetEffort { .. } => "setEffort",
        Command::PreviewCheckpoint { .. } => "previewCheckpoint",
        Command::RestoreCheckpoint { .. } => "restoreCheckpoint",
        Command::Compact => "compact",
        Command::EditSummary { .. } => "editSummary",
        Command::AnswerPermission { .. } => "answerPermission",
        Command::AnswerQuestion { .. } => "answerQuestion",
        Command::SetBypass { .. } => "setBypass",
        Command::WatchProcess { .. } => "watchProcess",
        Command::StopProcess { .. } => "stopProcess",
        Command::ReadFileView { .. } => "readFileView",
        Command::CancelQueued { .. } => "cancelQueued",
        Command::SearchFiles { .. } => "searchFiles",
        Command::ReconnectMcpServer { .. } => "reconnectMcpServer",
        Command::ApproveMcpServer { .. } => "approveMcpServer",
        Command::AnswerElicitation { .. } => "answerElicitation",
        Command::LoginMcpServer { .. } => "loginMcpServer",
        Command::LogoutMcpServer { .. } => "logoutMcpServer",
        Command::RestartLspServer { .. } => "restartLspServer",
    }
}

pub fn client_message_name(message: &ClientMessage) -> &'static str {
    match message {
        ClientMessage::Pair { .. } => "pair",
        ClientMessage::Hello { .. } => "hello",
        ClientMessage::Request { .. } => "request",
    }
}

pub fn call_name(call: &Call) -> &'static str {
    match call {
        Call::ListProjects => "listProjects",
        Call::ListSessions { .. } => "listSessions",
        Call::ListModels => "listModels",
        Call::ListChats => "listChats",
        Call::SessionFeed { .. } => "sessionFeed",
        Call::OpenChat { .. } => "openChat",
        Call::Attach { .. } => "attach",
        Call::Detach { .. } => "detach",
        Call::CloseChat { .. } => "closeChat",
        Call::Send { .. } => "send",
    }
}

pub fn server_message_name(message: &ServerMessage) -> &'static str {
    match message {
        ServerMessage::Paired { .. } => "paired",
        ServerMessage::Welcome { .. } => "welcome",
        ServerMessage::Refused { .. } => "refused",
        ServerMessage::Response { .. } => "response",
        ServerMessage::Frame { .. } => "frame",
        ServerMessage::Resync { .. } => "resync",
        ServerMessage::ChatsChanged { .. } => "chatsChanged",
        ServerMessage::Attention { .. } => "attention",
    }
}

pub fn reply_name(reply: &Reply) -> &'static str {
    match reply {
        Reply::Projects { .. } => "projects",
        Reply::Sessions { .. } => "sessions",
        Reply::Models { .. } => "models",
        Reply::Chats { .. } => "chats",
        Reply::Feed { .. } => "feed",
        Reply::Opened { .. } => "opened",
        Reply::Attached { .. } => "attached",
        Reply::Done => "done",
    }
}

pub fn call_result_name(result: &CallResult) -> &'static str {
    match result {
        CallResult::Ok { .. } => "ok",
        CallResult::Error { .. } => "error",
    }
}

pub fn refusal_name(refusal: &Refusal) -> &'static str {
    match refusal {
        Refusal::Unauthorized => "unauthorized",
        Refusal::PairingFailed => "pairingFailed",
        Refusal::ProtocolMismatch { .. } => "protocolMismatch",
        Refusal::TooManyAttempts => "tooManyAttempts",
        Refusal::HandshakeExpected => "handshakeExpected",
    }
}

pub fn attention_name(attention: &Attention) -> &'static str {
    match attention {
        Attention::Permission { .. } => "permission",
        Attention::Question { .. } => "question",
        Attention::Elicitation => "elicitation",
        Attention::TurnFinished { .. } => "turnFinished",
    }
}
