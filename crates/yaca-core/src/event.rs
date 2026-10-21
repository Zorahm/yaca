//! The `AgentEvent` stream is the only thing the core hands out (§15).
//!
//! The interface is a pure function of this stream, so nothing here may be
//! something that cannot be drawn, and nothing that §5 draws may be missing.
//!
//! Beyond the §15 list, five events are added; each closes a gap where §5
//! asks for a picture and §15 gives no data:
//!
//! * [`AgentEvent::Error`] and [`AgentEvent::RetryScheduled`] — §5.10 draws the
//!   error block with its code and the line `Retry in 4 s · attempt 2 of 5`.
//! * [`AgentEvent::ToolCallRejected`] — §5.9: a tripped invariant is coloured
//!   `--vermilion-ink`, not `--rust`, and it is not the same as a failed tool.
//! * [`AgentEvent::TodosUpdated`] — §5.6: the task list is updated in place;
//!   it cannot be derived from the `todo_write` result.
//! * [`AgentEvent::PlanProposed`] — §7.1: leaving `Plan` through `exit_plan_mode`
//!   shows the plan and waits until the human picks a mode.
//!
//! [`AgentEvent::ToolCallDenied`] gained a `stopped` field: §5.5
//! tells a refused permission apart from a stopped running call, while §15 gives
//! both one outcome.
//!
//! With the command slice, three events for the process panel (§6.5) were added,
//! which §15 does not have at all:
//!
//! * [`AgentEvent::ProcessesUpdated`] — the whole list of background commands: the panel
//!   is a function of the last such event, not a sum of the previous ones.
//! * [`AgentEvent::ProcessOutputReset`] — "selection shows output": the whole
//!   ring buffer at once, when the human picks a row.
//! * [`AgentEvent::ProcessOutput`] — the live tail of the selected row.
//!
//! The checkpoint slice adds three more on top of §15's `CheckpointCreated`:
//!
//! * [`AgentEvent::CheckpointsUpdated`] — the whole list of points. §1.1 gives
//!   checkpoints a panel, and a panel assembled from twenty separate
//!   `CheckpointCreated` events is a second copy of the truth.
//! * [`AgentEvent::CheckpointPreviewed`] — §5.8 requires a rollback to show the
//!   files it will touch and be confirmed against them. That list exists only
//!   in the core, and §15 has no event carrying it.
//! * [`AgentEvent::CheckpointRestored`] — the `Rolled back to 14:32 · 3 files` line
//!   of §5.5. `CheckpointCreated` marks the point; nothing in §15 marks the
//!   return to it.
//!
//! The storage slice adds one more beside §15's `ContextCompacted`:
//!
//! * [`AgentEvent::SummaryEdited`] — §5.11 lets the human rewrite the summary
//!   before continuing, and a session reopened from its journal has to show the
//!   text they left, not the one the model first wrote.
//!
//! Subagents add no event, and one field: `parent` on
//! [`AgentEvent::ToolCallStarted`]. A subagent's calls are rows like any
//! other — the same diff, the same question to the human — and the field says
//! which `Task` row they are drawn under. §15 has no place for it because the
//! spec leaves "where do subagents go" as an open question.
//!
//! A subagent sent in the background adds no event either, and moves one: the
//! `Task` row gets its `ToolCallFinished` (or the failure) when the subagent
//! is done, which is after the model has its `tool_result` and after rows of
//! the agent's own that started later. A row's end says the work under it is
//! over, not that the call returned.
//!
//! The agents panel adds [`AgentEvent::SubagentUsage`]: what one subagent has
//! spent so far. The session's own figure in `UsageUpdated` is every subagent
//! and the agent together, and the panel shows each of them its own.
//!
//! The context window adds no event, and one field: `breakdown` on the
//! snapshot [`AgentEvent::UsageUpdated`] carries. §7.1 has the ring show how
//! full the window is; what it is full of is known only where the request is
//! built, so the split travels with the figure it splits.
//!
//! The question slice adds two events beyond §15, for `AskUser`: the agent
//! asks the human a question with ready options and waits, and the answer is
//! the result of the tool call that asked. §5.7 draws a block only for
//! permissions, a yes or no about an action; a question is a choice the agent
//! cannot make for the human.
//!
//! * [`AgentEvent::QuestionAsked`] — the question on screen, with its options.
//! * [`AgentEvent::QuestionResolved`] — closed: answered, dismissed, or the
//!   turn stopped first (`reply: None`). A replayed journal without it would
//!   draw a question nobody can answer any more.
//!
//! Plugins add [`AgentEvent::HookReported`]: what a plugin hook said at a moment
//! that has no row of its own — the session's start, a message from the human,
//! the end of a turn. A hook around a call speaks in that call's row, as the
//! project's hooks do; these three have nowhere else to stand, and a hook that
//! keeps a message from the model, or sends a turn back to work, must not do it
//! out of sight.
//!
//! The file panel (§6.4) adds one event beyond §15:
//!
//! * [`AgentEvent::FileViewLoaded`] — the full diff of one file with its
//!   unchanged lines, answering [`Command::ReadFileView`]. §6.4 asks for the
//!   whole file, and the `FileChanged` diff is one call's hunks with three lines
//!   of context, which is not that.
//!
//! The queue (§7.4) adds [`AgentEvent::QueueUpdated`]: the messages sent during
//! a turn that the turn has not taken in yet, as a whole list, each with the id
//! its cancel button names. Like `ProcessesUpdated` it is a snapshot, not a
//! delta. It is not journaled: a queued message that was never taken in did not
//! happen, and a reopened session must not show a chip for it.
//!
//! The mention search (§7.2) adds [`AgentEvent::FilesFound`]: the answer to
//! [`Command::SearchFiles`], carrying the query it answers so the interface can
//! drop an answer to a query it has already left. It is not journaled either —
//! it is an answer about the disk now, like [`AgentEvent::FileViewLoaded`].
//!
//! MCP adds [`AgentEvent::McpUpdated`]: the session's MCP servers and the
//! state of each, as a whole list. §7.3 names `/mcp` and §10 once listed an
//! MCP section, but §15 has nothing either could be drawn from. A snapshot of
//! live state like `ProcessesUpdated`, and not journaled for the same reason:
//! a reopened session connects anew, and yesterday's "ready" would be a lie.
//!
//! Language servers add [`AgentEvent::LspUpdated`], for the same reason and
//! on the same terms as `McpUpdated`: a whole list of servers and their
//! states, a snapshot of what is live, not journaled.
//!
//! `SetTitle` adds [`AgentEvent::TitleSet`]: the conversation's name as the
//! model gave it. §3 takes the title from the first message, and that is often
//! a greeting; the name has to travel as an event so the index folds it like
//! everything else and a reopened or rebuilt session keeps it.
//!
//! `SendText` adds [`AgentEvent::TextShown`]: the text of a file put in front
//! of the human as a block of the feed, without the model writing it out. §15
//! has nothing to draw it from — `TextDelta` would pass the file off as the
//! model's own words, and a tool row is output, not a part of the answer. The
//! text travels in the event, so a reopened session shows what was shown then
//! and not what the file holds now.
//!
//! And the interface slice adds [`AgentEvent::UserMessage`]. §5.1 draws what
//! the human said, and §9 reopens a session by replaying this journal: without
//! it a reopened session is the agent talking to itself. The text is written
//! down twice — here for the feed, and in the message log for the model — but
//! the two carry different things, and the feed log is the one that has to
//! be replayable on its own.
//!
//! The model router adds [`AgentEvent::RouteDecided`] beyond §15: which model
//! answered a request, which decision matched it (`None` for the default), and
//! the signals that were read. §15 has no event naming the model, and a router
//! picks one per request, so the choice has to travel as an event of its own.
//! It comes once per request, ahead of that request's text, and not for a
//! subagent's requests, which have no feed. It is journaled and replayed: which
//! model answered is what happened, not live status.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::change::{FileDiff, FileFact, FileView};
use crate::checkpoint::{CheckpointChange, CheckpointSummary};
use crate::id::{BlockId, CallId, CheckpointId, QueuedId, RequestId, TurnId};
use crate::intent::{Effort, Mode};
use crate::mcp::McpServerSummary;
use crate::permission::PermissionRequest;
use crate::process::{ProcessId, ProcessSummary};
use crate::question::{QuestionReply, QuestionRequest};
use crate::usage::{TokenCounts, UsageSnapshot};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
    /// The human's message card (§5.1).
    ///
    /// It comes from the core, not drawn by the interface when the message is sent:
    /// otherwise a session reopened would show answers without their questions.
    #[serde(rename_all = "camelCase")]
    UserMessage {
        turn_id: TurnId,
        text: String,
        /// Files attached via `@` (§7.2) — chips above the text.
        attachments: Vec<String>,
    },

    /// The state ring spins, the send button becomes `■`.
    #[serde(rename_all = "camelCase")]
    TurnStarted { turn_id: TurnId, at: TsMillis },

    /// Appends to the current text block.
    #[serde(rename_all = "camelCase")]
    TextDelta {
        turn_id: TurnId,
        block_id: BlockId,
        text: String,
    },

    /// Appends to the collapsed reasoning block (§5.3).
    #[serde(rename_all = "camelCase")]
    ThinkingDelta {
        turn_id: TurnId,
        block_id: BlockId,
        text: String,
    },

    /// A row in the action group with a `●` dot, the label in the present tense.
    #[serde(rename_all = "camelCase")]
    ToolCallStarted {
        turn_id: TurnId,
        call_id: CallId,
        /// Tool name. Not shown in the feed (§16.6); needed for the icon and debugging.
        tool: String,
        /// The label the model wrote (§5.4.1). `None` — a tool the interface labels
        /// itself, or a field the model left empty.
        description: Option<String>,
        /// First line of the expanded row (§5.5.1): the full path or the full command.
        target: Option<String>,
        /// Call arguments. Needed for §5.7 (the detail of the permission request).
        #[ts(type = "unknown")]
        input: serde_json::Value,
        /// The `Task` row this call belongs under: it was made by a subagent,
        /// not by the agent. `None` for the agent's own calls, and absent
        /// from journals written before subagents existed.
        #[serde(default)]
        parent: Option<CallId>,
    },

    /// Appends output to the expanded row.
    #[serde(rename_all = "camelCase")]
    ToolCallProgress { call_id: CallId, chunk: String },

    /// The label in the past tense; counters and the summary header are rebuilt.
    #[serde(rename_all = "camelCase")]
    ToolCallFinished { call_id: CallId, meta: ToolMeta },

    /// The failure form in place of the model's description, the verb coloured `--rust`.
    #[serde(rename_all = "camelCase")]
    ToolCallFailed { call_id: CallId, message: String },

    /// An invariant refused (§5.9). The verb coloured `--vermilion-ink`: the guard
    /// worked, the human has nothing to do.
    #[serde(rename_all = "camelCase")]
    ToolCallRejected { call_id: CallId, reason: String },

    /// `Rejected by you` (§5.5). A third kind of outcome: neither a tool failure nor
    /// a tripped invariant, but the human's decision.
    #[serde(rename_all = "camelCase")]
    ToolCallDenied {
        call_id: CallId,
        reason: Option<String>,
        /// The call was already running and the human stopped it, rather than
        /// refusing the permission before it started. The kind of outcome is the
        /// same, a human decision, but the wording differs: "Stopped by you"
        /// against "Rejected by you". There is no field for it in §15; without it
        /// the feed would say the human rejected a stopped command, which never
        /// happened.
        stopped: bool,
    },

    /// Inline block for a permission request (§5.7). The ring pulses, the window
    /// comes up, and a notification goes out when the window is minimised.
    #[serde(rename_all = "camelCase")]
    PermissionRequested { request: PermissionRequest },

    /// The block can be removed: the answer has arrived or the turn was cancelled.
    #[serde(rename_all = "camelCase")]
    PermissionResolved { request_id: RequestId },

    /// The agent asks the human a question with ready options (`AskUser`).
    /// The tool that asked is blocked until the question is answered,
    /// dismissed, or the turn is cancelled.
    #[serde(rename_all = "camelCase")]
    QuestionAsked { request: QuestionRequest },

    /// The question is closed. `reply` is what the human did; `None` means
    /// the turn stopped before anyone answered.
    #[serde(rename_all = "camelCase")]
    QuestionResolved {
        request_id: RequestId,
        reply: Option<QuestionReply>,
    },

    /// An MCP server asks the human to fill in a form (elicitation). It stays
    /// up until answered, declined, or the server stops waiting.
    #[serde(rename_all = "camelCase")]
    ElicitationRequested {
        request: crate::mcp::ElicitationRequest,
    },

    /// The form is closed. `reply` is what the human did; `None` means the
    /// server stopped waiting first.
    #[serde(rename_all = "camelCase")]
    ElicitationResolved {
        request_id: RequestId,
        reply: Option<crate::mcp::ElicitationReply>,
    },

    /// The task list is updated in place; no new block is created (§5.6).
    #[serde(rename_all = "camelCase")]
    TodosUpdated { items: Vec<TodoItem> },

    /// The model named the conversation (`SetTitle`). The title in the list and
    /// the top bar changes at once, not at the end of the turn. A name the human
    /// typed in wins over it, which the index decides, not the interface.
    #[serde(rename_all = "camelCase")]
    TitleSet { title: String },

    /// A file's text shown to the human as a block of its own (`SendText`):
    /// the path and the lines it came from above, the text below.
    #[serde(rename_all = "camelCase")]
    TextShown {
        /// The file, or for a command's output `command <id>`.
        path: String,
        /// The command line, when what is shown is a background command's
        /// output rather than a file.
        command: Option<String>,
        /// The first line shown, 1-indexed.
        first_line: u32,
        /// How many lines are shown.
        lines: u32,
        /// How many lines the file has: with the two above it tells a whole
        /// file from a part of one.
        total: u32,
        text: String,
    },

    /// `exit_plan_mode`: show the plan and wait until the human picks a mode (§7.1).
    #[serde(rename_all = "camelCase")]
    PlanProposed { plan: String },

    /// The file changed in the working tree: the action row gets its counters, and
    /// its expansion shows the diff (§5.5.1).
    #[serde(rename_all = "camelCase")]
    FileChanged {
        call_id: CallId,
        fact: FileFact,
        diff: FileDiff,
    },

    /// The whole file for the panel (§6.4): its diff when the session changed
    /// it, its text as it is when it did not. `None`: there is no such file to
    /// show — it is gone, binary, or outside the workspace.
    #[serde(rename_all = "camelCase")]
    FileViewLoaded {
        path: String,
        view: Option<FileView>,
    },

    /// The messages sent during a turn that the turn has not taken in yet
    /// (§7.4), oldest first. Empty when nothing waits. Sent whenever the list
    /// changes, including on a cancel that found nothing to cancel.
    #[serde(rename_all = "camelCase")]
    QueueUpdated { queued: Vec<QueuedMessage> },

    /// The answer to [`Command::SearchFiles`] (§7.2): the project's files that
    /// match `query`, best first, capped. `query` is echoed so an answer that
    /// arrives after the human typed on can be dropped.
    #[serde(rename_all = "camelCase")]
    FilesFound { query: String, paths: Vec<String> },

    /// The whole process panel (§6.5).
    ///
    /// The panel is a function of the last such event, not a sum of the previous ones:
    /// the state lives in the core, the only part that knows whether a process is still alive.
    #[serde(rename_all = "camelCase")]
    ProcessesUpdated { processes: Vec<ProcessSummary> },

    /// The session's MCP servers in full: which are connecting, ready or
    /// failed, and the tools each brought. Sent whenever any of that changes.
    #[serde(rename_all = "camelCase")]
    McpUpdated { servers: Vec<McpServerSummary> },

    /// The session's language servers in full, sent whenever one starts,
    /// becomes ready or fails. Like `McpUpdated`: the list the interface
    /// draws is the last such event.
    #[serde(rename_all = "camelCase")]
    LspUpdated { servers: Vec<crate::lsp::LspServerSummary> },

    /// The human picked a row in the panel: show its output in full (§6.5).
    #[serde(rename_all = "camelCase")]
    ProcessOutputReset {
        id: ProcessId,
        text: String,
        /// How many lines were pushed out of the buffer. The panel says so outright:
        /// output lost in silence cannot be told apart from output that never existed.
        #[ts(type = "number")]
        dropped: u64,
    },

    /// The live tail of the selected process.
    ///
    /// Sent only for the selected row. The output of the others stays in the buffer:
    /// a chatty `npm run dev` nobody watches would push the real feed events out of
    /// the broadcast channel, and a lagging subscriber loses them.
    #[serde(rename_all = "camelCase")]
    ProcessOutput { id: ProcessId, chunk: String },

    /// A divider line in the feed (§5.8): `── ● 14:32 · before applying…`.
    ///
    /// Only sent when the point is new. A snapshot of a tree that has not moved
    /// since the previous checkpoint is not one, and a divider offering to roll
    /// back to the state the feed is already in would be an interface lying
    /// about state (§0).
    #[serde(rename_all = "camelCase")]
    CheckpointCreated { checkpoint: CheckpointSummary },

    /// The checkpoint panel in full (§1.1).
    ///
    /// Like the change bar, the panel is a function of the last such event and
    /// not the sum of the previous ones: the list is where a rollback adds a
    /// point of its own, and a list kept in the interface would miss it.
    #[serde(rename_all = "camelCase")]
    CheckpointsUpdated { checkpoints: Vec<CheckpointSummary> },

    /// The files a rollback would touch (§5.8). Shown for confirmation; nothing
    /// has happened on disk yet.
    #[serde(rename_all = "camelCase")]
    CheckpointPreviewed {
        id: CheckpointId,
        files: Vec<CheckpointChange>,
    },

    /// `Rolled back to 14:32 · 3 files` (§5.5). `at` is the time of the point that
    /// was returned to, which is the time the line names.
    #[serde(rename_all = "camelCase")]
    CheckpointRestored {
        id: CheckpointId,
        at: TsMillis,
        files: u32,
    },

    /// Divider `history compacted · freed 84k tokens ▾` (§5.11).
    ///
    /// `freed` is an estimate, and the field is one number rather than two
    /// because §5.11 shows one. The exact figure belongs to the provider and
    /// arrives a response later, by which time the divider has been on screen
    /// for a minute.
    #[serde(rename_all = "camelCase")]
    ContextCompacted { summary: String, freed: u32 },

    /// The human rewrote the summary before continuing (§5.11).
    #[serde(rename_all = "camelCase")]
    SummaryEdited { summary: String },

    /// What one subagent has spent since it was sent: a running total, not
    /// the last step, so the newest event for a `Task` row is the whole
    /// figure. Sent after each answer the subagent gets from the model.
    #[serde(rename_all = "camelCase")]
    SubagentUsage { call_id: CallId, tokens: TokenCounts },

    /// Which model a routing provider chose for this request, and why (see the
    /// module head). `route` is the matched decision, `None` for the default;
    /// `matched` names the signals that were read to choose.
    #[serde(rename_all = "camelCase")]
    RouteDecided {
        route: Option<String>,
        provider: String,
        model: String,
        matched: Vec<String>,
    },

    /// Fills the state ring; `usage.breakdown` says what the occupied part consists
    /// of: the ring expands into a list by part of the window.
    #[serde(rename_all = "camelCase")]
    UsageUpdated { usage: UsageSnapshot },

    /// The ring goes dark; the action group collapses into a summary.
    #[serde(rename_all = "camelCase")]
    TurnFinished {
        turn_id: TurnId,
        stop_reason: StopReason,
        /// When the turn ended. With `TurnStarted::at` it is the turn's
        /// duration, which the feed shows under the answer. Zero in a journal
        /// written before the field existed: "unknown", not "1970".
        #[serde(default)]
        at: TsMillis,
    },

    /// A `Stopped` line; unfinished tools in `cancelled`.
    #[serde(rename_all = "camelCase")]
    TurnCancelled {
        turn_id: TurnId,
        /// As [`AgentEvent::TurnFinished::at`].
        #[serde(default)]
        at: TsMillis,
    },

    /// Error block with the code, "Retry" and "Switch model" (§5.10).
    #[serde(rename_all = "camelCase")]
    Error {
        code: String,
        message: String,
        retryable: bool,
    },

    /// `Retry in 4 s · attempt 2 of 5` (§5.10).
    #[serde(rename_all = "camelCase")]
    RetryScheduled {
        attempt: u32,
        of: u32,
        delay_ms: u32,
        reason: String,
    },

    /// Reflects `SetMode` and model changes: the composer's bottom row always
    /// shows the current mode (§7.1, §8).
    #[serde(rename_all = "camelCase")]
    StateChanged { state: SessionState },

    /// A plugin hook spoke where there is no call row (see the module head).
    /// `event` is Claude Code's name for the moment (`SessionStart`,
    /// `UserPromptSubmit`, `Stop`); `blocked` says the hook stopped what was
    /// about to happen, and `text` is its reason or its message.
    #[serde(rename_all = "camelCase")]
    HookReported {
        plugin: String,
        event: String,
        text: String,
        blocked: bool,
    },
}

impl AgentEvent {
    /// The call this event belongs to, for the events that are about one.
    /// The session uses it to keep a silent call (`Tool::silent`) out of the
    /// stream whole: one place that knows every event a call can produce.
    pub fn call_id(&self) -> Option<&CallId> {
        match self {
            AgentEvent::ToolCallStarted { call_id, .. }
            | AgentEvent::ToolCallProgress { call_id, .. }
            | AgentEvent::ToolCallFinished { call_id, .. }
            | AgentEvent::ToolCallFailed { call_id, .. }
            | AgentEvent::ToolCallRejected { call_id, .. }
            | AgentEvent::ToolCallDenied { call_id, .. }
            | AgentEvent::FileChanged { call_id, .. } => Some(call_id),
            _ => None,
        }
    }

    /// The event tag: the same one that goes out in JSON, and the one the interface
    /// branches on. It lives here, not in the tests: a list of twenty-odd branches
    /// kept in two places drifts apart at the first event added.
    pub fn kind(&self) -> &'static str {
        match self {
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
}

/// Where the core puts events. A thin wrapper over the broadcast channel:
/// there may be no subscribers at all — the CLI runs the core without them.
#[derive(Clone)]
pub struct EventSink {
    tx: tokio::sync::broadcast::Sender<AgentEvent>,
    journal: Option<Journal>,
    /// Held while an event is written down and sent, so the two happen as one.
    /// Every clone of the sink shares it, whatever it journals.
    ///
    /// That is what lets a late subscriber ([`EventSink::attach`]) stand at a
    /// point in the stream: everything before it is in the journal, everything
    /// after it comes down the channel, and nothing is in both or in neither.
    gate: std::sync::Arc<std::sync::Mutex<()>>,
}

/// What a sink shows an event to before sending it on.
type Journal = std::sync::Arc<dyn Fn(&AgentEvent) + Send + Sync>;

impl EventSink {
    pub fn new(tx: tokio::sync::broadcast::Sender<AgentEvent>) -> Self {
        EventSink {
            tx,
            journal: None,
            gate: Default::default(),
        }
    }

    /// The same channel, with every event written down on the way out.
    ///
    /// For the sink a tool is handed: an event a tool sends is a part of the
    /// conversation (`TitleSet`, `TodosUpdated`), and one that reached the
    /// window but not the journal is gone at the next read of the index.
    pub fn journaled(&self, journal: impl Fn(&AgentEvent) + Send + Sync + 'static) -> Self {
        EventSink {
            tx: self.tx.clone(),
            journal: Some(std::sync::Arc::new(journal)),
            gate: self.gate.clone(),
        }
    }

    pub fn emit(&self, event: AgentEvent) {
        self.emit_after(event, |_| {});
    }

    /// [`EventSink::emit`], with `before` shown the event first: the session's
    /// own journal, for the sink that does not carry one.
    pub fn emit_after(&self, event: AgentEvent, before: impl FnOnce(&AgentEvent)) {
        // A panic elsewhere must not stop the stream: the guard protects
        // nothing but the order of two steps.
        let _held = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        before(&event);
        if let Some(journal) = &self.journal {
            journal(&event);
        }
        let _ = self.tx.send(event);
    }

    /// A subscriber standing at this very point in the stream. `at` runs while
    /// no event is being written or sent, and returns where the journal ends
    /// (what [`Recorder::journal_len`](crate::store::Recorder::journal_len)
    /// says); the receiver gets every event after it.
    pub fn attach<T>(&self, at: impl FnOnce() -> T) -> (tokio::sync::broadcast::Receiver<AgentEvent>, T) {
        let _held = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        let mark = at();
        (self.tx.subscribe(), mark)
    }
}

/// Epoch milliseconds. A `u64` in Rust, a plain `number` in TS.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS,
)]
#[ts(export)]
pub struct TsMillis(#[ts(type = "number")] pub u64);

impl TsMillis {
    pub fn now() -> Self {
        TsMillis(crate::id::now_ms())
    }
}

/// Counters of a finished tool. File deltas (`+124 −0`) travel separately,
/// in `FileChanged`: the diff counts them, not the tool.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ToolMeta {
    /// How many output lines went into the feed.
    pub lines: u32,
    /// Matches for `grep`, files for `glob`.
    pub items: Option<u32>,
    /// The output is cut; the full one is available by a button (§5.5.1).
    pub truncated: bool,
    /// How long it took, in milliseconds. Meta on the right of the row.
    pub duration_ms: u32,
    /// Images the call returned, kept as files of the project and named by
    /// paths relative to it: the row shows them and the viewer opens them.
    /// An MCP server's images arrive as bytes, and everything the window
    /// draws speaks in paths. Optional for the window, whose rows were
    /// written before there was a field; journals of those rows read it as
    /// empty.
    #[serde(default)]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub pictures: Vec<String>,
}

/// A message waiting in the queue (§7.4): its cancel handle and what the chip
/// shows. Attachments are carried as sent, so the chip can name them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct QueuedMessage {
    pub id: QueuedId,
    pub text: String,
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum TodoStatus {
    /// `○` coloured `--ink-muted`.
    Pending,
    /// `●` coloured `--vermilion-ink`, pulsing. `in_progress` is what models
    /// write from habit; the tool schema advertises `inProgress`.
    #[serde(alias = "in_progress")]
    InProgress,
    /// `✓` coloured `--moss`, text `line-through`.
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    /// The model refused on safety grounds.
    Refusal,
    /// A server-side tool paused the turn; the loop continues on its own.
    PauseTurn,
    /// The turn was cut off by an error. Beyond the provider's set: the state ring
    /// goes dark on `TurnFinished`, so this event has to arrive here too.
    Failed,
}

/// Snapshot of the session settings for the composer's bottom row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub project_root: String,
    #[serde(default)]
    pub provider: String,
    pub model: String,
    pub mode: Mode,
    pub effort: Effort,
    pub busy: bool,
    /// Permission bypass (§8). Not a fifth mode but a flag on top of the current one:
    /// the mode chip and the ring turn `--rust`.
    pub bypass: bool,
    /// The worktree this chat works in, when it was asked to be isolated
    /// (`isolation`). Absent for a chat in the project's own tree.
    #[serde(default)]
    #[ts(optional)]
    pub isolation: Option<crate::isolation::Isolation>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn numbered(n: u64) -> AgentEvent {
        AgentEvent::TurnStarted {
            turn_id: crate::id::TurnId::from(n.to_string().as_str()),
            at: TsMillis(n),
        }
    }

    fn number(event: &AgentEvent) -> u64 {
        match event {
            AgentEvent::TurnStarted { at, .. } => at.0,
            other => panic!("{other:?}"),
        }
    }

    /// The property a reload depends on: a subscriber that stands in the stream
    /// is handed the journal up to its place and the channel after it, so the
    /// two together are the stream — every event once, in order.
    ///
    /// The journal here is slow after it has recorded, which is the gap an
    /// attach would otherwise fall into: written down, not yet sent.
    #[test]
    fn a_subscriber_standing_in_the_stream_sees_each_event_exactly_once() {
        const EVENTS: u64 = 300;

        let (tx, _keep) = tokio::sync::broadcast::channel(1024);
        let written: Arc<Mutex<Vec<u64>>> = Arc::default();
        let log = written.clone();
        let sink = EventSink::new(tx).journaled(move |event| {
            log.lock().unwrap().push(number(event));
            std::thread::sleep(Duration::from_micros(300));
        });

        let emitter = {
            let sink = sink.clone();
            std::thread::spawn(move || {
                for n in 0..EVENTS {
                    sink.emit(numbered(n));
                    // Linux mutexes are not fair: an emitter that takes the lock
                    // again at once starves `attach` until the stream is over,
                    // and the test would attach too few times to mean anything.
                    // A real stream has gaps; this gives the test one, long enough for
                    // the woken waiter to take the lock (a bare `yield_now` is not).
                    std::thread::sleep(Duration::from_micros(50));
                }
            })
        };

        let mut stands = Vec::new();
        while !emitter.is_finished() {
            let log = written.clone();
            let (events, before) = sink.attach(|| log.lock().unwrap().len());
            stands.push((events, before));
            std::thread::sleep(Duration::from_micros(100));
        }
        emitter.join().unwrap();

        assert!(stands.len() > 10, "too few attaches to mean anything");
        let journal = written.lock().unwrap().clone();
        for (mut events, before) in stands {
            let mut seen: Vec<u64> = journal[..before].to_vec();
            while let Ok(event) = events.try_recv() {
                seen.push(number(&event));
            }
            let all: Vec<u64> = (0..EVENTS).collect();
            assert_eq!(seen, all, "attached after {before} journaled events");
        }
    }
}

