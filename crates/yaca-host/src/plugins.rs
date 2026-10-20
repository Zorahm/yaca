//! Plugins and their marketplaces (§10).

use yaca_core::plugin::install::{CatalogEntry, Installer, MarketplaceSummary, UpdateOutcome};
use yaca_core::plugin::{PluginReport, PluginSources, PluginSummary};

use crate::config_dir;

/// Every plugin found — Yaca's own and Claude Code's — with its switch and
/// what did not load. Read on every call, like the skills.
pub fn list_plugins() -> Result<PluginReport, String> {
    Ok(PluginSources::standard(&config_dir()?).load().report())
}

/// Turns a plugin on or off for the chats opened after this; an open chat
/// keeps the plugins it started with, as it keeps its MCP servers.
pub fn set_plugin_enabled(id: &str, enabled: bool) -> Result<PluginReport, String> {
    let config = config_dir()?;
    yaca_core::plugin::set_enabled(&config, id, enabled).map_err(|e| e.to_string())?;
    Ok(PluginSources::standard(&config).load().report())
}

/// Runs an installer operation off the async workers: it is git and the disk,
/// and a clone takes as long as the network does.
async fn installing<T: Send + 'static>(
    work: impl FnOnce(&Installer) -> Result<T, yaca_core::error::CoreError> + Send + 'static,
) -> Result<T, String> {
    let config = config_dir()?;
    tokio::task::spawn_blocking(move || work(&Installer::standard(&config)).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

pub async fn list_marketplaces() -> Result<Vec<MarketplaceSummary>, String> {
    installing(|i| Ok(i.marketplaces())).await
}

pub async fn add_marketplace(source: String) -> Result<MarketplaceSummary, String> {
    installing(move |i| i.add_marketplace(&source)).await
}

pub async fn remove_marketplace(name: String) -> Result<(), String> {
    installing(move |i| i.remove_marketplace(&name)).await
}

pub async fn refresh_marketplace(name: String) -> Result<MarketplaceSummary, String> {
    installing(move |i| i.refresh_marketplace(&name)).await
}

/// What a marketplace offers; fetches it the first time it is looked into.
pub async fn marketplace_catalog(name: String) -> Result<Vec<CatalogEntry>, String> {
    installing(move |i| i.catalog(&name)).await
}

/// `plugin@marketplace` from the catalog. Installed plugins are on, and reach
/// the chats opened after this.
pub async fn install_plugin(id: String) -> Result<PluginSummary, String> {
    installing(move |i| i.install(&id)).await
}

pub async fn update_plugin(id: String) -> Result<UpdateOutcome, String> {
    installing(move |i| i.update(&id)).await
}

pub async fn uninstall_plugin(id: String) -> Result<(), String> {
    installing(move |i| i.uninstall(&id)).await
}
