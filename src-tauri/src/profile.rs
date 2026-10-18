//! The webview profile the window had under its old identifier.
//!
//! Tauri names the webview's data directory after the identifier, and the
//! identifier changed with the rename to Yaca. What lives there is the
//! window's own state — `localStorage` with the composer drafts (§7.4) — and
//! without the move the window would open as if for the first time. The move
//! itself is the core's ([`yaca_core::legacy`]); this is only where it points.

use std::path::PathBuf;

use yaca_core::legacy;

/// Moves the profile before the webview opens it.
///
/// Windows only: WebView2 keeps the profile in `%LOCALAPPDATA%\<identifier>`,
/// a directory this process can move. WKWebView and WebKitGTK keep theirs
/// where the platform decides, and a draft there stays behind — see
/// `README.md`.
#[cfg(windows)]
pub fn adopt(identifier: &str) {
    if let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        legacy::adopt_dir(&base.join(legacy::IDENTIFIER), &base.join(identifier));
    }
}

#[cfg(not(windows))]
pub fn adopt(_identifier: &str) {}
