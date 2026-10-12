//! `file://` addresses, for the protocols that name files by them (MCP roots,
//! LSP documents).

use std::path::{Path, PathBuf};

/// A path as a `file://` address. Only what a path can hold and an address
/// cannot is escaped.
pub(crate) fn file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    // The verbatim prefix `canonicalize` leaves on Windows is not part of the
    // path anyone else knows it by.
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    let mut uri = String::from("file://");
    if !text.starts_with('/') {
        uri.push('/');
    }
    for ch in text.chars() {
        match ch {
            ' ' => uri.push_str("%20"),
            '%' => uri.push_str("%25"),
            '#' => uri.push_str("%23"),
            '?' => uri.push_str("%3F"),
            other => uri.push(other),
        }
    }
    uri
}

/// The path a `file://` address names. `None` for any other scheme.
///
/// Servers percent-encode more than [`file_uri`] does — the drive colon on
/// Windows, non-ASCII letters — so everything is decoded.
pub(crate) fn path_of(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // `file://host/path` has a host part; only the empty one is local.
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let mut bytes = Vec::with_capacity(rest.len());
    let raw = rest.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            if let Ok(hex) = std::str::from_utf8(&raw[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    bytes.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        bytes.push(raw[i]);
        i += 1;
    }
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    // `/C:/Kod` is `C:/Kod` on Windows.
    let drive = text.as_bytes();
    if drive.len() >= 3 && drive[0] == b'/' && drive[2] == b':' && drive[1].is_ascii_alphabetic() {
        text.remove(0);
        // Servers disagree about the case of a drive letter; the roots use the
        // capital one.
        text[..1].make_ascii_uppercase();
    }
    Some(PathBuf::from(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_becomes_an_address_a_server_can_parse() {
        assert_eq!(file_uri(Path::new("/home/me/my project")), "file:///home/me/my%20project");
        assert_eq!(file_uri(Path::new(r"C:\Kod\yaca")), "file:///C:/Kod/yaca");
        assert_eq!(file_uri(Path::new(r"\\?\C:\Kod")), "file:///C:/Kod");
    }

    #[test]
    fn an_address_comes_back_to_the_path_it_was_made_from() {
        assert_eq!(path_of("file:///home/me/my%20project"), Some("/home/me/my project".into()));
        assert_eq!(path_of("file:///c%3A/Kod/%D1%91.rs"), Some("C:/Kod/ё.rs".into()));
        assert_eq!(path_of("file:///C:/Kod"), Some("C:/Kod".into()));
        assert_eq!(path_of("https://example.test/a"), None);
    }
}
