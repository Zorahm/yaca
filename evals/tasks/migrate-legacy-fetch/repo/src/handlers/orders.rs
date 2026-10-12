use crate::net::legacy::legacy_fetch;

/// The summary document for order `id`.
pub fn summary(id: u32) -> String {
    legacy_fetch(&format!("https://api.example/orders/{id}/summary"))
}
