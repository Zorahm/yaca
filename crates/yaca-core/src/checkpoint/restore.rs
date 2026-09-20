//! Coming back to a checkpoint (§5.8).
//!
//! The list of files is produced before anything is written, because the spec
//! requires the rollback to be confirmed against it. List and rollback are the
//! same computation — the snapshot against a fresh scan — so what the human
//! confirms is what happens, and not a guess made a moment earlier.
//!
//! Only tracked files are deleted. Anything the scan never looks at — build
//! output, a file above the size cap, a name the manifest cannot spell — is in
//! neither side of the comparison, and a rollback reaching it would be deleting
//! what it never saved.

use std::cmp::Ordering;
use std::io;
use std::path::{Component, Path, PathBuf};

use super::manifest::{Entry, Manifest, Mode};
use super::store::Store;
use super::CheckpointChange;
use crate::change::{diff, ChangeKind};

/// What actually happened on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestoreReport {
    /// Paths the rollback wrote or removed, relative to the project root.
    pub touched: Vec<String>,
    /// Paths it meant to touch and could not. Empty in the ordinary case; when
    /// it is not, the human is told rather than left with a partial rollback
    /// reported as a whole one.
    pub failed: Vec<String>,
}

/// What rolling back to `target` would do to the tree described by `current`.
///
/// The kinds are named from the file's point of view, not the snapshot's:
/// `Created` is a file the rollback brings back, `Deleted` one it takes away.
pub fn plan(store: &Store, target: &Manifest, current: &Manifest) -> Vec<CheckpointChange> {
    let mut out = Vec::new();
    let (mut t, mut c) = (0usize, 0usize);

    loop {
        match (target.entries.get(t), current.entries.get(c)) {
            (Some(want), Some(have)) => match want.path.cmp(&have.path) {
                Ordering::Less => {
                    out.push(change(store, ChangeKind::Created, None, Some(want)));
                    t += 1;
                }
                Ordering::Greater => {
                    out.push(change(store, ChangeKind::Deleted, Some(have), None));
                    c += 1;
                }
                Ordering::Equal => {
                    if want.hash != have.hash || want.mode != have.mode {
                        out.push(change(store, ChangeKind::Modified, Some(have), Some(want)));
                    }
                    t += 1;
                    c += 1;
                }
            },
            (Some(want), None) => {
                out.push(change(store, ChangeKind::Created, None, Some(want)));
                t += 1;
            }
            (None, Some(have)) => {
                out.push(change(store, ChangeKind::Deleted, Some(have), None));
                c += 1;
            }
            (None, None) => break,
        }
    }
    out
}

/// Carries out a plan. Every object it needs is checked first: a rollback that
/// stops halfway leaves a tree that matches neither end.
pub fn apply(
    store: &Store,
    root: &Path,
    target: &Manifest,
    plan: &[CheckpointChange],
) -> io::Result<RestoreReport> {
    for change in plan {
        if change.kind == ChangeKind::Deleted {
            continue;
        }
        let entry = target
            .get(&change.path)
            .ok_or_else(|| io::Error::other(format!("snapshot lost {}", change.path)))?;
        if !store.has(&entry.hash) {
            return Err(io::Error::other(format!(
                "snapshot content for {} is missing from the store",
                change.path
            )));
        }
    }

    let mut report = RestoreReport::default();
    for change in plan {
        let Some(path) = inside(root, &change.path) else {
            report.failed.push(change.path.clone());
            continue;
        };
        let done = match change.kind {
            ChangeKind::Deleted => remove(root, &path),
            // A move is never planned: the rollback sees the two paths it
            // touches as one file arriving and another leaving.
            _ => match target.get(&change.path) {
                Some(entry) => write(store, &path, entry),
                None => false,
            },
        };
        if done {
            report.touched.push(change.path.clone());
        } else {
            report.failed.push(change.path.clone());
        }
    }
    Ok(report)
}

fn change(
    store: &Store,
    kind: ChangeKind,
    have: Option<&Entry>,
    want: Option<&Entry>,
) -> CheckpointChange {
    let path = want.or(have).map(|e| e.path.clone()).unwrap_or_default();
    // Counted the way the rollback will read: from what is on disk now to what
    // the snapshot holds. Binary content simply has no line counts, and a
    // rollback of a `.png` says so by showing none.
    let before = have.and_then(|e| store.get_text(&e.hash).ok());
    let after = want.and_then(|e| store.get_text(&e.hash).ok());
    let (added, removed) = match (&before, &after) {
        (None, None) => (0, 0),
        (before, after) => {
            let d = diff::compute(
                before.as_deref().unwrap_or(""),
                after.as_deref().unwrap_or(""),
            );
            (d.added, d.removed)
        }
    };
    CheckpointChange {
        path,
        kind,
        added,
        removed,
    }
}

/// Resolves a manifest path against the root, refusing anything that would
/// leave the project. The manifest is ours, but a rollback writes files, and a
/// writer that trusts its input is a writer waiting for a corrupt store.
fn inside(root: &Path, rel: &str) -> Option<PathBuf> {
    let candidate = Path::new(rel);
    if candidate
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    Some(root.join(candidate))
}

fn write(store: &Store, path: &Path, entry: &Entry) -> bool {
    let Ok(bytes) = store.get(&entry.hash) else {
        return false;
    };
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    if std::fs::write(path, &bytes).is_err() {
        return false;
    }
    set_mode(path, entry.mode);
    true
}

fn remove(root: &Path, path: &Path) -> bool {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        // Already gone: the rollback wanted it gone.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return false,
    }
    if let Some(parent) = path.parent() {
        prune_empty(root, parent);
    }
    true
}

/// Removes directories the rollback has just emptied.
///
/// A snapshot holds files, not directories, so without this a subtree the agent
/// created survives its own rollback as a skeleton of empty folders.
fn prune_empty(root: &Path, from: &Path) {
    let mut dir = from;
    while dir != root && dir.starts_with(root) {
        // Fails when the directory still holds something, which is the signal
        // to stop climbing.
        if std::fs::remove_dir(dir).is_err() {
            return;
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return,
        }
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: Mode) {
    use std::os::unix::fs::PermissionsExt;
    let bits = match mode {
        Mode::Exec => 0o755,
        Mode::File => 0o644,
    };
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(bits));
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: Mode) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::scan::Scanner;
    use crate::testutil::TempDir;

    struct Fixture {
        dir: TempDir,
        store: Store,
        scanner: Scanner,
    }

    fn fixture() -> Fixture {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let scanner = Scanner::new(dir.path());
        Fixture {
            dir,
            store,
            scanner,
        }
    }

    impl Fixture {
        fn scan(&mut self) -> Manifest {
            // Coarse clocks: two writes inside one tick would otherwise look
            // like no write at all.
            std::thread::sleep(std::time::Duration::from_millis(15));
            self.scanner.scan(&self.store).unwrap()
        }

        fn roll_back_to(&mut self, snapshot: &Manifest) -> RestoreReport {
            let now = self.scan();
            let plan = plan(&self.store, snapshot, &now);
            apply(&self.store, self.dir.path(), snapshot, &plan).unwrap()
        }
    }

    #[test]
    fn an_edited_file_comes_back() {
        let mut f = fixture();
        let path = f.dir.write("a.rs", "before\n");
        let snapshot = f.scan();

        std::fs::write(&path, "after\n").unwrap();
        let report = f.roll_back_to(&snapshot);

        assert_eq!(report.touched, vec!["a.rs"]);
        assert!(report.failed.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "before\n");
    }

    #[test]
    fn a_deleted_file_comes_back() {
        let mut f = fixture();
        let path = f.dir.write("src/a.rs", "body\n");
        let snapshot = f.scan();

        std::fs::remove_file(&path).unwrap();
        f.roll_back_to(&snapshot);

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "body\n");
    }

    #[test]
    fn a_file_created_since_the_snapshot_goes_away_with_its_directory() {
        let mut f = fixture();
        f.dir.write("keep.rs", "x\n");
        let snapshot = f.scan();

        let added = f.dir.write("src/deep/new.rs", "x\n");
        f.roll_back_to(&snapshot);

        assert!(!added.exists());
        // Otherwise the rollback leaves the skeleton of a subtree it undid.
        assert!(!f.dir.path().join("src").exists());
        assert!(f.dir.path().join("keep.rs").exists());
    }

    #[test]
    fn a_git_ignored_file_is_restored_like_any_other() {
        let mut f = fixture();
        f.dir.write(".gitignore", ".env\n");
        let env = f.dir.write(".env", "SECRET=1\n");
        let snapshot = f.scan();

        std::fs::write(&env, "SECRET=2\n").unwrap();
        f.roll_back_to(&snapshot);

        // §5.8 names this case outright.
        assert_eq!(std::fs::read_to_string(&env).unwrap(), "SECRET=1\n");
    }

    #[test]
    fn untracked_output_is_not_touched() {
        let mut f = fixture();
        f.dir.write("a.rs", "x\n");
        let snapshot = f.scan();

        let built = f.dir.write("target/debug/app.bin", "binary\n");
        let report = f.roll_back_to(&snapshot);

        assert!(report.touched.is_empty());
        assert!(
            built.exists(),
            "a rollback must not delete what it never saved"
        );
    }

    #[test]
    fn the_plan_says_what_the_rollback_will_do() {
        let mut f = fixture();
        f.dir.write("edited.rs", "one\ntwo\n");
        f.dir.write("removed.rs", "x\n");
        let snapshot = f.scan();

        std::fs::write(f.dir.path().join("edited.rs"), "one\n").unwrap();
        std::fs::remove_file(f.dir.path().join("removed.rs")).unwrap();
        f.dir.write("added.rs", "x\n");

        let now = f.scan();
        let plan = plan(&f.store, &snapshot, &now);

        let kinds: Vec<_> = plan.iter().map(|c| (c.path.as_str(), c.kind)).collect();
        assert_eq!(
            kinds,
            vec![
                ("added.rs", ChangeKind::Deleted),
                ("edited.rs", ChangeKind::Modified),
                ("removed.rs", ChangeKind::Created),
            ]
        );
        // Counted from the disk towards the snapshot: the rollback puts the
        // second line back.
        let edited = plan.iter().find(|c| c.path == "edited.rs").unwrap();
        assert_eq!((edited.added, edited.removed), (1, 0));
    }

    #[test]
    fn an_unchanged_tree_plans_nothing() {
        let mut f = fixture();
        f.dir.write("a.rs", "x\n");
        let snapshot = f.scan();
        let now = f.scan();
        assert!(plan(&f.store, &snapshot, &now).is_empty());
    }

    #[test]
    fn a_path_leaving_the_project_is_refused() {
        assert!(inside(Path::new("/p"), "../outside.rs").is_none());
        assert!(inside(Path::new("/p"), "src/a.rs").is_some());
    }
}
