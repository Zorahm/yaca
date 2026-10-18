//! Getting a plugin's files into a staging directory, from wherever its
//! source says they are.
//!
//! Nothing that arrives is run: no `npm install`, no build step, no script the
//! package ships. A plugin is files, and what the files declare runs only once
//! the plugin is loaded into a session — through the same hooks and MCP rules
//! as a plugin put in place by hand.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::git;
use super::marketplace::{Entry, EntrySource, Manifest};
use crate::plugin::{agent, claude, contained};

/// A package in the staging directory.
pub(super) struct Fetched {
    pub package: PathBuf,
    /// The commit the files came from, when they came from git.
    pub revision: Option<String>,
}

/// The entry's files, under `staging/package`.
pub(super) fn from_entry(market_dir: &Path, manifest: &Manifest, entry: &Entry, staging: &Path) -> Result<Fetched, String> {
    let package = staging.join("package");
    let revision = match &entry.source {
        EntrySource::Relative(rel) => {
            let market = std::fs::canonicalize(market_dir).map_err(|e| format!("the marketplace cannot be opened: {e}"))?;
            let base = match &manifest.plugin_root {
                Some(root) => market.join(trim(root)),
                None => market.clone(),
            };
            let from = contained(&market, &base.join(trim(rel)))
                .ok_or_else(|| format!("plugin path «{rel}» was not found inside the marketplace"))?;
            git::copy_tree(&from, &package)?;
            git::head(&market)
        }
        EntrySource::Git { url, reference, sha, path } => {
            let download = staging.join("download");
            let revision = sha.as_deref().or(reference.as_deref());
            let commit = git::fetch(url, revision, &download)?;
            let root = std::fs::canonicalize(&download).map_err(|e| e.to_string())?;
            let from = match path {
                Some(p) => contained(&root, &root.join(trim(p)))
                    .ok_or_else(|| format!("the repository has no directory «{p}»"))?,
                None => root,
            };
            git::copy_tree(&from, &package)?;
            Some(commit)
        }
        EntrySource::Unsupported(kind) => return Err(format!("source «{kind}» is not supported in Yaca")),
    };
    apply_entry(&package, entry)?;
    Ok(Fetched { package, revision })
}

/// A package from an address a human typed: a folder, `owner/repo`, or a git
/// address.
pub(super) fn from_address(raw: &str, staging: &Path) -> Result<Fetched, String> {
    let package = staging.join("package");
    let raw = raw.trim();
    if Path::new(raw).is_dir() {
        git::copy_tree(Path::new(raw), &package)?;
        return Ok(Fetched {
            package,
            revision: git::head(Path::new(raw)),
        });
    }
    let url = match super::marketplace::MarketplaceSource::parse(raw)? {
        super::marketplace::MarketplaceSource::Github { repo, .. } => format!("https://github.com/{repo}.git"),
        super::marketplace::MarketplaceSource::Git { url, .. } => url,
        super::marketplace::MarketplaceSource::Directory { path } => {
            git::copy_tree(&path, &package)?;
            return Ok(Fetched { package, revision: git::head(&path) });
        }
    };
    let download = staging.join("download");
    let commit = git::fetch(&url, None, &download)?;
    git::copy_tree(&download, &package)?;
    Ok(Fetched {
        package,
        revision: Some(commit),
    })
}

fn trim(path: &str) -> &str {
    path.trim_start_matches("./").trim_start_matches('/')
}

/// What the marketplace entry says about the plugin, made its manifest where
/// Claude Code would read it so: with `strict: false`, or when the package has
/// no manifest of its own. A package in the Agent Plugins format keeps its own
/// manifest, since a Claude one beside it would change how it is read.
fn apply_entry(package: &Path, entry: &Entry) -> Result<(), String> {
    let manifest_path = package.join(claude::MANIFEST_DIR).join(claude::MANIFEST);
    let has_claude = manifest_path.is_file();
    let has_agent = package.join(agent::MANIFEST).is_file();
    if entry.strict && (has_claude || has_agent) {
        return Ok(());
    }
    let mut manifest: Map<String, Value> = if has_claude {
        std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    } else {
        Map::new()
    };
    for (key, value) in &entry.fields {
        if !["source", "strict", "category", "tags"].contains(&key.as_str()) {
            manifest.insert(key.clone(), value.clone());
        }
    }
    std::fs::create_dir_all(package.join(claude::MANIFEST_DIR)).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&Value::Object(manifest)).map_err(|e| e.to_string())?;
    std::fs::write(&manifest_path, text).map_err(|e| format!("cannot write the manifest: {e}"))
}
