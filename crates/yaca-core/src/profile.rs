//! What kind of work a chat is for: code or documents.
//!
//! This is not the permission axis. [`Mode`](crate::intent::Mode) says how far
//! Yaca may go — read only, ask about everything, edit freely — and this says
//! what the work is. The two are independent: a chat about a report still asks
//! before writing and running, and a cautious chat about code is exactly what
//! "Manual" is for. Keeping them apart is what lets the mode menu stay a menu
//! of trust levels instead of turning into a list of jobs.
//!
//! What a profile changes is the system prompt and the set of tools in the
//! request. Most tools are the same in both: reading, searching, writing and
//! running commands are how documents are handled too — an office file is
//! produced by a script, not typed in as bytes. What `Chat` leaves out is what
//! only means something for a codebase: language servers and notebooks.
//!
//! `Code` is the default, as it is in the window: Yaca is a coding agent, and a
//! chat that says nothing about its kind is a chat about code.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What kind of work the chat is for (§7.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Profile {
    /// A codebase: the usual agent, with language servers and notebooks.
    #[default]
    Code,
    /// Documents and the person's files: the same tools, without the ones that
    /// only mean something in a repository, and a prompt that talks about Word,
    /// Excel and PowerPoint instead of source files.
    Chat,
}

impl Profile {
    /// Both, in the order a menu shows them.
    pub const ALL: &'static [Profile] = &[Profile::Code, Profile::Chat];
}
