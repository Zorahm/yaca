//! Walking the working tree for a snapshot (§5.8).
//!
//! Two rules shape this walk.
//!
//! The project's `.gitignore` is deliberately not consulted. §5.8 requires the
//! snapshot to cover git-ignored files — `.env` must come back — and a user's
//! ignore list is precisely the list of things it would fail to bring back.
//! What is skipped instead is Yaca's own list of build output: without it a
//! `target/` of ten gigabytes would be read before every command.
//!
//! The second rule is the stat cache. A snapshot before each permitted command
//! must not re-read the repository; a file whose size and mtime have not moved
//! keeps the hash it already had, and only the rest is read and stored.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use blake3::Hash;
use ignore::{DirEntry, WalkBuilder};

use super::manifest::{Entry, Manifest, Mode};
use super::store::Store;

/// Directories the shadow repository does not enter.
///
/// Build output and other people's object stores. Not a taste judgement: these
/// are the directories that are regenerated from what *is* tracked, and reading
/// them before every command would cost more than the whole feature saves.
///
/// §10 lets a project add to this list through `.yaca/settings.json`; it cannot
/// take from it. A project that dropped `.yaca` would be snapshotting its own
/// snapshots, and one that dropped `.git` would keep a second copy of every
/// file it has.
pub(crate) const EXCLUDED_DIRS: &[&str] = &[
    // Yaca's own data, the shadow store included: snapshotting the snapshots
    // is the one certain way to make the store grow without bound. Under the
    // old name too: a folder whose move failed is used where it is
    // (`legacy::adopt_dir`), shadow store and all.
    crate::project::DIR,
    crate::legacy::PROJECT_DIR,
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".parcel-cache",
    ".gradle",
];

/// The built-in list, for anyone who has to show it.
///
/// The settings window (§10) draws these beside the project's own additions:
/// a human who cannot see that `target` is already excluded adds it again, and
/// a second copy of the list in the window would be a second thing to keep in
/// step with this one.
pub fn excluded_dirs() -> &'static [&'static str] {
    EXCLUDED_DIRS
}

/// Files above this are not tracked, and therefore never deleted by a rollback:
/// a rollback that removed what it had not saved would be worse than no
/// rollback at all.
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// Beyond this the walk gives up rather than truncating. A partial manifest is
/// not a smaller snapshot — restoring from one deletes every file it left out.
pub const MAX_TRACKED_FILES: usize = 100_000;

/// A file whose mtime is younger than this when it was first seen is re-read
/// on the next scan instead of trusted.
///
/// Filesystem clocks are coarse — on Windows coarse enough that two writes land
/// on the same tick. A file written twice within one tick, to the same length,
/// would otherwise keep the hash of the first write forever. Git calls this
/// state racily clean and handles it the same way.
const RACE_WINDOW_MS: u64 = 2_000;

#[derive(Debug, Clone, Copy)]
struct Cached {
    mtime_ms: u64,
    len: u64,
    hash: Hash,
    /// When the entry was recorded. Together with `mtime_ms` this is what
    /// decides whether the cached hash can be trusted at all.
    seen_at_ms: u64,
}

impl Cached {
    fn trustworthy(&self, mtime_ms: u64, len: u64) -> bool {
        self.mtime_ms == mtime_ms
            && self.len == len
            && self.mtime_ms + RACE_WINDOW_MS <= self.seen_at_ms
    }
}

/// Walks the tree and keeps what it learned for the next walk.
pub struct Scanner {
    root: PathBuf,
    cache: HashMap<String, Cached>,
    /// What the project added to the built-in list (§10).
    extra: Vec<String>,
}

impl Scanner {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Scanner::with_excludes(root, &[])
    }

    pub fn with_excludes(root: impl Into<PathBuf>, extra: &[String]) -> Self {
        Scanner {
            root: root.into(),
            cache: HashMap::new(),
            extra: extra
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }

    /// The tree as it is now. Everything read on the way lands in the store, so
    /// the manifest is restorable the moment it is returned.
    pub fn scan(&mut self, store: &Store) -> io::Result<Manifest> {
        let now = crate::id::now_ms();
        let mut entries = Vec::new();
        let mut fresh: HashMap<String, Cached> = HashMap::new();

        // Cloned into the filter: `ignore` wants it `'static`, and the list is
        // a handful of directory names.
        let extra = self.extra.clone();
        let walk = WalkBuilder::new(&self.root)
            // `.env` belongs in the snapshot, so hidden files are not hidden.
            .hidden(false)
            .parents(false)
            .ignore(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .follow_links(false)
            .filter_entry(move |e| !excluded(e, &extra))
            .build();

        for found in walk {
            let entry = match found {
                Ok(e) => e,
                // A directory that vanished mid-walk, or one we may not read.
                // Neither is a reason to lose the snapshot.
                Err(_) => continue,
            };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let Some(path) = self.relative(entry.path()) else {
                continue;
            };
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let len = meta.len();
            if len > MAX_FILE_BYTES {
                continue;
            }
            if entries.len() >= MAX_TRACKED_FILES {
                // Almost always a folder that is not a project: a home directory
                // or a drive. Say which one, so the person can see it is the wrong one.
                return Err(io::Error::other(format!(
                    "{} holds more than {MAX_TRACKED_FILES} tracked files, too many to snapshot; \
                     if this is not one project, open a smaller folder",
                    self.root.display()
                )));
            }

            let mtime_ms = mtime_of(&meta);
            let mode = mode_of(&meta);

            // A cached hash names an object that the sweep may have taken since
            // the cache was filled. Reuse refreshes it, or the file is read again.
            let (hash, seen_at_ms) = match self.cache.get(&path) {
                Some(c) if c.trustworthy(mtime_ms, len) && store.keep_alive(&c.hash) => {
                    (c.hash, c.seen_at_ms)
                }
                _ => {
                    let bytes = match std::fs::read(entry.path()) {
                        Ok(b) => b,
                        // Read after stat: the file could be gone by now.
                        Err(_) => continue,
                    };
                    (store.put(&bytes)?, now)
                }
            };

            fresh.insert(
                path.clone(),
                Cached {
                    mtime_ms,
                    len,
                    hash,
                    seen_at_ms,
                },
            );
            entries.push(Entry {
                path,
                hash,
                mode,
                len,
            });
        }

        // Replaced, not merged: a file that is gone must not keep a slot in the
        // cache, or the cache grows for as long as the session lives.
        self.cache = fresh;
        Ok(Manifest::new(entries))
    }

    /// The path as the manifest spells it, or `None` when it cannot be spelled.
    ///
    /// A name that is not UTF-8, or one holding a newline, has no line in the
    /// format. Such a file is simply not tracked — and because a rollback only
    /// touches tracked files, not tracking it is also what keeps it safe.
    fn relative(&self, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(&self.root).ok()?.to_str()?;
        if rel.is_empty() || rel.contains(['\n', '\r']) {
            return None;
        }
        Some(rel.replace('\\', "/"))
    }
}

fn excluded(entry: &DirEntry, extra: &[String]) -> bool {
    // The root is where the walk starts; a project that happens to be called
    // `build` is still the project.
    if entry.depth() == 0 {
        return false;
    }
    if !entry.file_type().is_some_and(|t| t.is_dir()) {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| EXCLUDED_DIRS.contains(&name) || extra.iter().any(|e| e == name))
}

fn mtime_of(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(unix)]
fn mode_of(meta: &std::fs::Metadata) -> Mode {
    use std::os::unix::fs::PermissionsExt;
    if meta.permissions().mode() & 0o111 != 0 {
        Mode::Exec
    } else {
        Mode::File
    }
}

#[cfg(not(unix))]
fn mode_of(_meta: &std::fs::Metadata) -> Mode {
    Mode::File
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn setup() -> (TempDir, Store, Scanner) {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let scanner = Scanner::new(dir.path());
        (dir, store, scanner)
    }

    fn paths(m: &Manifest) -> Vec<&str> {
        m.entries.iter().map(|e| e.path.as_str()).collect()
    }

    #[test]
    fn git_ignored_files_are_in_the_snapshot() {
        let (dir, store, mut scanner) = setup();
        dir.write(".gitignore", ".env\n");
        dir.write(".env", "SECRET=1\n");
        dir.write("src/a.rs", "fn main() {}\n");

        // §5.8 is explicit: `.env` must come back, so the project's own
        // ignore list has no say here.
        let m = scanner.scan(&store).unwrap();
        assert!(paths(&m).contains(&".env"), "got {:?}", paths(&m));
    }

    #[test]
    fn build_output_is_left_alone() {
        let (dir, store, mut scanner) = setup();
        dir.write("src/a.rs", "x\n");
        dir.write("target/debug/huge.bin", "x\n");
        dir.write("node_modules/left-pad/index.js", "x\n");

        assert_eq!(paths(&scanner.scan(&store).unwrap()), vec!["src/a.rs"]);
    }

    #[test]
    fn the_shadow_store_does_not_snapshot_itself() {
        let (dir, store, mut scanner) = setup();
        dir.write("a.rs", "x\n");
        scanner.scan(&store).unwrap();

        // The first scan filled `.yaca/shadow`; the second must not see it.
        let m = scanner.scan(&store).unwrap();
        assert_eq!(paths(&m), vec!["a.rs"]);
    }

    #[test]
    fn an_unchanged_file_is_not_read_again() {
        let (dir, store, mut scanner) = setup();
        let path = dir.write("a.rs", "one\n");

        // The file has to age out of the race window before the cache is
        // allowed to speak for it at all — that is the point of the window.
        std::thread::sleep(std::time::Duration::from_millis(RACE_WINDOW_MS + 100));
        let first = scanner.scan(&store).unwrap();

        // Now the content changes behind the scanner's back, with mtime and
        // length put back exactly as they were. The proof that the cache
        // answered instead of the disk is that the hash did not move.
        let times = std::fs::FileTimes::new()
            .set_modified(std::fs::metadata(&path).unwrap().modified().unwrap());
        std::fs::write(&path, "two\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(times)
            .unwrap();

        assert_eq!(scanner.scan(&store).unwrap(), first);
    }

    #[test]
    fn a_file_written_this_instant_is_not_trusted_to_the_cache() {
        let (dir, store, mut scanner) = setup();
        let path = dir.write("a.rs", "one\n");
        let first = scanner.scan(&store).unwrap();

        // Same length, and on a coarse clock quite possibly the same mtime.
        // Inside the race window the scanner has to read the file anyway.
        let times = std::fs::FileTimes::new()
            .set_modified(std::fs::metadata(&path).unwrap().modified().unwrap());
        std::fs::write(&path, "two\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(times)
            .unwrap();

        assert_ne!(scanner.scan(&store).unwrap(), first);
    }

    #[test]
    fn a_changed_file_gets_a_new_hash_and_a_new_object() {
        let (dir, store, mut scanner) = setup();
        dir.write("a.rs", "one\n");
        let first = scanner.scan(&store).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(20));
        dir.write("a.rs", "two\n");
        let second = scanner.scan(&store).unwrap();

        assert_ne!(first.entries[0].hash, second.entries[0].hash);
        // Both versions are in the store, which is what makes a rollback
        // between them possible at all.
        assert!(store.has(&first.entries[0].hash));
        assert!(store.has(&second.entries[0].hash));
    }

    #[test]
    fn a_project_can_add_to_the_list_but_not_take_from_it() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        dir.write(
            "src/a.rs", "x
",
        );
        dir.write(
            "fixtures/huge.bin",
            "x
",
        );

        // §10: "exclusions of the shadow repository".
        let mut scanner =
            Scanner::with_excludes(dir.path(), &["fixtures".to_string(), ".yaca".to_string()]);
        assert_eq!(paths(&scanner.scan(&store).unwrap()), vec!["src/a.rs"]);

        // And naming `.yaca` changes nothing: it was never negotiable.
        dir.write(
            "keep.rs", "x
",
        );
        let m = scanner.scan(&store).unwrap();
        assert!(paths(&m).contains(&"keep.rs"));
        assert!(!paths(&m).iter().any(|p| p.starts_with(".yaca")));
    }

    #[test]
    fn a_file_above_the_size_cap_is_not_tracked() {
        let (dir, store, mut scanner) = setup();
        dir.write("small.rs", "x\n");
        dir.write("big.bin", &"x".repeat(MAX_FILE_BYTES as usize + 1));
        assert_eq!(paths(&scanner.scan(&store).unwrap()), vec!["small.rs"]);
    }

    #[test]
    fn a_deleted_file_leaves_the_cache() {
        let (dir, store, mut scanner) = setup();
        let path = dir.write("a.rs", "x\n");
        scanner.scan(&store).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert!(scanner.scan(&store).unwrap().entries.is_empty());
        assert!(
            scanner.cache.is_empty(),
            "the cache must not outlive the file"
        );
    }
}
