//! The Tauri shell: a window around the host, on a desktop, and a bare webview
//! around a remote host on a phone.
//!
//! On a desktop the work is `yaca-host`'s, in this process: the window sends
//! `host_call` and listens to the events the host emits. What only a window has
//! — the browser column, OS toasts — is in the modules here and joins the host
//! through the hooks of `yaca_host::Shell`.
//!
//! On a phone there is no host in the process at all: the core's tools (shell,
//! language servers, the keychain) have no business on an Android device, and
//! the interface talks to a `yaca-server` over the network instead.

#[cfg(desktop)]
mod browser;
#[cfg(desktop)]
mod desktop;
#[cfg(desktop)]
mod notify;
#[cfg(desktop)]
mod profile;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(desktop)]
    desktop::run();

    #[cfg(mobile)]
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("could not start the application");
}
