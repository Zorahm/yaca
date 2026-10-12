pub mod legacy;

/// Fetches `url` and returns the body. The one entry point handlers use.
pub fn fetch(url: &str) -> String {
    format!("body of {url}")
}
