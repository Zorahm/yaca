use crate::net::legacy::legacy_fetch;

/// The profile document for user `id`.
pub fn profile(id: u32) -> String {
    legacy_fetch(&format!("https://api.example/users/{id}"))
}
