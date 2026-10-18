//! Garbage collection of the shadow store (§5.8).
//!
//! The store only grows: every snapshot adds a manifest and whatever file
//! content it had not seen before, and nothing removed either. This module is
//! a mark and sweep over it.
//!
//! Roots are the manifests that are kept, and a manifest is kept when it was
//! recorded by this process, or is named by the saved list of any session of
//! the project ([`super::ledger`]), or is younger than [`RETENTION`]. The
//! saved lists are what keep a chat's points for as long as the chat exists,
//! whatever their age and whichever process wrote them; the age clause is what
//! protects points nothing names — a run whose list is not saved, in another
//! process, whose points are in its memory and nowhere this sweep can see.
//! Everything a kept manifest names is live. Anything else is swept once it is
//! older than [`GRACE`].
//!
//! The grace window is for writers that are not yet visible to the mark. A
//! snapshot stores its objects before it writes the manifest that names them,
//! and another process may be between those two steps. Such an object is
//! fresh, so it survives. Reuse refreshes the mtime (see
//! [`super::store::REFRESH_AFTER`]), so an object a snapshot is about to name
//! does not look old either.
//!
//! Failures on single files are skipped, never raised, and the counts in the
//! [`Report`] say what was done: a session must not fail because a sweep could
//! not delete something. The one case
//! that stops the object sweep is a kept manifest that cannot be read, because
//! its objects are then unknown.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime};

use blake3::Hash;

use super::manifest::Manifest;
use super::store::{Store, REFRESH_AFTER};

/// How long a manifest outlives the process that wrote it, when no saved list
/// names it. Two weeks is longer than a chat stays open in practice, and a
/// manifest is a few kilobytes, so the cost of keeping it is negligible next to
/// deleting a point another window can still roll back to.
pub const RETENTION: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// How long an unreferenced object must sit untouched before it is removed.
/// An hour is far longer than any single snapshot takes, and the window only
/// has to cover a writer between storing an object and storing its manifest.
pub const GRACE: Duration = Duration::from_secs(60 * 60);

// A reused object must come out of its refresh younger than the grace window,
// or a sweep could take it between the refresh and the manifest that names it.
const _: () = assert!(REFRESH_AFTER.as_secs() < GRACE.as_secs());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub retention: Duration,
    pub grace: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            retention: RETENTION,
            grace: GRACE,
        }
    }
}

/// What one sweep removed. Tests assert on it; the session only logs it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub manifests_removed: usize,
    pub objects_removed: usize,
    pub bytes_freed: u64,
    /// Leftovers of writes that never reached their rename.
    pub temporary_removed: usize,
    /// Set when a kept manifest could not be read, so the objects it names are
    /// unknown and nothing was swept at all.
    pub sweep_skipped: bool,
}

/// Manifests recorded by this process, per shadow directory. Filled by every
/// [`super::Checkpoints`] record, so it holds what a live session still points
/// at, whichever session it belongs to.
static RECORDED: OnceLock<Mutex<HashMap<PathBuf, HashSet<Hash>>>> = OnceLock::new();

fn recorded() -> MutexGuard<'static, HashMap<PathBuf, HashSet<Hash>>> {
    // A poisoned map is still a set of names; the only way to lose one is a
    // panic in the middle of an insert, which leaves it usable.
    RECORDED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Notes that a manifest is in use by this process.
pub(super) fn remember(store: &Store, manifest: Hash) {
    recorded()
        .entry(store.dir().to_path_buf())
        .or_default()
        .insert(manifest);
}

/// Runs one sweep of `store` as a session sees it: the manifests recorded in
/// this process are live, the clock is the real one.
pub fn run(store: &Store) -> Report {
    run_keeping(store, &HashSet::new())
}

/// [`run`], with `kept` live on top of what this process recorded: the
/// manifests that the saved lists of the project's sessions name (see
/// [`super::ledger`]), which no age may take.
pub fn run_keeping(store: &Store, kept: &HashSet<Hash>) -> Report {
    let mut live = recorded().get(store.dir()).cloned().unwrap_or_default();
    live.extend(kept.iter().copied());
    let report = collect(store, &live, SystemTime::now(), Policy::default());
    tracing::debug!(
        manifests = report.manifests_removed,
        objects = report.objects_removed,
        bytes = report.bytes_freed,
        "shadow store swept"
    );
    report
}

/// The sweep with its inputs made explicit: `live` names the manifests this
/// process still holds, and `now` is the clock the thresholds are measured
/// against.
pub fn collect(store: &Store, live: &HashSet<Hash>, now: SystemTime, policy: Policy) -> Report {
    let mut report = Report::default();
    let mut reachable: HashSet<Hash> = HashSet::new();
    let mut sweep_ok = true;

    for (name, path) in named_files(store.manifests_dir()) {
        // A manifest whose age cannot be read is kept: deleting on a guess is
        // the one mistake this module must not make.
        let expired = older_than(&path, now, policy.retention).unwrap_or(false);
        if !live.contains(&name) && expired {
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    report.manifests_removed += 1;
                    continue;
                }
                // Gone already, which is what removing it was for.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => {}
            }
        }
        match read_manifest(&path) {
            Some(manifest) => reachable.extend(manifest.entries.iter().map(|e| e.hash)),
            None => sweep_ok = false,
        }
    }
    if !sweep_ok {
        report.sweep_skipped = true;
        return report;
    }

    for (name, path) in named_files(store.objects_dir()) {
        if reachable.contains(&name) {
            continue;
        }
        if older_than(&path, now, policy.grace) != Some(true) {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if std::fs::remove_file(&path).is_ok() {
            report.objects_removed += 1;
            report.bytes_freed += meta.len();
        }
    }

    sweep_temporary(store.tmp_dir(), now, policy.grace, &mut report);
    report
}

/// Every file under the two-level fan-out of `dir`, with the hash its path
/// spells. Anything that does not spell a hash is not ours and is left alone.
fn named_files(dir: &Path) -> Vec<(Hash, PathBuf)> {
    let mut found = Vec::new();
    let Ok(fans) = std::fs::read_dir(dir) else {
        return found;
    };
    for fan in fans.flatten() {
        let fan_name = fan.file_name().to_string_lossy().into_owned();
        if fan_name.len() != 2 {
            continue;
        }
        let Ok(files) = std::fs::read_dir(fan.path()) else {
            continue;
        };
        for file in files.flatten() {
            let rest = file.file_name().to_string_lossy().into_owned();
            if let Ok(name) = Hash::from_hex(format!("{fan_name}{rest}")) {
                found.push((name, file.path()));
            }
        }
    }
    found
}

fn read_manifest(path: &Path) -> Option<Manifest> {
    let text = std::fs::read_to_string(path).ok()?;
    Manifest::decode(&text).ok()
}

/// `Some(true)` when the file's mtime is further back than `limit`. `None`
/// when it cannot be read. A future mtime counts as fresh.
fn older_than(path: &Path, now: SystemTime, limit: Duration) -> Option<bool> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(now.duration_since(modified).is_ok_and(|age| age > limit))
}

fn sweep_temporary(dir: &Path, now: SystemTime, grace: Duration, report: &mut Report) {
    let Ok(files) = std::fs::read_dir(dir) else {
        return;
    };
    for file in files.flatten() {
        let path = file.path();
        if older_than(&path, now, grace) == Some(true) && std::fs::remove_file(&path).is_ok() {
            report.temporary_removed += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{CheckpointReason, Checkpoints};
    use crate::testutil::TempDir;

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    /// Sets an mtime `age` before `now`. Writable handle, as the scanner's own
    /// tests do, so the change lands on Windows too.
    fn set_age(path: &Path, now: SystemTime, age: Duration) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(now - age)
            .unwrap();
    }

    /// Every file under the store, manifests and objects alike, aged.
    fn age_everything(store: &Store, now: SystemTime, age: Duration) {
        for dir in [store.manifests_dir(), store.objects_dir()] {
            for (_, path) in named_files(dir) {
                set_age(&path, now, age);
            }
        }
    }

    /// Two points over one file that changes and one that does not:
    /// `a.rs` goes `one` then `two`; `common.rs` is `common` in both.
    fn two_points(
        dir: &TempDir,
    ) -> (
        Checkpoints,
        crate::checkpoint::CheckpointSummary,
        crate::checkpoint::CheckpointSummary,
    ) {
        let ck = Checkpoints::open(dir.path(), &[]).unwrap();
        dir.write("a.rs", "one\n");
        dir.write("common.rs", "common\n");
        let first = ck.create(CheckpointReason::Message).unwrap().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(15));
        dir.write("a.rs", "two\n");
        let second = ck.create(CheckpointReason::Message).unwrap().unwrap();
        (ck, first, second)
    }

    #[test]
    fn a_kept_checkpoint_keeps_its_objects_and_still_restores() {
        let dir = TempDir::new();
        let (ck, _first, second) = two_points(&dir);
        let store = Store::open(dir.path()).unwrap();
        let now = SystemTime::now();
        age_everything(&store, now, 30 * DAY);

        let keep = ck.manifest_name(&second.id).unwrap();
        let report = collect(&store, &HashSet::from([keep]), now, Policy::default());
        assert_eq!(report.manifests_removed, 1);
        assert!(!report.sweep_skipped);

        // The second point is named by `live`, so its content survives.
        std::fs::write(dir.path().join("a.rs"), "three\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(15));
        let restored = ck.restore(&second.id).unwrap().unwrap();
        assert!(restored.failed.is_empty());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "two\n"
        );
    }

    #[test]
    fn an_unreferenced_old_object_goes_and_a_fresh_one_stays() {
        let dir = TempDir::new();
        let (ck, _first, second) = two_points(&dir);
        let store = Store::open(dir.path()).unwrap();
        let now = SystemTime::now();

        let stale = store.put(b"orphan, written long ago").unwrap();
        let fresh = store.put(b"orphan, written just now").unwrap();
        set_age(
            &store.path_of(&stale),
            now,
            Duration::from_secs(2 * 60 * 60),
        );
        set_age(&store.path_of(&fresh), now, Duration::from_secs(10 * 60));
        let keep = ck.manifest_name(&second.id).unwrap();

        let report = collect(&store, &HashSet::from([keep]), now, Policy::default());
        assert!(!store.has(&stale), "an old unreferenced object must go");
        assert!(
            store.has(&fresh),
            "an object inside the grace window must stay"
        );
        assert!(report.objects_removed >= 1);
        assert!(report.bytes_freed >= b"orphan, written long ago".len() as u64);
    }

    #[test]
    fn an_old_manifest_outside_the_list_goes_with_the_objects_only_it_named() {
        let dir = TempDir::new();
        let (ck, first, second) = two_points(&dir);
        let store = Store::open(dir.path()).unwrap();
        let now = SystemTime::now();
        age_everything(&store, now, 30 * DAY);

        let dropped = ck.manifest_name(&first.id).unwrap();
        let keep = ck.manifest_name(&second.id).unwrap();
        let only_first = Store::hash(b"one\n");
        assert!(store.has(&only_first));

        let report = collect(&store, &HashSet::from([keep]), now, Policy::default());
        assert_eq!(report.manifests_removed, 1);
        assert!(
            store.get_manifest(&dropped).is_err(),
            "the dropped manifest is gone"
        );
        assert!(!store.has(&only_first), "its only object goes with it");
    }

    #[test]
    fn objects_shared_by_a_dropped_and_a_kept_manifest_survive() {
        let dir = TempDir::new();
        let (ck, _first, second) = two_points(&dir);
        let store = Store::open(dir.path()).unwrap();
        let now = SystemTime::now();
        age_everything(&store, now, 30 * DAY);

        let keep = ck.manifest_name(&second.id).unwrap();
        collect(&store, &HashSet::from([keep]), now, Policy::default());

        // `common.rs` is named by both points, so the dropped one cannot take it.
        assert!(store.has(&Store::hash(b"common\n")));
        assert!(store.has(&Store::hash(b"two\n")));
    }

    #[test]
    fn a_manifest_younger_than_retention_survives_even_unlisted() {
        let dir = TempDir::new();
        let (_ck, _first, _second) = two_points(&dir);
        let store = Store::open(dir.path()).unwrap();
        let now = SystemTime::now();
        let report = collect(&store, &HashSet::new(), now, Policy::default());
        assert_eq!(report.manifests_removed, 0);
    }

    #[test]
    fn a_reused_object_is_not_taken_by_a_sweep_that_follows() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let hash = store.put(b"reused content").unwrap();
        let now = SystemTime::now();
        set_age(&store.path_of(&hash), now, Duration::from_secs(2 * 60 * 60));

        // Storing it again is a use: it refreshes the mtime, so this sweep,
        // which would otherwise take it, leaves it alone.
        assert_eq!(store.put(b"reused content").unwrap(), hash);
        collect(
            &store,
            &HashSet::new(),
            SystemTime::now(),
            Policy::default(),
        );
        assert!(store.has(&hash));
    }

    #[test]
    fn a_cached_hash_whose_object_was_swept_is_stored_again() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "one\n");
        // Old enough that the scanner trusts its cache at once, without a wait.
        let now = SystemTime::now();
        set_age(&path, now, Duration::from_secs(60 * 60));

        let ck = Checkpoints::open(dir.path(), &[]).unwrap();
        let first = ck.create(CheckpointReason::Message).unwrap().unwrap();
        let store = Store::open(dir.path()).unwrap();
        age_everything(&store, now, 30 * DAY);
        collect(&store, &HashSet::new(), now, Policy::default());
        assert!(!store.has(&Store::hash(b"one\n")));

        // The scanner still caches the hash of `a.rs`, but the object is
        // gone; the next snapshot must read the file again rather than name it.
        assert!(ck.create(CheckpointReason::Message).unwrap().is_none());
        assert!(store.has(&Store::hash(b"one\n")));
        std::fs::write(&path, "two\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(15));
        let restored = ck.restore(&first.id).unwrap().unwrap();
        assert!(restored.failed.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\n");
    }
}
