//! Reports shared by desktop and CLI.
use super::marketplace::MarketplaceOrigin;
use serde::Serialize;
use ts_rs::TS;

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
    #[ts(type = "number | null")]
    pub updated_at: Option<u64>,
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
