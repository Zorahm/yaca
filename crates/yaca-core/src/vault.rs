//! Secrets in a file of the configuration directory, for a machine that has no
//! keychain.
//!
//! A server has no desktop session: no Secret Service, no Keychain, no
//! Credential Manager. Asking the person who set Yaca up there to export
//! environment variables for every key they add from the phone is not a
//! settings screen, it is a chore, so [`crate::secrets`] falls back to this
//! when the platform store refuses a write.
//!
//! It is plaintext at rest, readable by the owner only (`0600` on Unix) — the
//! same protection an SSH key has on that machine, and no more. The keychain
//! stays the first choice wherever there is one.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::error::CoreError;
use crate::provider::registry;

const FILE: &str = "secrets.json";

type Items = BTreeMap<String, String>;

fn path() -> Result<PathBuf, CoreError> {
    Ok(registry::config_dir()?.join(FILE))
}

/// A file that does not parse reads as empty and is replaced on the next write:
/// the alternative, refusing to store a key until someone repairs the file by
/// hand, is worse than asking for the key again.
fn load() -> Items {
    path()
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(items: &Items) -> Result<(), CoreError> {
    let path = path()?;
    let fail = |e: std::io::Error| CoreError::Config(format!("cannot save {}: {e}", path.display()));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(fail)?;
    }
    let text = serde_json::to_string_pretty(items).map_err(|e| CoreError::Config(e.to_string()))?;
    // Written through a temporary file that is already private when it is
    // created: a key must never sit in a file others can read, even for the
    // moment between the write and a later `chmod`.
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    let mut file = private_file(&temp).map_err(fail)?;
    file.write_all(text.as_bytes()).map_err(fail)?;
    file.sync_all().map_err(fail)?;
    fs::rename(&temp, &path).map_err(fail)
}

#[cfg(unix)]
fn private_file(path: &std::path::Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn private_file(path: &std::path::Path) -> std::io::Result<fs::File> {
    fs::File::create(path)
}

pub(crate) fn get(name: &str) -> Option<String> {
    load().remove(name)
}

pub(crate) fn set(name: &str, value: &str) -> Result<(), CoreError> {
    let mut items = load();
    items.insert(name.to_string(), value.to_string());
    save(&items)
}

/// Missing is not a failure: the goal was that it be gone.
pub(crate) fn forget(name: &str) -> Result<(), CoreError> {
    let mut items = load();
    if items.remove(name).is_some() {
        save(&items)?;
    }
    Ok(())
}
