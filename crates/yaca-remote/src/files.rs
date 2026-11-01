//! How the remote directory's files are read and written: JSON that falls back
//! to a default, writes that never leave half a file, and the one unit of time
//! every stored timestamp uses.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::Error;

/// Reads a JSON file, or the type's default when it is not usable. A missing
/// file is the normal first run and says nothing; a file that exists but does
/// not parse is logged, because someone edited it or the disk is failing, and
/// the human should be able to find out why the setting did not take.
///
/// Shared with the devices list, which has the same first-run and corruption
/// cases and the same answer to them.
pub(crate) fn load_json_or_default<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            tracing::warn!(path = %path.display(), error = %e, "remote file unreadable, using defaults");
            T::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => T::default(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "remote file unreadable, using defaults");
            T::default()
        }
    }
}

/// Writes `value` as pretty JSON, atomically (see [`write_atomic`]).
pub(crate) fn save_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), Error> {
    write_atomic(path, &serde_json::to_vec_pretty(value)?)
}

/// Replaces `path` with `bytes` so a reader never sees half a file: the bytes go
/// to a sibling temporary file, which is then renamed over the target. On unix
/// the file is created 0600, because the remote directory holds a private key
/// and token hashes, and the permission must be there from the first byte
/// rather than tightened after a world-readable window.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    use std::io::Write;

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4().simple()));

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> std::io::Result<()> {
        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        // Best effort: the rename may have failed after the temporary file was
        // written, and it holds the same secrets as the target.
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(Error::from)
}

/// A directory under the system temp dir, unique per call and removed when it
/// goes out of scope, so a failing test leaves nothing behind. Shared by the
/// unit tests of every module here: each one touches the real filesystem.
#[cfg(test)]
pub(crate) struct TestDir(std::path::PathBuf);

#[cfg(test)]
impl TestDir {
    pub(crate) fn new() -> Self {
        Self(std::env::temp_dir().join(format!("yaca-remote-test-{}", uuid::Uuid::new_v4().simple())))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Milliseconds since the Unix epoch, the unit every stored time uses. A clock
/// before 1970 reads as 0 rather than failing a pairing over it.
pub(crate) fn epoch_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

