//! Commands going into the core — the other side of §15.
//!
//! We keep only what the core can actually carry out: declaring a command that
//! silently does nothing is exactly the case where the interface lies about
//! the state.
//!
//! Two of these are beyond §15's list.
//!
//! [`Command::PreviewCheckpoint`] — §5.8 requires a rollback to show the files
//! it will touch and be confirmed against them, so there has to be a way to ask
//! what a rollback would do without doing it.
//!
//! [`Command::AnswerQuestion`] — the answer to an `AskUser` question. §15 has
//! no command for it because §5.7 has no question beyond permissions; this is
//! the same road the permission answer takes, with its own reply shape.
//!
//! [`Command::EditSummary`] — §5.11 lets the human rewrite the summary before
//! the conversation continues, and what they write has to reach the history the
//! model is given.
//!
//! The file panel adds one more, for §6.4: [`Command::ReadFileView`] (the whole
//! file, for the panel). §15 has none: it draws the diff of a call. There is no
//! command that decides a hunk — the one way back is a checkpoint.
//!
//! Two more came with the queue and the mention search, both beyond §15's list:
//! [`Command::CancelQueued`] (§7.4, the chip's "cancel") and
//! [`Command::SearchFiles`] (§7.2, the `@` menu). Their answers are
//! `QueueUpdated` and `FilesFound`, which are in the header of `event.rs`.
//!
//! MCP adds two, both beyond §15's list: [`Command::ReconnectMcpServer`] and
//! [`Command::ApproveMcpServer`]. Their answer is `McpUpdated`.
//!
//! What is deliberately *not* here is the session panel of §9: listing,
//! renaming, duplicating, exporting and deleting sessions. This channel talks
//! to the session that is running, and last week's work is not something the
//! current turn has an opinion about. Those live on
//! [`SessionStore`](crate::store::SessionStore).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::{CheckpointId, QueuedId, RequestId};
use crate::intent::{Effort, Mode};
use crate::permission::PermissionAnswer;
use crate::process::ProcessId;
use crate::question::QuestionReply;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    #[serde(rename_all = "camelCase")]
    SendMessage {
        text: String,
        /// Files attached via `@` (§7.2). They are registered as read: the human
        /// attached them, so making the agent read them again would be pointless.
        #[serde(default)]
        attachments: Vec<String>,
    },

    /// `Esc`. Unfinished tools get "cancelled".
    Cancel,

    /// Mode switch in the bottom row of the composer (§7.1, §8).
    #[serde(rename_all = "camelCase")]
    SetMode { mode: Mode },

    #[serde(rename_all = "camelCase")]
    SetModel { model: String },

    #[serde(rename_all = "camelCase")]
    SetEffort { effort: Effort },

    /// What rolling back to this point would touch (§5.8). Answers with
    /// `CheckpointPreviewed`; nothing on disk moves.
    #[serde(rename_all = "camelCase")]
    PreviewCheckpoint { id: CheckpointId },

    /// "Rollback here" (§5.8), once the human has confirmed the list.
    #[serde(rename_all = "camelCase")]
    RestoreCheckpoint { id: CheckpointId },

    /// "Compact" (§5.11, §6.1.1). The core also does this on its own once the
    /// context is nearly full; the button is for doing it sooner.
    Compact,

    /// §5.11: the summary can be rewritten before the conversation continues.
    #[serde(rename_all = "camelCase")]
    EditSummary { text: String },

    /// Answer to the active permission request (§5.7).
    #[serde(rename_all = "camelCase")]
    AnswerPermission {
        request_id: RequestId,
        answer: PermissionAnswer,
    },

    /// Answer to an `AskUser` question with options. Not in §15: §5.7 gives an
    /// answer only to permissions. Declining to answer is the same `Dismissed`
    /// variant rather than a separate command, so that a question has one way to
    /// close.
    #[serde(rename_all = "camelCase")]
    AnswerQuestion {
        request_id: RequestId,
        answer: QuestionReply,
    },

    /// Bypass of permissions. The human confirms turning it on; the flag does not
    /// survive a restart and is reset when the project changes (§8).
    #[serde(rename_all = "camelCase")]
    SetBypass { on: bool },

    /// Selecting a row in the process panel (§6.5). `None` — selection cleared.
    ///
    /// Output flows out only for the selected row. A chatty `npm run dev` nobody
    /// is looking at would push the real feed events out of the broadcast
    /// channel, and a lagging subscriber loses them.
    #[serde(rename_all = "camelCase")]
    WatchProcess { id: Option<ProcessId> },

    /// "Stop" in the process panel (§6.5). Hits the whole tree that the
    /// command has spawned.
    #[serde(rename_all = "camelCase")]
    StopProcess { id: ProcessId },

    /// The whole file for the panel, with its unchanged lines (§6.4). Answers
    /// with `FileViewLoaded`; nothing on disk moves.
    #[serde(rename_all = "camelCase")]
    ReadFileView { path: String },

    /// Take a message out of the queue before the turn takes it in (§7.4). A
    /// message the turn has already taken in, or one that was never queued, is
    /// left alone: the queue as the core reports it is the answer either way.
    #[serde(rename_all = "camelCase")]
    CancelQueued { id: QueuedId },

    /// `@` (§7.2): the project's files matching `query`. Answers with
    /// `FilesFound`; the walk runs off the dispatcher and nothing on disk moves.
    #[serde(rename_all = "camelCase")]
    SearchFiles { query: String },

    /// Drop the connection to an MCP server, if there is one, and connect
    /// again: the way back for a server that failed or went away. A name the
    /// session does not have is ignored.
    #[serde(rename_all = "camelCase")]
    ReconnectMcpServer { server: String },

    /// The human's "yes" to a server declared in the project's `.mcp.json`,
    /// which is not started until they give it (`mcp::config`). Remembered for
    /// the project until the declaration changes.
    #[serde(rename_all = "camelCase")]
    ApproveMcpServer { server: String },

    /// The human's answer to an MCP server's form. An answer that does not
    /// fit the form is refused with an `Error`, and the form stays open.
    #[serde(rename_all = "camelCase")]
    AnswerElicitation {
        request_id: RequestId,
        reply: crate::mcp::ElicitationReply,
    },

    /// Start the OAuth login of a remote MCP server that asked for one. The
    /// page to open comes back in `McpUpdated` as the server's `loginUrl`.
    #[serde(rename_all = "camelCase")]
    LoginMcpServer { server: String },

    /// Forget a remote MCP server's OAuth login.
    #[serde(rename_all = "camelCase")]
    LogoutMcpServer { server: String },

    /// Stop a language server and start it again (the `/lsp` panel). Its
    /// answer is `LspUpdated`. A name the session does not have is ignored.
    #[serde(rename_all = "camelCase")]
    RestartLspServer { server: String },
}
