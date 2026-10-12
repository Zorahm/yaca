//! Content-addressed storage for the shadow repository (§5.8).
//!
//! An object is named by the hash of what is inside it, so a content already
//! stored is never stored twice. That is the whole reason a snapshot before
//! every permitted command is affordable: a hundred checkpoints of a repository
//! where three files moved cost three files.
//!
//! Nothing here is ever overwritten. Equal name means equal content, which is
//! what makes the store safe to share between checkpoints — and what makes a
//! concurrent writer harmless.
//!
//! Two kinds of file live in the store, in two directories: file content in
//! `objects`, and manifests in `manifests`. Keeping them apart is what lets the
//! sweep ([`super::gc`]) tell which names are roots and which are leaves.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use blake3::Hash;

/// Where the shadow repository lives inside the project.
///
/// Its own directory under `.yaca`, never the project's `.git`: the snapshot
/// has to cover git-ignored files — the spec is explicit that `.env` must come
/// back — so it cannot be a guest in a repository whose rules exclude them.
pub const SHADOW_DIR: &str = "shadow";

/// An object reused by a snapshot is refreshed once its mtime is older than
/// this. Reuse on every scan would cost a write per file per command; refreshing
/// only once it is this old keeps the mtime inside the sweep's grace window
/// without a write each time. Must stay well below [`super::gc::GRACE`].
pub const REFRESH_AFTER: Duration = Duration::from_secs(10 * 60);

pub struct Store {
    shadow: PathBuf,
    objects: PathBuf,
    manifests: PathBuf,
    tmp: PathBuf,
}

impl Store {
    pub fn open(root: &Path) -> io::Result<Self> {
        let shadow = crate::project::dir(root).join(SHADOW_DIR);
        let objects = shadow.join("objects");
        let manifests = shadow.join("manifests");
        let tmp = shadow.join("tmp");
        std::fs::create_dir_all(&objects)?;
        std::fs::create_dir_all(&manifests)?;
        std::fs::create_dir_all(&tmp)?;
        Ok(Store {
            shadow,
            objects,
            manifests,
            tmp,
        })
    }

    /// The shadow directory itself. Used as the key for in-process bookkeeping,
    /// since every session of one project opens the same one.
    pub fn dir(&self) -> &Path {
        &self.shadow
    }

    pub(super) fn objects_dir(&self) -> &Path {
        &self.objects
    }

    pub(super) fn manifests_dir(&self) -> &Path {
        &self.manifests
    }

    pub(super) fn tmp_dir(&self) -> &Path {
        &self.tmp
    }

    pub fn hash(bytes: &[u8]) -> Hash {
        blake3::hash(bytes)
    }

    /// Two hex characters of the name become a directory. Flat would work
    /// until a directory holds a hundred thousand entries, which is where
    /// every filesystem starts to complain.
    pub fn path_of(&self, hash: &Hash) -> PathBuf {
        path_in(&self.objects, hash)
    }

    pub fn has(&self, hash: &Hash) -> bool {
        self.path_of(hash).exists()
    }

    /// Whether the object is still stored, refreshed so that a sweep running
    /// now does not take it. A snapshot that reuses a cached hash calls this
    /// before trusting the hash: the sweep may have removed the object since
    /// the cache was filled.
    pub fn keep_alive(&self, hash: &Hash) -> bool {
        refresh_if_stored(&self.path_of(hash))
    }

    /// Stores the bytes and returns their name. A no-op when the content is
    /// already there.
    pub fn put(&self, bytes: &[u8]) -> io::Result<Hash> {
        self.put_in(&self.objects, bytes)
    }

    /// Stores a manifest and returns its name, which is what a checkpoint
    /// holds on to.
    pub fn put_manifest(&self, text: &str) -> io::Result<Hash> {
        self.put_in(&self.manifests, text.as_bytes())
    }

    fn put_in(&self, dir: &Path, bytes: &[u8]) -> io::Result<Hash> {
        let hash = Self::hash(bytes);
        let target = path_in(dir, &hash);
        // Reuse counts as a use: an object that is already stored must not
        // lose to a sweep that is about to delete it as unreferenced.
        if refresh_if_stored(&target) {
            return Ok(hash);
        }

        // Written aside and renamed: a reader must never find a half-written
        // file under a name that promises the whole content.
        let temp = self.tmp.join(uuid::Uuid::new_v4().simple().to_string());
        std::fs::write(&temp, bytes)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::rename(&temp, &target) {
            Ok(()) => Ok(hash),
            // Someone stored the same content first. Same name, same bytes —
            // nothing was lost.
            Err(_) if target.exists() => {
                let _ = std::fs::remove_file(&temp);
                Ok(hash)
            }
            Err(e) => {
                let _ = std::fs::remove_file(&temp);
                Err(e)
            }
        }
    }

    pub fn get(&self, hash: &Hash) -> io::Result<Vec<u8>> {
        std::fs::read(self.path_of(hash))
    }

    /// The same as [`Store::get`], for objects that are known to be text.
    pub fn get_text(&self, hash: &Hash) -> io::Result<String> {
        let bytes = self.get(hash)?;
        String::from_utf8(bytes).map_err(|_| io::Error::other("object is not utf-8"))
    }

    /// A manifest as it was stored by [`Store::put_manifest`].
    pub fn get_manifest(&self, hash: &Hash) -> io::Result<String> {
        let bytes = std::fs::read(path_in(&self.manifests, hash))?;
        String::from_utf8(bytes).map_err(|_| io::Error::other("manifest is not utf-8"))
    }
}

fn path_in(dir: &Path, hash: &Hash) -> PathBuf {
    let hex = hash.to_hex();
    dir.join(&hex[..2]).join(&hex[2..])
}

/// True when the file exists and is safe from the sweep for another grace
/// window. Only touches the mtime when it is older than [`REFRESH_AFTER`].
fn refresh_if_stored(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let recent = meta
        .modified()
        .ok()
        .and_then(|m| SystemTime::now().duration_since(m).ok())
        .is_some_and(|age| age < REFRESH_AFTER);
    if recent {
        return true;
    }
    // Opened for writing only to move the mtime; a failure means the file
    // cannot be vouched for, and the caller writes it again.
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|file| file.set_modified(SystemTime::now()))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn store() -> (TempDir, Store) {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        (dir, store)
    }

    #[test]
    fn stored_bytes_come_back_unchanged() {
        let (_d, store) = store();
        let hash = store.put(b"\x00\x01binary\xff").unwrap();
        assert_eq!(store.get(&hash).unwrap(), b"\x00\x01binary\xff");
    }

    #[test]
    fn equal_content_is_stored_once() {
        let (_d, store) = store();
        let first = store.put(b"same").unwrap();
        let second = store.put(b"same").unwrap();
        assert_eq!(first, second);
        assert!(store.has(&first));
        // The affordable snapshot rests on exactly this.
        assert_ne!(first, store.put(b"other").unwrap());
    }

    #[test]
    fn nothing_is_left_in_the_temporary_directory() {
        let (dir, store) = store();
        store.put(b"one").unwrap();
        store.put(b"two").unwrap();
        let tmp = dir.path().join(".yaca").join(SHADOW_DIR).join("tmp");
        assert_eq!(std::fs::read_dir(tmp).unwrap().count(), 0);
    }

    #[test]
    fn a_manifest_is_kept_apart_from_file_content() {
        let (_d, store) = store();
        let name = store.put_manifest("abc 644 3 a.rs\n").unwrap();
        assert_eq!(store.get_manifest(&name).unwrap(), "abc 644 3 a.rs\n");
        assert!(!store.has(&name), "a manifest is not an object");
    }

    #[test]
    fn reusing_an_old_object_refreshes_its_mtime() {
        let (_d, store) = store();
        let hash = store.put(b"reused").unwrap();
        let path = store.path_of(&hash);
        let old = SystemTime::now() - (REFRESH_AFTER + Duration::from_secs(60));
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(old)
            .unwrap();

        assert_eq!(store.put(b"reused").unwrap(), hash);
        let age = std::fs::metadata(&path)
            .unwrap()
            .modified()
            .unwrap()
            .elapsed()
            .unwrap();
        assert!(age < REFRESH_AFTER, "a reused object must count as fresh");
    }
}
