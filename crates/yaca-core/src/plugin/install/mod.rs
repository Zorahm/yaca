//! Installing plugins: marketplaces, fetching, updating, removing.
//!
//! Claude Code's marketplace format (`.claude-plugin/marketplace.json`); Agent
//! Plugins defines none, and a plugin in that format is installed by address.
//! Everything lives under the configuration directory:
//!
//! ```text
//! marketplaces.json                       the human's list (`marketplace`)
//! marketplaces/<name>/                    clones
//! plugin-cache/<marketplace>/<plugin>/<version>/   installed copies, no .git
//! installed_plugins.json                  what is installed, from where (`registry`)
//! ```
//!
//! An install is checked before it counts. The files go to a staging directory
//! first and are read by the same loader a session uses; only a package that
//! loader accepts is moved into place, so a failed install leaves nothing
//! behind and a broken plugin is refused at the door rather than reported on
//! every chat.
//!
//! Installing is the consent, as in Claude Code: the plugin is enabled. What it
//! brings — hooks, MCP servers — is in the summary the install returns.
//!
//! Blocking: this is git and the filesystem. The applications run it off their
//! event loop.

mod fetch;
mod git;
pub mod marketplace;
mod registry;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use crate::error::CoreError;
use crate::plugin::{read_package, set_enabled, PluginSources, PluginSummary};
use crate::workspace::roots::plain;
use marketplace::{Known, MarketplaceOrigin, MarketplaceSource};
use registry::{From, Install, Registry};

pub use registry::installed;

pub const CACHE_DIR: &str = "plugin-cache";
/// The marketplace name of a plugin installed by address.
pub const DIRECT: &str = "direct";

/// A marketplace as the window and the CLI show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceSummary {
    pub name: String,
    pub source: String,
    pub origin: MarketplaceOrigin,
    /// Its `marketplace.json` is on disk: cloned, or a folder.
    pub fetched: bool,
    #[ts(type = "number | null")]
    pub plugins: Option<u32>,
    /// Why the fetched copy cannot be read.
    pub error: Option<String>,
}

/// One plugin a marketplace offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    /// `plugin@marketplace`.
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub homepage: Option<String>,
    pub source: String,
    /// Why Yaca cannot install it; `None` when it can.
    pub unavailable: Option<String>,
    /// The version installed, if it is.
    pub installed: Option<String>,
}

/// What an update did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOutcome {
    pub id: String,
    pub from: String,
    pub to: String,
    pub changed: bool,
}

pub struct Installer {
    config_dir: PathBuf,
    claude_home: Option<PathBuf>,
}

fn err(text: impl Into<String>) -> CoreError {
    CoreError::Config(text.into())
}

impl Installer {
    pub fn new(config_dir: impl Into<PathBuf>, claude_home: Option<PathBuf>) -> Self {
        Installer {
            config_dir: config_dir.into(),
            claude_home,
        }
    }

    pub fn standard(config_dir: &Path) -> Self {
        Installer::new(config_dir, crate::plugin::claude_home())
    }

    fn known(&self) -> Vec<Known> {
        marketplace::known(&self.config_dir, self.claude_home.as_deref())
    }

    fn find(&self, name: &str) -> Result<Known, CoreError> {
        self.known()
            .into_iter()
            .find(|k| k.name == name)
            .ok_or_else(|| err(format!("no marketplace {name}; list them with yaca plugins marketplace list")))
    }

    pub fn marketplaces(&self) -> Vec<MarketplaceSummary> {
        self.known().iter().map(|k| self.summary(k)).collect()
    }

    fn summary(&self, k: &Known) -> MarketplaceSummary {
        let dir = k.dir(&self.config_dir);
        let fetched = dir.join(marketplace::MANIFEST).is_file();
        let (plugins, error) = if fetched {
            match marketplace::read(&dir) {
                Ok(m) => (Some(m.plugins.len() as u32), None),
                Err(why) => (None, Some(why)),
            }
        } else {
            (None, None)
        };
        MarketplaceSummary {
            name: k.name.clone(),
            source: k.source.display(),
            origin: k.origin,
            fetched,
            plugins,
            error,
        }
    }

    /// Adds a marketplace by address and fetches it: its name is in its
    /// `marketplace.json`, so it cannot be listed before that.
    pub fn add_marketplace(&self, raw: &str) -> Result<MarketplaceSummary, CoreError> {
        let source = MarketplaceSource::parse(raw).map_err(err)?;
        let name = match &source {
            MarketplaceSource::Directory { path } => marketplace::read(path).map_err(err)?.name,
            remote => {
                let staging = self.staging("marketplace")?;
                let result = (|| -> Result<String, CoreError> {
                    let url = remote.url().unwrap_or_default();
                    git::fetch(&url, remote.reference(), &staging.join("repo")).map_err(err)?;
                    let name = marketplace::read(&staging.join("repo")).map_err(err)?.name;
                    let target = self.config_dir.join(marketplace::CLONES_DIR).join(&name);
                    git::remove_tree(&target);
                    std::fs::create_dir_all(target.parent().unwrap_or(&self.config_dir)).map_err(|e| err(e.to_string()))?;
                    std::fs::rename(staging.join("repo"), &target).map_err(|e| err(format!("cannot move the clone into place: {e}")))?;
                    Ok(name)
                })();
                git::remove_tree(&staging);
                result?
            }
        };
        let mut list = marketplace::ListFile::load(&self.config_dir);
        list.hidden.retain(|h| h != &name);
        list.marketplaces.insert(name.clone(), marketplace::Stored { source });
        list.save(&self.config_dir)?;
        let known = self.find(&name)?;
        Ok(self.summary(&known))
    }

    /// Takes a marketplace off the list. A built-in one, or one borrowed from
    /// Claude Code, is hidden rather than forgotten. What was installed from it
    /// stays installed; it can no longer be updated from it.
    pub fn remove_marketplace(&self, name: &str) -> Result<(), CoreError> {
        let known = self.find(name)?;
        let mut list = marketplace::ListFile::load(&self.config_dir);
        list.marketplaces.remove(name);
        if known.origin != MarketplaceOrigin::Own && !list.hidden.iter().any(|h| h == name) {
            list.hidden.push(name.to_string());
        }
        list.save(&self.config_dir)?;
        if !matches!(known.source, MarketplaceSource::Directory { .. }) {
            git::remove_tree(&known.dir(&self.config_dir));
        }
        Ok(())
    }

    /// Fetches a marketplace again, or for the first time. A folder is read
    /// where it is and has nothing to fetch.
    pub fn refresh_marketplace(&self, name: &str) -> Result<MarketplaceSummary, CoreError> {
        let known = self.find(name)?;
        self.fetch_marketplace(&known)?;
        Ok(self.summary(&known))
    }

    fn fetch_marketplace(&self, known: &Known) -> Result<(), CoreError> {
        let Some(url) = known.source.url() else {
            return Ok(());
        };
        let staging = self.staging("marketplace")?;
        let result = (|| {
            git::fetch(&url, known.source.reference(), &staging.join("repo")).map_err(err)?;
            let target = known.dir(&self.config_dir);
            git::remove_tree(&target);
            std::fs::create_dir_all(target.parent().unwrap_or(&self.config_dir)).map_err(|e| err(e.to_string()))?;
            std::fs::rename(staging.join("repo"), &target).map_err(|e| err(format!("cannot move the clone into place: {e}")))
        })();
        git::remove_tree(&staging);
        result
    }

    /// The marketplace's `marketplace.json`, fetched first if it never was.
    fn manifest(&self, known: &Known) -> Result<marketplace::Manifest, CoreError> {
        let dir = known.dir(&self.config_dir);
        if !dir.join(marketplace::MANIFEST).is_file() {
            self.fetch_marketplace(known)?;
        }
        marketplace::read(&dir).map_err(err)
    }

    /// What a marketplace offers, with what of it is installed.
    pub fn catalog(&self, name: &str) -> Result<Vec<CatalogEntry>, CoreError> {
        let known = self.find(name)?;
        let manifest = self.manifest(&known)?;
        let registry = Registry::load(&self.config_dir);
        Ok(manifest
            .plugins
            .iter()
            .map(|e| {
                let id = format!("{}@{name}", e.name);
                CatalogEntry {
                    installed: registry.plugins.get(&id).map(|i| i.version.clone()),
                    unavailable: match &e.source {
                        marketplace::EntrySource::Unsupported(kind) => {
                            Some(format!("source «{kind}» is not supported in Yaca"))
                        }
                        _ => None,
                    },
                    id,
                    name: e.name.clone(),
                    description: e.description.clone(),
                    version: e.version.clone(),
                    category: e.category.clone(),
                    homepage: e.homepage.clone(),
                    source: e.source.display(),
                }
            })
            .collect())
    }

    /// Installs `plugin@marketplace` and enables it.
    pub fn install(&self, id: &str) -> Result<PluginSummary, CoreError> {
        let (entry_name, market) = id
            .split_once('@')
            .ok_or_else(|| err(format!("«{id}» is not plugin@marketplace")))?;
        let known = self.find(market)?;
        let manifest = self.manifest(&known)?;
        let entry = manifest
            .plugins
            .iter()
            .find(|e| e.name == entry_name)
            .ok_or_else(|| err(format!("marketplace {market} has no plugin {entry_name}")))?;
        let dir = known.dir(&self.config_dir);
        let from = From::Marketplace {
            marketplace: market.to_string(),
            entry: entry_name.to_string(),
        };
        self.put(id, entry_name, market, entry.version.clone(), from, |staging| {
            fetch::from_entry(&dir, &manifest, entry, staging)
        })
        .map(|(summary, _)| summary)
    }

    /// Installs a plugin from an address — a folder, `owner/repo`, a git
    /// address — under `<name>@direct`.
    pub fn install_from(&self, address: &str) -> Result<PluginSummary, CoreError> {
        // The id carries the package's own name, known only once it is here;
        // what was fetched to learn it is what gets installed.
        let probe = self.staging("probe")?;
        let result = (|| {
            let fetched = fetch::from_address(address, &probe).map_err(err)?;
            let root = std::fs::canonicalize(&fetched.package).map_err(|e| err(e.to_string()))?;
            let name = read_package(&root, None).map_err(err)?.1.name;
            let id = format!("{name}@{DIRECT}");
            let from = From::Address {
                address: address.to_string(),
            };
            self.put(&id, &name, DIRECT, None, from, move |_| Ok(fetched))
        })();
        git::remove_tree(&probe);
        result.map(|(summary, _)| summary)
    }

    /// Fetches the plugin again and replaces it if its files changed.
    pub fn update(&self, id: &str) -> Result<UpdateOutcome, CoreError> {
        let registry = Registry::load(&self.config_dir);
        let old = registry
            .plugins
            .get(id)
            .cloned()
            .ok_or_else(|| err(format!("{id} was not installed through Yaca")))?;
        let new = match &old.from {
            From::Marketplace { marketplace, .. } => {
                let known = self.find(marketplace)?;
                self.fetch_marketplace(&known)?;
                self.install(id)?;
                Registry::load(&self.config_dir).plugins.get(id).cloned()
            }
            From::Address { address } => {
                self.install_from(address)?;
                Registry::load(&self.config_dir).plugins.get(id).cloned()
            }
        }
        .ok_or_else(|| err(format!("{id} disappeared from the registry during the update")))?;
        Ok(UpdateOutcome {
            id: id.to_string(),
            from: old.version,
            to: new.version,
            changed: old.content != new.content,
        })
    }

    /// Every plugin Yaca installed, updated; one that fails does not stop the
    /// others.
    pub fn update_all(&self) -> Vec<(String, Result<UpdateOutcome, CoreError>)> {
        let ids: Vec<String> = Registry::load(&self.config_dir).plugins.into_keys().collect();
        ids.into_iter().map(|id| {
            let outcome = self.update(&id);
            (id, outcome)
        }).collect()
    }

    /// Removes an installed plugin: its files, its entry and its switch. Its
    /// `PLUGIN_DATA` is kept — that is where a plugin keeps what the human made
    /// with it.
    pub fn uninstall(&self, id: &str) -> Result<(), CoreError> {
        let mut registry = Registry::load(&self.config_dir);
        let install = registry
            .plugins
            .remove(id)
            .ok_or_else(|| err(format!("{id} was not installed through Yaca")))?;
        registry.save(&self.config_dir)?;
        self.remove_install_dir(&install.path);
        crate::plugin::forget_switch(&self.config_dir, id)
    }

    /// Fetches into staging, checks, moves into place, records.
    ///
    /// Returns the plugin's summary and whether the files differ from the
    /// version that was installed before.
    fn put(
        &self,
        id: &str,
        name: &str,
        market: &str,
        declared_version: Option<String>,
        from: From,
        get: impl FnOnce(&Path) -> Result<fetch::Fetched, String>,
    ) -> Result<(PluginSummary, bool), CoreError> {
        let staging = self.staging("install")?;
        let result = (|| -> Result<bool, CoreError> {
            let fetched = get(&staging).map_err(err)?;
            let package = std::fs::canonicalize(&fetched.package).map_err(|e| err(e.to_string()))?;
            let (_, contents) = read_package(&package, Some(name))
                .map_err(|why| err(format!("the plugin was not accepted: {why}")))?;
            let content = registry::content_hash(&package);
            let version = declared_version
                .or(contents.version.clone())
                .or_else(|| fetched.revision.as_ref().map(|r| r.chars().take(12).collect()))
                .unwrap_or_else(|| content.clone());

            let mut registry = Registry::load(&self.config_dir);
            let previous = registry.plugins.get(id).cloned();
            let changed = previous.as_ref().is_none_or(|p| p.content != content);
            let target = self
                .config_dir
                .join(CACHE_DIR)
                .join(market)
                .join(name)
                .join(dir_safe(&version));
            if changed || previous.as_ref().is_none_or(|p| p.path != target) {
                git::remove_tree(&target);
                std::fs::create_dir_all(target.parent().unwrap_or(&self.config_dir)).map_err(|e| err(e.to_string()))?;
                std::fs::rename(&package, &target).map_err(|e| err(format!("cannot move the plugin into place: {e}")))?;
                if let Some(p) = &previous {
                    if p.path != target {
                        self.remove_install_dir(&p.path);
                    }
                }
            }
            registry.plugins.insert(
                id.to_string(),
                Install {
                    path: target,
                    version,
                    from,
                    revision: fetched.revision,
                    content,
                    installed_at: now(),
                },
            );
            registry.save(&self.config_dir)?;
            if previous.is_none() {
                set_enabled(&self.config_dir, id, true)?;
            }
            Ok(changed)
        })();
        git::remove_tree(&staging);
        let changed = result?;
        let report = PluginSources::new(&self.config_dir, self.claude_home.clone()).load().report();
        let summary = report
            .plugins
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| err(format!("{id} was installed, but did not load: its name is taken by another plugin")))?;
        Ok((summary, changed))
    }

    /// A directory under the cache that is removed when the installer is done
    /// with it, whatever happened.
    fn staging(&self, what: &str) -> Result<PathBuf, CoreError> {
        let dir = self
            .config_dir
            .join(CACHE_DIR)
            .join(format!(".{what}-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).map_err(|e| err(format!("cannot create {}: {e}", plain(&dir))))?;
        Ok(dir)
    }

    /// Removes an installed copy, and only one: a path outside the cache is
    /// never deleted, whatever the registry says.
    fn remove_install_dir(&self, path: &Path) {
        let cache = self.config_dir.join(CACHE_DIR);
        if path.starts_with(&cache) && path != cache {
            git::remove_tree(path);
        }
    }
}

fn dir_safe(text: &str) -> String {
    let out: String = text
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "-_.+".contains(c) { c } else { '_' })
        .collect();
    if out.is_empty() || out.starts_with('.') {
        format!("v{out}")
    } else {
        out
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> Result<(), CoreError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| err(format!("cannot create {}: {e}", dir.display())))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| err(e.to_string()))?;
    std::fs::write(path, text).map_err(|e| err(format!("cannot write {}: {e}", plain(path))))
}
