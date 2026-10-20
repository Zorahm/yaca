//! The desktop shell around the host.

use std::any::Any;
use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;
use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::SessionConfig;
use yaca_host::{Host, PrefsHandle, Shell};

use crate::{browser, notify, profile};

/// What the desktop window adds around the host: its events reach the webview,
/// each chat gets a browser column, and a permission request becomes a toast.
struct Desktop {
    app: AppHandle,
}

impl Shell for Desktop {
    fn emit(&self, channel: &str, payload: Value) {
        let _ = self.app.emit(channel, payload);
    }

    /// The chat's browser, offered to its session as an MCP server of the
    /// window's own. The page itself is made on first use.
    fn prepare_chat(&self, config: &mut SessionConfig) -> Option<Box<dyn Any + Send>> {
        let (browser, server) = self.app.state::<browser::Browsers>().prepare(&self.app)?;
        config.app_servers.push(server);
        Some(Box::new(browser))
    }

    fn chat_opened(&self, id: &SessionId, prepared: Option<Box<dyn Any + Send>>) {
        let Some(prepared) = prepared else { return };
        if let Ok(browser) = prepared.downcast::<Arc<browser::Browser>>() {
            self.app.state::<browser::Browsers>().bind(id.clone(), *browser);
        }
    }

    fn chat_closed(&self, id: &SessionId) {
        self.app.state::<browser::Browsers>().forget(id);
    }

    fn watch_chat(&self, events: Receiver<AgentEvent>, stop: CancellationToken, prefs: PrefsHandle) {
        tokio::spawn(notify::watch(self.app.clone(), events, stop, prefs));
    }
}

/// Every command of the host, by name. One Tauri command instead of seventy:
/// the names and arguments are the ones a remote client sends over the socket,
/// so the interface calls both the same way.
#[tauri::command]
async fn host_call(host: State<'_, Arc<Host>>, cmd: String, args: Value) -> Result<Value, String> {
    host.call(&cmd, args).await
}

/// The window's commands answer the window's own webview and nothing else.
///
/// The browser column's pages live in the same window, and Tauri lets the
/// application's commands answer any page it counts as local: a page of the
/// browser that got there must not be able to read chats or start one. The
/// capabilities file limits the plugins' commands the same way.
fn only_the_window(
    handler: impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    move |invoke| {
        if invoke.message.webview_ref().label() != "main" {
            invoke.resolver.reject("commands are for the Yaca window only");
            return true;
        }
        handler(invoke)
    }
}

pub fn run() {
    let context = tauri::generate_context!();
    // Before the builder: the webview opens its profile as the window is made.
    profile::adopt(&context.config().identifier);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(browser::Browsers::default())
        .setup(|app| {
            let shell = Arc::new(Desktop {
                app: app.handle().clone(),
            });
            // Inside the runtime: the host starts its background update check.
            let host = tauri::async_runtime::block_on(async { Host::new(shell) });
            app.manage(host);
            Ok(())
        })
        .invoke_handler(only_the_window(tauri::generate_handler![
            host_call,
            browser::browser_supported,
            browser::browser_states,
            browser::browser_place,
            browser::browser_open,
            browser::browser_go,
            browser::browser_external,
            browser::browser_devtools,
            browser::browser_reset_viewport,
        ]))
        .run(context)
        .expect("could not start the application");
}
