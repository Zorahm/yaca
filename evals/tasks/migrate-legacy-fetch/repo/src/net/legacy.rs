/// Old entry point, kept only for the transition.
#[deprecated(note = "use net::fetch")]
pub fn legacy_fetch(url: &str) -> String {
    super::fetch(url)
}
