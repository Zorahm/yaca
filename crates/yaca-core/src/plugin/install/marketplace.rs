//! Marketplaces: where they are, and what their `marketplace.json` says.
//!
//! The list a human sees is three lists in one, each name once:
//!
//! 1. their own, in `<config dir>/marketplaces.json`;
//! 2. the official one, `anthropics/claude-plugins-official`, there from the
//!    start as it is in Claude Code;
//! 3. the ones Claude Code knows (`~/.claude/plugins/known_marketplaces.json`).
//!
//! A built-in or borrowed one that the human removes is remembered as hidden.
//! Nothing is cloned until it is asked for: a list is names and addresses, and
//! the first look into a marketplace, or "update", is what fetches it, into
//! Yaca's own directory. Claude Code's clone is never read or written; it may be
//! weeks old.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::error::CoreError;
use crate::plugin::claude::is_namespace;
use crate::workspace::roots::plain;

pub const LIST_FILE: &str = "marketplaces.json";
pub const CLONES_DIR: &str = "marketplaces";
pub const MANIFEST: &str = ".claude-plugin/marketplace.json";
pub const OFFICIAL: &str = "claude-plugins-official";
const OFFICIAL_REPO: &str = "anthropics/claude-plugins-official";

/// Where a marketplace is fetched from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "camelCase")]
pub enum MarketplaceSource {
    Github {
        repo: String,
        #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
    },
    Git {
        url: String,
        #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
    },
    /// Read where it is, never copied.
    Directory { path: PathBuf },
}

impl MarketplaceSource {
    /// What a human typed: a folder that exists, `owner/repo`, or a git address.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        let path = Path::new(raw);
        if path.is_dir() {
            let path = std::fs::canonicalize(path).map_err(|e| format!("{raw}: {e}"))?;
            return Ok(MarketplaceSource::Directory { path });
        }
        if raw.contains("://") || raw.starts_with("git@") || raw.ends_with(".git") {
            return Ok(MarketplaceSource::Git {
                url: raw.to_string(),
                reference: None,
            });
        }
        let mut parts = raw.split('/');
        if let (Some(owner), Some(repo), None) = (parts.next(), parts.next(), parts.next()) {
            if is_namespace(owner) && is_namespace(repo) {
                return Ok(MarketplaceSource::Github {
                    repo: raw.to_string(),
                    reference: None,
                });
            }
        }
        Err(format!("«{raw}» — не папка, не owner/repo и не git-адрес"))
    }

    pub(super) fn url(&self) -> Option<String> {
        match self {
            MarketplaceSource::Github { repo, .. } => Some(format!("https://github.com/{repo}.git")),
            MarketplaceSource::Git { url, .. } => Some(url.clone()),
            MarketplaceSource::Directory { .. } => None,
        }
    }

    pub(super) fn reference(&self) -> Option<&str> {
        match self {
            MarketplaceSource::Github { reference, .. } | MarketplaceSource::Git { reference, .. } => {
                reference.as_deref()
            }
            MarketplaceSource::Directory { .. } => None,
        }
    }

    pub fn display(&self) -> String {
        match self {
            MarketplaceSource::Github { repo, .. } => repo.clone(),
            MarketplaceSource::Git { url, .. } => url.clone(),
            MarketplaceSource::Directory { path } => plain(path),
        }
    }
}

/// Where a marketplace in the list came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum MarketplaceOrigin {
    Builtin,
    ClaudeCode,
    Own,
}

#[derive(Debug, Clone)]
pub struct Known {
    pub name: String,
    pub source: MarketplaceSource,
    pub origin: MarketplaceOrigin,
}

impl Known {
    /// Where its `marketplace.json` is read from.
    pub fn dir(&self, config_dir: &Path) -> PathBuf {
        match &self.source {
            MarketplaceSource::Directory { path } => path.clone(),
            _ => config_dir.join(CLONES_DIR).join(&self.name),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct ListFile {
    #[serde(default)]
    pub marketplaces: BTreeMap<String, Stored>,
    #[serde(default)]
    pub hidden: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Stored {
    pub source: MarketplaceSource,
}

impl ListFile {
    pub(super) fn load(config_dir: &Path) -> Self {
        std::fs::read_to_string(config_dir.join(LIST_FILE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub(super) fn save(&self, config_dir: &Path) -> Result<(), CoreError> {
        super::write_json(&config_dir.join(LIST_FILE), self)
    }
}

/// Every marketplace in the list, by name.
pub fn known(config_dir: &Path, claude_home: Option<&Path>) -> Vec<Known> {
    let list = ListFile::load(config_dir);
    let mut out: BTreeMap<String, Known> = BTreeMap::new();
    if let Some(home) = claude_home {
        for (name, source) in claude_known(home) {
            out.insert(name.clone(), Known { name, source, origin: MarketplaceOrigin::ClaudeCode });
        }
    }
    out.insert(
        OFFICIAL.into(),
        Known {
            name: OFFICIAL.into(),
            source: MarketplaceSource::Github {
                repo: OFFICIAL_REPO.into(),
                reference: None,
            },
            origin: MarketplaceOrigin::Builtin,
        },
    );
    for name in &list.hidden {
        out.remove(name);
    }
    for (name, stored) in list.marketplaces {
        out.insert(
            name.clone(),
            Known {
                name,
                source: stored.source,
                origin: MarketplaceOrigin::Own,
            },
        );
    }
    out.into_values().collect()
}

/// Claude Code's `known_marketplaces.json`, the sources Yaca can fetch. A
/// marketplace that is one JSON file at a URL is not one of them.
fn claude_known(home: &Path) -> Vec<(String, MarketplaceSource)> {
    let file = home.join("plugins").join("known_marketplaces.json");
    let Some(Value::Object(doc)) = std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
    else {
        return Vec::new();
    };
    doc.into_iter()
        .filter(|(name, _)| is_namespace(name))
        .filter_map(|(name, entry)| {
            let source = entry.get("source")?;
            let text = |key: &str| source.get(key).and_then(Value::as_str).map(str::to_string);
            let reference = text("ref");
            let parsed = match source.get("source").and_then(Value::as_str)? {
                "github" => MarketplaceSource::Github { repo: text("repo")?, reference },
                "git" => MarketplaceSource::Git { url: text("url")?, reference },
                "directory" => MarketplaceSource::Directory { path: PathBuf::from(text("path")?) },
                _ => return None,
            };
            Some((name, parsed))
        })
        .collect()
}

/// One plugin a marketplace offers.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub homepage: Option<String>,
    pub source: EntrySource,
    /// Claude Code's `strict`: `false` makes the entry the plugin's manifest.
    pub strict: bool,
    /// The entry as written, for a manifest made from it.
    pub fields: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntrySource {
    /// A directory inside the marketplace itself.
    Relative(String),
    /// A repository, at a branch or tag (`reference`) or exactly at `sha`, and
    /// a directory inside it (`path`).
    Git {
        url: String,
        reference: Option<String>,
        sha: Option<String>,
        path: Option<String>,
    },
    /// What Yaca cannot fetch (`npm`, `pip`, …), by its kind.
    Unsupported(String),
}

impl EntrySource {
    pub fn display(&self) -> String {
        match self {
            EntrySource::Relative(path) => path.clone(),
            EntrySource::Git { url, path, .. } => match path {
                Some(p) => format!("{url} → {p}"),
                None => url.clone(),
            },
            EntrySource::Unsupported(kind) => kind.clone(),
        }
    }
}

/// A marketplace's `marketplace.json`, read.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    /// `metadata.pluginRoot`: what relative sources are relative to.
    pub plugin_root: Option<String>,
    pub plugins: Vec<Entry>,
}

pub fn read(dir: &Path) -> Result<Manifest, String> {
    let file = dir.join(MANIFEST);
    let text = std::fs::read_to_string(&file).map_err(|_| format!("нет {}", plain(&file)))?;
    let Value::Object(doc) = serde_json::from_str::<Value>(&text).map_err(|e| format!("marketplace.json — не JSON: {e}"))? else {
        return Err("marketplace.json — не объект".into());
    };
    let name = match doc.get("name").and_then(Value::as_str) {
        Some(n) if is_namespace(n) => n.to_string(),
        Some(n) => return Err(format!("имя маркетплейса «{n}» нельзя использовать")),
        None => return Err("в marketplace.json нет name".into()),
    };
    let plugin_root = doc
        .get("metadata")
        .and_then(|m| m.get("pluginRoot"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let plugins = doc
        .get("plugins")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(entry).collect())
        .unwrap_or_default();
    Ok(Manifest { name, plugin_root, plugins })
}

fn entry(value: &Value) -> Option<Entry> {
    let fields = value.as_object()?.clone();
    let name = fields.get("name").and_then(Value::as_str).filter(|n| is_namespace(n))?.to_string();
    let text = |key: &str| fields.get(key).and_then(Value::as_str).map(str::to_string);
    let source = match fields.get("source") {
        Some(Value::String(path)) => EntrySource::Relative(path.clone()),
        Some(Value::Object(s)) => source(s),
        _ => EntrySource::Unsupported("—".into()),
    };
    Some(Entry {
        description: text("description"),
        version: text("version"),
        category: text("category"),
        homepage: text("homepage"),
        strict: fields.get("strict").and_then(Value::as_bool).unwrap_or(true),
        name,
        source,
        fields,
    })
}

fn source(s: &Map<String, Value>) -> EntrySource {
    let text = |key: &str| s.get(key).and_then(Value::as_str).map(str::to_string);
    let kind = text("source").unwrap_or_default();
    let url = match kind.as_str() {
        "github" => text("repo").map(|r| format!("https://github.com/{r}.git")),
        "url" | "git" | "git-subdir" => text("url"),
        _ => None,
    };
    match url {
        Some(url) => EntrySource::Git {
            url,
            reference: text("ref"),
            sha: text("sha"),
            path: text("path"),
        },
        None => EntrySource::Unsupported(if kind.is_empty() { "—".into() } else { kind }),
    }
}
