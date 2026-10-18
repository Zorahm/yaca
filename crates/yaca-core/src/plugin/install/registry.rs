//! `<config dir>/installed_plugins.json`: what Yaca installed, and from where.
//!
//! Enough to load each plugin (`path`) and to update it (`from`). `content` is
//! a hash of the installed files, which is how an update tells a new version
//! from the same one fetched again: a marketplace's commit moves for every
//! plugin in it, a plugin's own files only when the plugin changed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

pub const FILE: &str = "installed_plugins.json";

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct Registry {
    #[serde(default)]
    pub plugins: BTreeMap<String, Install>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Install {
    pub path: PathBuf,
    pub version: String,
    pub from: From,
    /// The commit the files came from, when they came from git.
    #[serde(default)]
    pub revision: Option<String>,
    pub content: String,
    /// Seconds since the Unix epoch.
    pub installed_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub(super) enum From {
    Marketplace { marketplace: String, entry: String },
    /// `yaca plugins install <address>`: what was typed, to fetch again.
    Address { address: String },
}

impl Registry {
    pub(super) fn load(config_dir: &Path) -> Self {
        std::fs::read_to_string(config_dir.join(FILE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub(super) fn save(&self, config_dir: &Path) -> Result<(), CoreError> {
        super::write_json(&config_dir.join(FILE), self)
    }
}

/// Every plugin Yaca installed, by id, for the loader.
pub fn installed(config_dir: &Path) -> Vec<(String, PathBuf)> {
    Registry::load(config_dir)
        .plugins
        .into_iter()
        .map(|(id, install)| (id, install.path))
        .collect()
}

/// A hash over every file under `dir`, by relative path and bytes, in a fixed
/// order.
pub(super) fn content_hash(dir: &Path) -> String {
    let mut files = Vec::new();
    collect(dir, dir, &mut files);
    files.sort();
    let mut hasher = blake3::Hasher::new();
    for (rel, path) in files {
        hasher.update(rel.as_bytes());
        hasher.update(&[0]);
        if let Ok(bytes) = std::fs::read(&path) {
            hasher.update(&(bytes.len() as u64).to_le_bytes());
            hasher.update(&bytes);
        }
    }
    hasher.finalize().to_hex()[..16].to_string()
}

fn collect(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            collect(base, &path, out);
        } else if kind.is_file() {
            let rel = path.strip_prefix(base).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            out.push((rel, path));
        }
    }
}
