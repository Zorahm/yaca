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

use std::io;
use std::path::{Path, PathBuf};

use blake3::Hash;

/// Where the shadow repository lives inside the project.
///
/// Its own directory under `.yaca`, never the project's `.git`: the snapshot
/// has to cover git-ignored files — the spec is explicit that `.env` must come
/// back — so it cannot be a guest in a repository whose rules exclude them.
pub const SHADOW_DIR: &str = "shadow";

pub struct Store {
    objects: PathBuf,
    tmp: PathBuf,
}

impl Store {
    pub fn open(root: &Path) -> io::Result<Self> {
        let shadow = crate::project::dir(root).join(SHADOW_DIR);
        let objects = shadow.join("objects");
        let tmp = shadow.join("tmp");
        std::fs::create_dir_all(&objects)?;
        std::fs::create_dir_all(&tmp)?;
        Ok(Store { objects, tmp })
    }

    pub fn hash(bytes: &[u8]) -> Hash {
        blake3::hash(bytes)
    }

    /// Two hex characters of the name become a directory. Flat would work
    /// until a directory holds a hundred thousand entries, which is where
    /// every filesystem starts to complain.
    pub fn path_of(&self, hash: &Hash) -> PathBuf {
        let hex = hash.to_hex();
        self.objects.join(&hex[..2]).join(&hex[2..])
    }

    pub fn has(&self, hash: &Hash) -> bool {
        self.path_of(hash).exists()
    }

    /// Stores the bytes and returns their name. A no-op when the content is
    /// already there.
    pub fn put(&self, bytes: &[u8]) -> io::Result<Hash> {
        let hash = Self::hash(bytes);
        let target = self.path_of(&hash);
        if target.exists() {
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
}
