//! The full list of wire variant names, written out once. The test checks that the
//! names of the values the fixtures contain are exactly these: a variant added to
//! Rust without a fixture, or a fixture for a variant that is not listed, fails.

pub const AGENT_EVENTS: &[&str] = &[
    "userMessage",
    "turnStarted",
    "textDelta",
    "thinkingDelta",
    "toolCallStarted",
    "toolCallProgress",
    "toolCallFinished",
    "toolCallFailed",
    "toolCallRejected",
    "toolCallDenied",
    "permissionRequested",
    "permissionResolved",
    "questionAsked",
    "questionResolved",
    "elicitationRequested",
    "elicitationResolved",
    "todosUpdated",
    "titleSet",
    "textShown",
    "fileSent",
    "planProposed",
    "fileChanged",
    "fileViewLoaded",
    "queueUpdated",
    "filesFound",
    "processesUpdated",
    "mcpUpdated",
    "lspUpdated",
    "processOutputReset",
    "processOutput",
    "checkpointCreated",
    "checkpointsUpdated",
    "checkpointPreviewed",
    "checkpointRestored",
    "contextCompacted",
    "summaryEdited",
    "subagentUsage",
    "routeDecided",
    "usageUpdated",
    "turnFinished",
    "turnCancelled",
    "error",
    "retryScheduled",
    "stateChanged",
    "hookReported",
];

pub const COMMANDS: &[&str] = &[
    "sendMessage",
    "cancel",
    "setMode",
    "setProfile",
    "setModel",
    "setEffort",
    "previewCheckpoint",
    "restoreCheckpoint",
    "compact",
    "editSummary",
    "answerPermission",
    "answerQuestion",
    "setBypass",
    "watchProcess",
    "stopProcess",
    "readFileView",
    "cancelQueued",
    "searchFiles",
    "reconnectMcpServer",
    "approveMcpServer",
    "answerElicitation",
    "loginMcpServer",
    "logoutMcpServer",
    "restartLspServer",
    "setLspServerEnabled",
    "installLspServer",
];

pub const CLIENT_MESSAGES: &[&str] = &["pair", "hello", "request"];

pub const CALLS: &[&str] = &[
    "listProjects",
    "listSessions",
    "listModels",
    "listChats",
    "sessionFeed",
    "openChat",
    "attach",
    "detach",
    "closeChat",
    "send",
];

pub const SERVER_MESSAGES: &[&str] = &[
    "paired",
    "welcome",
    "refused",
    "response",
    "frame",
    "resync",
    "chatsChanged",
    "attention",
];

pub const REPLIES: &[&str] = &[
    "projects", "sessions", "models", "chats", "feed", "opened", "attached", "done",
];

pub const CALL_RESULTS: &[&str] = &["ok", "error"];

pub const ERROR_CODES: &[&str] = &["chatNotOpen", "badRequest", "host"];

pub const REFUSALS: &[&str] = &[
    "unauthorized",
    "pairingFailed",
    "protocolMismatch",
    "tooManyAttempts",
    "handshakeExpected",
];

pub const ATTENTIONS: &[&str] = &["permission", "question", "elicitation", "turnFinished"];

pub const STOP_REASONS: &[&str] = &[
    "endTurn",
    "maxTokens",
    "stopSequence",
    "toolUse",
    "refusal",
    "pauseTurn",
    "failed",
];

pub const PERMISSION_KINDS: &[&str] = &[
    "runCommand",
    "deleteFile",
    "deleteDirectory",
    "moveFile",
    "editFile",
    "fetchUrl",
    "mcpWrite",
];

pub const SCOPE_KEYS: &[&str] = &["tool", "directory", "commandPrefix", "domain", "mcpServer"];

pub const PERMISSION_ANSWERS: &[&str] = &["allow", "allowAlways", "deny", "denyAlways"];

pub const QUESTION_REPLIES: &[&str] = &["answered", "dismissed"];

pub const ELICITATION_REPLIES: &[&str] = &["accept", "decline", "cancel"];

pub const PROCESS_STATES: &[&str] = &["running", "exited", "stopped"];

pub const CHANGE_KINDS: &[&str] = &["created", "modified", "deleted", "moved"];

pub const CHECKPOINT_REASONS: &[&str] = &["message", "command", "rollback"];

pub const MODES: &[&str] = &["plan", "manual", "auto", "edits"];

pub const EFFORTS: &[&str] = &["low", "medium", "high", "extra", "max"];

pub const LINE_KINDS: &[&str] = &["context", "add", "del"];

pub const MCP_STATUSES: &[&str] = &[
    "connecting",
    "ready",
    "failed",
    "disabled",
    "needsApproval",
    "needsLogin",
    "loggingIn",
];

pub const LSP_STATUSES: &[&str] = &[
    "idle",
    "starting",
    "ready",
    "failed",
    "disabled",
    "missing",
    "installing",
];

pub const TODO_STATUSES: &[&str] = &["pending", "inProgress", "completed"];

pub const VIEW_SEGMENTS: &[&str] = &["unchanged", "hunk"];

pub const CONTEXT_SOURCES: &[&str] = &[
    "default_guess",
    "provider_reported",
    "detected_allocated",
    "detected_maximum",
    "hand",
];

pub const INVARIANTS: &[&str] = &["gitCommit", "gitPush", "gitUnresolved", "gitConfig"];

pub const ELICIT_KINDS: &[&str] = &["text", "number", "integer", "boolean", "choice"];

pub const MCP_SOURCES: &[&str] = &["global", "projectFile", "project", "plugin", "app"];

pub const MCP_TRANSPORTS: &[&str] = &["stdio", "http", "app"];
