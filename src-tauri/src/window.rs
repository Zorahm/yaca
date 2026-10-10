//! The settings window (§10).
//!
//! A window of its own, not a drawer over the feed. §10 asks for one, and the
//! reason holds up in use: a provider is fixed while the agent keeps working
//! in the other window, and the feed is not made inert to do it. The drawer
//! §1.1 that carried providers until now was written down as an interim step
//! (§16, п. 30) and this is what it was waiting for.
//!
//! One instance. Asking again focuses the window that is already open rather
//! than making a second one: two settings windows would be two answers about
//! what the theme is, and the last one closed would win.
//!
//! The window loads `settings.html`, a second entry point of the same
//! frontend bundle (`vite.config.ts`), so it shares the tokens, the
//! dictionaries and the client with the main window.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// The label the window is found by. Not `main`, which [`crate::notify`] asks
/// about focus: a toast is about the conversation, and the settings window
/// being focused does not mean the question was seen.
const LABEL: &str = "settings";

/// Opens the settings window, or focuses the one already open.
///
/// Human-readable text in the shell is Russian, as everywhere in `src-tauri`:
/// the window title is set before any dictionary is loaded.
#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        // Already open — and possibly behind the main window or minimized.
        let _ = window.unminimize();
        let _ = window.show();
        return window.set_focus().map_err(|e| e.to_string());
    }

    WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("settings.html".into()))
        .title("Настройки")
        // The size the sections were drawn for: navigation 208 and a content
        // column that does not stretch a settings row across the screen.
        .inner_size(900.0, 640.0)
        .min_inner_size(760.0, 520.0)
        .build()
        .map(|_| ())
        .map_err(|e| format!("не открыть окно настроек: {e}"))
}
