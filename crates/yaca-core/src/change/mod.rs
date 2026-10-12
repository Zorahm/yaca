//! A file change: what happened, and how it looks as a diff (§5.5, §6.3).
//!
//! Writing tools write to the working tree right away, and the decision about
//! the write is made once, by the permission request before it (§8, §16.37).
//! What the feed needs is still here: the fact about the file that the action
//! caption is written from, and the diff that expands it (`diff`).
//!
//! The panel shows a whole file with everything the session changed in it
//! (§6.4): `set` remembers what each file held before the first write, `view`
//! draws the difference. Nothing there can be decided — the one way back is a
//! checkpoint (§5.8).

pub mod diff;
pub mod set;
pub mod view;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use diff::{DiffLine, FileDiff, Hunk, LineKind, Span};
pub use set::ChangeSet;
pub use view::{FileView, ViewSegment};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Created,
    Modified,
    Deleted,
    Moved,
}

/// A fact about a file action — the interface writes the action caption from
/// it (§5.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FileFact {
    pub path: String,
    pub kind: ChangeKind,
    pub moved_from: Option<String>,
    pub added: u32,
    pub removed: u32,
}

impl FileFact {
    /// The fact and the diff of one change, computed from what was there before
    /// and what is there now — the tool knows both contents at write time.
    pub fn of(
        path: String,
        kind: ChangeKind,
        before: Option<&str>,
        after: Option<&str>,
    ) -> (FileFact, FileDiff) {
        let d = diff::compute(before.unwrap_or(""), after.unwrap_or(""));
        let fact = FileFact {
            path,
            kind,
            moved_from: None,
            added: d.added,
            removed: d.removed,
        };
        (fact, d)
    }
}
