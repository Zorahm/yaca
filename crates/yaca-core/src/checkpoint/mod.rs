//! Checkpoints (§5.8): the shadow repository, and the way back from it.
//!
//! A checkpoint is a snapshot of the working tree taken before something that
//! cannot be undone: before the change set is written to disk, before a
//! permitted command runs, and before each message, which is where `Esc Esc`
//! comes back to (§13).
//!
//! Three decisions shape the whole module.
//!
//! *The project's ignore list has no say.* §5.8 requires `.env` to come back,
//! and a user's `.gitignore` is exactly the list of things git would refuse to
//! restore. So this is not a guest inside the project's `.git`: it is Yaca's own
//! store under `.yaca/shadow`, with its own list of what to skip — see
//! [`scan`].
//!
//! *A snapshot must be affordable.* One is taken before every command, so it
//! cannot re-read the repository. The walk keeps a stat cache and re-reads only
//! files whose size or mtime moved; content is stored under its own hash, so
//! anything already stored costs nothing.
//!
//! *A snapshot that changes nothing is not a checkpoint.* Equal trees encode to
//! one manifest and therefore to one name, and a snapshot that repeats the
//! previous one — same tree, same kind of reason — is not recorded. Without
//! that, twenty `cargo check`s would leave twenty dividers in the feed, each
//! offering to roll back to the state the feed is already in.
//!
//! The kind of reason is part of that comparison, and the value is not. Two
//! commands over an unchanged tree are one point, because rolling back to
//! either does the same thing. A message and an apply over an unchanged tree
//! are two, because the divider is also a label, and `before applying
//! 3 files` is what the human will be looking for when they want that apply
//! undone.
//!
//! What the core does not do here is write the sentence. The reason is
//! structured — [`CheckpointReason`] — and `before applying 3 files` is
//! written by the feed, on the same rule as §5.4.1 and the permission block:
//! the core names the action, the interface words it.

pub mod gc;
pub mod ledger;
pub mod manifest;
pub mod restore;
pub mod scan;
pub mod store;

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use blake3::Hash;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::change::ChangeKind;
use crate::event::TsMillis;
use crate::id::CheckpointId;

use manifest::Manifest;
use scan::Scanner;
use store::Store;

/// Why the snapshot was taken. Named, not worded: the feed writes the line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CheckpointReason {
    /// Before a turn starts. This is where `Esc Esc` comes back to (§13), and
    /// where §5.1's "roll back to before this message" points.
    Message,
    /// Before a permitted command runs (§5.8).
    #[serde(rename_all = "camelCase")]
    Command { command: String },
    /// Before a rollback overwrites the tree. A rollback is destructive like
    /// anything else here, and it does not get to be the one thing without a
    /// way back.
    Rollback,
}

/// A point in the checkpoint list, and the divider line in the feed (§5.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointSummary {
    pub id: CheckpointId,
    pub at: TsMillis,
    pub reason: CheckpointReason,
    /// How many files the snapshot holds. Not the `3 files` of a rollback —
    /// that number is counted against the tree at the moment of rolling back.
    pub tracked: u32,
}

/// One line of the list a rollback shows before it is confirmed (§5.8).
///
/// The kind is what the rollback does to the file: `Created` brings it back,
/// `Deleted` takes it away, `Modified` rewrites it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointChange {
    pub path: String,
    pub kind: ChangeKind,
    pub added: u32,
    pub removed: u32,
}

/// The outcome of a rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// The point that was rolled back to — the feed needs its time for
    /// `Rolled back to 14:32`.
    pub checkpoint: CheckpointSummary,
    /// Paths that were written or removed, relative to the project root.
    pub touched: Vec<String>,
    /// Paths the rollback could not touch. A partial rollback is reported as
    /// partial; §0 leaves no room for reporting it as whole.
    pub failed: Vec<String>,
}

#[derive(Debug, Clone)]
struct Record {
    summary: CheckpointSummary,
    /// The name the manifest has in the store.
    manifest: Hash,
}

struct State {
    scanner: Scanner,
    records: Vec<Record>,
}

struct Inner {
    root: PathBuf,
    store: Store,
    /// Where the list of points is kept between runs, if it is (`ledger`).
    ledger: Option<PathBuf>,
    /// Scanning and recording under one lock. Every entry point here blocks on
    /// the filesystem and is called from a blocking task; two snapshots racing
    /// each other would only duplicate the walk.
    state: Mutex<State>,
}

/// The shadow repository of one project.
#[derive(Clone)]
pub struct Checkpoints {
    inner: Arc<Inner>,
}

impl Checkpoints {
    /// `exclude` is what the project added to the built-in skip list (§10).
    ///
    /// The list of points lives in memory only: for a run that is not written
    /// down, which is what a one-shot CLI run is.
    pub fn open(root: impl Into<PathBuf>, exclude: &[String]) -> io::Result<Self> {
        let root = root.into();
        Self::open_in(root.clone(), &root, exclude, None)
    }

    /// A tree that is not where the project's `.yaca` is — a chat's worktree
    /// (`isolation`). The points are of `root` and only of it; the objects go
    /// to the project's store, which is safe to share because a name is the
    /// hash of what is under it.
    ///
    /// `ledger` is the file the list of points is kept in. It is read now —
    /// a session that is reopened, or whose window was reloaded, finds the
    /// points it had — and rewritten whenever the list grows.
    pub fn open_in(
        root: impl Into<PathBuf>,
        home: &std::path::Path,
        exclude: &[String],
        ledger: Option<PathBuf>,
    ) -> io::Result<Self> {
        let root = root.into();
        let store = Store::open(home)?;
        let scanner = Scanner::with_excludes(root.clone(), exclude);
        let records = match &ledger {
            Some(path) => ledger::load(path, &store),
            None => Vec::new(),
        };
        // Held by this process from the first moment, so a sweep running as
        // the session starts cannot take what the list is about to show.
        for record in &records {
            gc::remember(&store, record.manifest);
        }
        Ok(Checkpoints {
            inner: Arc::new(Inner {
                root,
                store,
                ledger,
                state: Mutex::new(State { scanner, records }),
            }),
        })
    }

    /// Every point of this session, oldest first — the checkpoint panel (§1.1).
    pub fn list(&self) -> Vec<CheckpointSummary> {
        self.inner
            .state
            .lock()
            .unwrap()
            .records
            .iter()
            .map(|r| r.summary.clone())
            .collect()
    }

    /// Takes a snapshot. `None` when it would repeat the previous point: same
    /// tree, same kind of reason, and therefore nothing to come back to that is
    /// not already there.
    pub fn create(&self, reason: CheckpointReason) -> io::Result<Option<CheckpointSummary>> {
        let mut state = self.inner.state.lock().unwrap();
        let manifest = state.scanner.scan(&self.inner.store)?;
        self.inner.record(&mut state, &manifest, reason)
    }

    /// The name the point's manifest has in the store. Lets the sweep tests
    /// say which manifests are live without reaching into the list.
    #[cfg(test)]
    pub(crate) fn manifest_name(&self, id: &CheckpointId) -> Option<Hash> {
        let state = self.inner.state.lock().unwrap();
        find(&state, id).map(|r| r.manifest)
    }

    /// Runs the sweep of the shadow store on a blocking task and does not wait
    /// for it: a session starts while the walk is still going. See [`gc`].
    pub fn sweep_in_background(&self) {
        let inner = self.inner.clone();
        // Detached on purpose. A sweep that fails or panics costs disk space,
        // never the session.
        tokio::task::spawn_blocking(move || gc::run_keeping(&inner.store, &inner.kept()));
    }

    /// What rolling back would touch. `None` — no such point.
    pub fn preview(&self, id: &CheckpointId) -> io::Result<Option<Vec<CheckpointChange>>> {
        let mut state = self.inner.state.lock().unwrap();
        let Some(record) = find(&state, id) else {
            return Ok(None);
        };
        let target = self.inner.manifest_of(&record.manifest)?;
        let current = state.scanner.scan(&self.inner.store)?;
        Ok(Some(restore::plan(&self.inner.store, &target, &current)))
    }

    /// Rolls the working tree back. `None` — no such point.
    ///
    /// The plan is recomputed here rather than carried over from
    /// [`Checkpoints::preview`]: the human confirmed a rollback to a point, and
    /// between the question and the answer the tree may have moved again.
    pub fn restore(&self, id: &CheckpointId) -> io::Result<Option<Restored>> {
        let mut state = self.inner.state.lock().unwrap();
        let Some(record) = find(&state, id) else {
            return Ok(None);
        };
        let target = self.inner.manifest_of(&record.manifest)?;
        let current = state.scanner.scan(&self.inner.store)?;
        let plan = restore::plan(&self.inner.store, &target, &current);

        if plan.is_empty() {
            return Ok(Some(Restored {
                checkpoint: record.summary,
                touched: Vec::new(),
                failed: Vec::new(),
            }));
        }

        self.inner
            .record(&mut state, &current, CheckpointReason::Rollback)?;
        let report = restore::apply(&self.inner.store, &self.inner.root, &target, &plan)?;

        Ok(Some(Restored {
            checkpoint: record.summary,
            touched: report.touched,
            failed: report.failed,
        }))
    }
}

impl Inner {
    fn record(
        &self,
        state: &mut State,
        manifest: &Manifest,
        reason: CheckpointReason,
    ) -> io::Result<Option<CheckpointSummary>> {
        let hash = self.store.put_manifest(&manifest.encode())?;
        gc::remember(&self.store, hash);
        if let Some(last) = state.records.last() {
            let same_kind =
                std::mem::discriminant(&last.summary.reason) == std::mem::discriminant(&reason);
            if last.manifest == hash && same_kind {
                return Ok(None);
            }
        }
        let summary = CheckpointSummary {
            id: CheckpointId::new(),
            at: TsMillis::now(),
            reason,
            tracked: manifest.entries.len() as u32,
        };
        state.records.push(Record {
            summary: summary.clone(),
            manifest: hash,
        });
        // The point exists whether or not it could be written down; the only
        // thing lost is that a reload will not find it, and the log says so.
        // Failing the snapshot over it would trade a missing safety net for a
        // dead agent, the same trade `session::checkpoints::create` refuses.
        if let Some(path) = &self.ledger {
            if let Err(e) = ledger::save(path, &state.records) {
                tracing::warn!(path = %path.display(), error = %e, "could not save the checkpoint list");
            }
        }
        Ok(Some(summary))
    }

    /// The manifests the saved lists of the project's sessions name — the
    /// sleeping ones too — read now, at the time of a sweep and not before.
    fn kept(&self) -> std::collections::HashSet<Hash> {
        self.ledger
            .as_deref()
            .and_then(|path| path.parent())
            .map(ledger::manifests)
            .unwrap_or_default()
    }

    fn manifest_of(&self, hash: &Hash) -> io::Result<Manifest> {
        Manifest::decode(&self.store.get_manifest(hash)?)
    }
}

fn find(state: &State, id: &CheckpointId) -> Option<Record> {
    state.records.iter().find(|r| &r.summary.id == id).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    /// Filesystem clocks are coarse enough that two writes land on one tick.
    fn tick() {
        std::thread::sleep(std::time::Duration::from_millis(15));
    }

    fn setup() -> (TempDir, Checkpoints) {
        let dir = TempDir::new();
        let checkpoints = Checkpoints::open(dir.path(), &[]).unwrap();
        (dir, checkpoints)
    }

    #[test]
    fn read_only_commands_do_not_pile_up_dividers() {
        let (dir, ck) = setup();
        dir.write("a.rs", "x\n");

        let command = |c: &str| CheckpointReason::Command { command: c.into() };
        assert!(ck.create(command("cargo check")).unwrap().is_some());
        tick();
        // Rolling back to either does the same thing, so they are one point —
        // which command named it is beside the point.
        assert!(ck.create(command("cargo check")).unwrap().is_none());
        assert!(ck.create(command("cargo tree")).unwrap().is_none());
        assert_eq!(ck.list().len(), 1);
    }

    #[test]
    fn a_changed_tree_gets_its_own_point() {
        let (dir, ck) = setup();
        dir.write("a.rs", "one\n");
        ck.create(CheckpointReason::Message).unwrap();

        tick();
        dir.write("a.rs", "two\n");
        let second = ck.create(CheckpointReason::Message).unwrap().unwrap();

        assert_eq!(ck.list().len(), 2);
        assert_eq!(second.reason, CheckpointReason::Message);
    }

    #[test]
    fn rolling_back_returns_the_tree_and_names_the_files() {
        let (dir, ck) = setup();
        let kept = dir.write("kept.rs", "x\n");
        let edited = dir.write("edited.rs", "before\n");
        let point = ck.create(CheckpointReason::Message).unwrap().unwrap();

        tick();
        std::fs::write(&edited, "after\n").unwrap();
        let added = dir.write("added.rs", "x\n");

        let preview = ck.preview(&point.id).unwrap().unwrap();
        let named: Vec<_> = preview.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(named, vec!["added.rs", "edited.rs"]);

        let done = ck.restore(&point.id).unwrap().unwrap();
        assert_eq!(done.touched.len(), 2);
        assert!(done.failed.is_empty());
        assert_eq!(std::fs::read_to_string(&edited).unwrap(), "before\n");
        assert!(!added.exists());
        assert!(kept.exists());
    }

    #[test]
    fn the_state_before_a_rollback_becomes_a_point_of_its_own() {
        let (dir, ck) = setup();
        let path = dir.write("a.rs", "one\n");
        let first = ck.create(CheckpointReason::Message).unwrap().unwrap();

        tick();
        std::fs::write(&path, "two\n").unwrap();
        ck.restore(&first.id).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\n");

        // A rollback is destructive too, and does not get to be the one
        // destructive thing without a way back.
        let rollback = ck
            .list()
            .into_iter()
            .find(|c| c.reason == CheckpointReason::Rollback)
            .expect("the tree before the rollback must be a point");
        tick();
        ck.restore(&rollback.id).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two\n");
    }

    #[test]
    fn rolling_back_to_the_current_state_touches_nothing() {
        let (dir, ck) = setup();
        dir.write("a.rs", "x\n");
        let point = ck.create(CheckpointReason::Message).unwrap().unwrap();

        tick();
        let done = ck.restore(&point.id).unwrap().unwrap();
        assert!(done.touched.is_empty());
        assert_eq!(ck.list().len(), 1, "and adds no point either");
    }

    /// What survives a restart for a run whose list is not saved (a one-shot
    /// CLI run), pinned down. Content does; the list does not. A new instance
    /// knows no earlier point, so it cannot list one or roll back to one, and
    /// it records the same tree again as a new point.
    #[test]
    fn a_reopened_store_keeps_content_but_not_the_list() {
        let dir = TempDir::new();
        let first = {
            let ck = Checkpoints::open(dir.path(), &[]).unwrap();
            dir.write("a.rs", "kept\n");
            ck.create(CheckpointReason::Message).unwrap().unwrap()
        };

        let reopened = Checkpoints::open(dir.path(), &[]).unwrap();
        assert!(reopened.list().is_empty());
        assert!(reopened.restore(&first.id).unwrap().is_none());
        let again = reopened.create(CheckpointReason::Message).unwrap();
        assert!(again.is_some(), "the same tree is a new point to this instance");

        // The bytes are still on disk for anything that still holds the name.
        let store = Store::open(dir.path()).unwrap();
        assert!(store.has(&Store::hash(b"kept\n")));
    }

    /// The project of a session whose list is saved. The list lives in a
    /// folder of its own beside it, as the journals do: inside the project it
    /// would be one more file the snapshots take.
    fn project(dir: &TempDir) -> PathBuf {
        dir.mkdir("project")
    }

    fn persisted(dir: &TempDir) -> io::Result<Checkpoints> {
        let ledger = ledger::path(&dir.path().join("chats"), &"ses_a".into());
        let root = project(dir);
        Checkpoints::open_in(root.clone(), &root, &[], Some(ledger))
    }

    #[test]
    fn a_saved_list_comes_back_and_still_rolls_back() {
        let dir = TempDir::new();
        let file = dir.write("project/a.rs", "one
");
        let first = {
            let ck = persisted(&dir).unwrap();
            ck.create(CheckpointReason::Message).unwrap().unwrap()
        };
        tick();
        std::fs::write(&file, "two
").unwrap();

        // The window was reloaded, or the chat was closed and opened again:
        // a new instance over the same list.
        let reopened = persisted(&dir).unwrap();
        assert_eq!(reopened.list(), vec![first.clone()]);

        let preview = reopened.preview(&first.id).unwrap().unwrap();
        assert_eq!(preview.len(), 1);
        reopened.restore(&first.id).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "one
");
    }

    #[test]
    fn a_reopened_list_does_not_repeat_the_tree_it_already_holds() {
        let dir = TempDir::new();
        dir.write("project/a.rs", "one
");
        persisted(&dir)
            .unwrap()
            .create(CheckpointReason::Message)
            .unwrap()
            .unwrap();

        let reopened = persisted(&dir).unwrap();
        tick();
        // A reload must not add a divider that rolls back to where we are.
        assert!(reopened.create(CheckpointReason::Message).unwrap().is_none());
        assert_eq!(reopened.list().len(), 1);
    }

    #[test]
    fn the_list_keeps_growing_across_reopens() {
        let dir = TempDir::new();
        let file = dir.write("project/a.rs", "one
");
        let first = persisted(&dir)
            .unwrap()
            .create(CheckpointReason::Message)
            .unwrap()
            .unwrap();
        tick();
        std::fs::write(&file, "two
").unwrap();

        let reopened = persisted(&dir).unwrap();
        let second = reopened
            .create(CheckpointReason::Message)
            .unwrap()
            .unwrap();

        let third = persisted(&dir).unwrap();
        let ids: Vec<_> = third.list().into_iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![first.id, second.id]);
    }

    #[test]
    fn a_rollback_is_saved_with_the_points_it_made() {
        let dir = TempDir::new();
        let file = dir.write("project/a.rs", "one
");
        let ck = persisted(&dir).unwrap();
        let first = ck.create(CheckpointReason::Message).unwrap().unwrap();
        tick();
        std::fs::write(&file, "two
").unwrap();
        ck.restore(&first.id).unwrap().unwrap();

        let reopened = persisted(&dir).unwrap();
        assert!(reopened
            .list()
            .iter()
            .any(|c| c.reason == CheckpointReason::Rollback));
    }

    #[test]
    fn the_sweep_is_told_about_the_points_of_every_saved_list() {
        let dir = TempDir::new();
        dir.write("project/a.rs", "one
");
        let ck = persisted(&dir).unwrap();
        let point = ck.create(CheckpointReason::Message).unwrap().unwrap();
        let manifest = ck.manifest_name(&point.id).unwrap();

        // Another session of the project, asleep: its list is a file, and
        // that is all that tells the sweep its snapshot is wanted.
        let root = project(&dir);
        let other = Checkpoints::open_in(
            root.clone(),
            &root,
            &[],
            Some(ledger::path(&dir.path().join("chats"), &"ses_b".into())),
        )
        .unwrap();
        assert!(other.inner.kept().contains(&manifest));
        assert!(
            Checkpoints::open(project(&dir), &[]).unwrap().inner.kept().is_empty(),
            "a run that saves nothing names nothing"
        );
    }

    #[test]
    fn an_unknown_point_is_not_an_error() {
        let (_d, ck) = setup();
        let stranger = CheckpointId::new();
        assert!(ck.preview(&stranger).unwrap().is_none());
        assert!(ck.restore(&stranger).unwrap().is_none());
    }
}
