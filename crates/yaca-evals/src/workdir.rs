//! Per-run working trees.
//!
//! Every run gets a fresh copy of `repo/`, so one run cannot see what another
//! changed, and the fixture on disk is never touched. Names are made unique by
//! `create_dir` failing on a clash, not by the clock: on Windows two calls in
//! the same tick read the same timestamp, and the second run would silently
//! reuse the first one's tree.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// A fresh, empty directory under `parent`, named `<label>-<stamp>-<seq>`.
/// Fails only if the filesystem fails; a name taken by someone else is retried.
pub fn create_unique(parent: &Path, label: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(parent)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    loop {
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!("{label}-{nanos:x}-{:x}-{seq:x}", std::process::id()));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

/// A run's tree. Removed on drop unless the run asked to keep it, which is the
/// way to look at what the agent left behind.
pub struct WorkDir {
    path: PathBuf,
    keep: bool,
}

impl WorkDir {
    pub fn new(path: PathBuf, keep: bool) -> Self {
        WorkDir { path, keep }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        if !self.keep {
            // Best effort: a tree the agent left a process inside of may refuse
            // removal on Windows, and a failed cleanup must not fail the run.
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Copies `src` into `dst` recursively. Symlinks are refused: a link in a
/// fixture would point at the machine it was made on, and the copy would
/// quietly follow it out of the tree.
pub fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("fixture contains a symlink: {}", from.display()),
            ));
        } else if kind.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> PathBuf {
        create_unique(&std::env::temp_dir().join("yaca-evals-tests"), label).unwrap()
    }

    #[test]
    fn names_do_not_repeat_even_in_a_tight_loop() {
        let parent = scratch("uniq-parent");
        let names: std::collections::HashSet<PathBuf> =
            (0..200).map(|_| create_unique(&parent, "run").unwrap()).collect();
        assert_eq!(names.len(), 200);
    }

    #[test]
    fn copy_is_deep_and_leaves_the_source_alone() {
        let src = scratch("copy-src");
        fs::create_dir_all(src.join("src/nested")).unwrap();
        fs::write(src.join("src/nested/a.rs"), "fn a() {}").unwrap();
        fs::write(src.join("top.txt"), "t").unwrap();
        let dst = scratch("copy-dst").join("tree");

        copy_tree(&src, &dst).unwrap();
        assert_eq!(fs::read_to_string(dst.join("src/nested/a.rs")).unwrap(), "fn a() {}");
        assert_eq!(fs::read_to_string(dst.join("top.txt")).unwrap(), "t");

        fs::write(dst.join("top.txt"), "changed").unwrap();
        assert_eq!(fs::read_to_string(src.join("top.txt")).unwrap(), "t");
    }

    #[test]
    fn dropping_a_workdir_removes_it_unless_kept() {
        let gone = scratch("drop-gone").join("t");
        fs::create_dir_all(&gone).unwrap();
        drop(WorkDir::new(gone.clone(), false));
        assert!(!gone.exists());

        let kept = scratch("drop-kept").join("t");
        fs::create_dir_all(&kept).unwrap();
        drop(WorkDir::new(kept.clone(), true));
        assert!(kept.exists());
    }
}
