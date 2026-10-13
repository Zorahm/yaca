//! What each file looked like before this session first changed it, so that the
//! panel can show the whole file with every change in it (§6.4).
//!
//! There is no decision to make about a change here. A write lands in the
//! working tree at once, and the one way back is a checkpoint (§5.8); the panel
//! only shows. Each file keeps two texts:
//!
//! * `baseline` — what the file held before Yaca first wrote it. `None` when
//!   the file did not exist.
//! * `after` — what Yaca last wrote. `None` when Yaca removed the file.
//!
//! The file's view is the difference between the two. A file written back to
//! its baseline has no difference and drops out.

use std::path::PathBuf;
use std::sync::Mutex;

use super::diff::LineKind;
use super::view::{self, FileView};
use super::{ChangeKind, FileFact};

#[derive(Debug, Clone)]
struct Tracked {
    path: String,
    abs: PathBuf,
    baseline: Option<String>,
    after: Option<String>,
}

impl Tracked {
    fn changed(&self) -> bool {
        text(&self.baseline) != text(&self.after)
    }
}

/// The baselines of one session's files.
#[derive(Debug, Default)]
pub struct ChangeSet {
    files: Mutex<Vec<Tracked>>,
}

impl ChangeSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Notes a write Yaca has just made. `before` is the file before the call,
    /// `after` is the file now. A file already tracked keeps its baseline: the
    /// view is "this session's changes", not "this call's".
    pub fn record(&self, path: String, abs: PathBuf, before: Option<String>, after: Option<String>) {
        let mut files = self.files.lock().unwrap();
        match files.iter().position(|t| t.abs == abs) {
            Some(at) => files[at].after = after,
            None => files.push(Tracked {
                path,
                abs,
                baseline: before,
                after,
            }),
        }
        files.retain(|t| t.changed());
    }

    /// Drops a file. Used when something other than a write by Yaca changed it,
    /// as a rollback to a checkpoint does: its baseline describes a tree that
    /// no longer exists.
    pub fn forget(&self, path: &str) {
        self.files.lock().unwrap().retain(|t| t.path != path);
    }

    /// The full view of one file (§6.4), or `None` when the session has not
    /// changed it.
    pub fn view(&self, path: &str) -> Option<FileView> {
        let files = self.files.lock().unwrap();
        let t = files.iter().find(|t| t.path == path && t.changed())?;
        Some(view::view(text(&t.baseline), text(&t.after), Some(fact(t))))
    }
}

/// The fact about one tracked file: what changed, and how much.
fn fact(t: &Tracked) -> FileFact {
    let hunks = view::hunks(text(&t.baseline), text(&t.after));
    let count = |kind| {
        hunks
            .iter()
            .map(|h| h.lines.iter().filter(|l| l.kind == kind).count() as u32)
            .sum()
    };
    FileFact {
        path: t.path.clone(),
        kind: match (&t.baseline, &t.after) {
            (None, _) => ChangeKind::Created,
            (Some(_), None) => ChangeKind::Deleted,
            (Some(_), Some(_)) => ChangeKind::Modified,
        },
        moved_from: None,
        added: count(LineKind::Add),
        removed: count(LineKind::Del),
    }
}

fn text(s: &Option<String>) -> &str {
    s.as_deref().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    #[test]
    fn a_second_write_keeps_the_first_baseline() {
        let set = ChangeSet::new();
        set.record("a.txt".into(), abs("a.txt"), Some("one\n".into()), Some("two\n".into()));
        set.record("a.txt".into(), abs("a.txt"), Some("two\n".into()), Some("three\n".into()));

        let fact = set.view("a.txt").unwrap().fact.unwrap();
        assert_eq!(fact.kind, ChangeKind::Modified);
        assert_eq!((fact.added, fact.removed), (1, 1));
    }

    #[test]
    fn a_created_file_is_shown_as_created() {
        let set = ChangeSet::new();
        set.record("a.txt".into(), abs("a.txt"), None, Some("первая\n".into()));

        assert_eq!(set.view("a.txt").unwrap().fact.unwrap().kind, ChangeKind::Created);
    }

    #[test]
    fn writing_back_the_baseline_leaves_nothing_to_show() {
        let set = ChangeSet::new();
        set.record("a.txt".into(), abs("a.txt"), Some("x\n".into()), Some("y\n".into()));
        set.record("a.txt".into(), abs("a.txt"), Some("y\n".into()), Some("x\n".into()));

        assert!(set.view("a.txt").is_none());
    }

    #[test]
    fn a_file_changed_by_a_rollback_is_forgotten() {
        let set = ChangeSet::new();
        set.record("a.txt".into(), abs("a.txt"), Some("x\n".into()), Some("y\n".into()));
        set.forget("a.txt");

        assert!(set.view("a.txt").is_none());
    }

    #[test]
    fn a_file_the_session_never_wrote_has_no_view() {
        assert!(ChangeSet::new().view("never.txt").is_none());
    }
}
