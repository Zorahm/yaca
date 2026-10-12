use crate::net::legacy::legacy_fetch;

/// The stock level document for `sku`.
pub fn level(sku: &str) -> String {
    legacy_fetch(&format!("https://api.example/stock/{sku}"))
}
