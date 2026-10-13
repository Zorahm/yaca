//! What the shell holds: the chats that are open right now.
//!
//! Several at once, in one project or in many. The core never held one at a
//! time — `spawn` hands back an independent session with its own id, its own
//! event channel and its own store — so all that stood in the way was this
//! table having room for a single entry.
//!
//! Nothing here is the «current» chat. Which one the window is showing is a
//! question about the window, and the window answers it; commands name the
//! session they are for. A shell that also had an opinion would be a second
//! source of truth, and the two would part ways on the first switch.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use yaca_core::id::SessionId;
use yaca_core::SessionHandle;
use tokio_util::sync::CancellationToken;

use crate::prefs::Preferences;

/// A live chat: the core session and the bridge pumping its events out.
struct Chat {
    handle: Arc<SessionHandle>,
    /// Stops this chat's side jobs: the bridge to the webview and the
    /// watcher that raises an OS toast on permission requests.
    ///
    /// One per chat: a watcher left running for a closed session is a source
    /// of toasts out of nowhere.
    bridge: CancellationToken,
}

#[derive(Default)]
pub struct AppState {
    chats: Mutex<HashMap<SessionId, Chat>>,
    /// Preferences as they are right now (§10).
    ///
    /// A copy of what is on disk, kept because the toast watcher asks about
    /// notifications on every event and reading a file there would put the
    /// disk in the path of a permission request. The file stays the truth;
    /// this is refreshed by the same command that writes it.
    prefs: Mutex<Preferences>,
}

impl AppState {
    pub fn new() -> Self {
        AppState::default()
    }

    pub fn prefs(&self) -> Preferences {
        self.prefs.lock().unwrap().clone()
    }

    pub fn set_prefs(&self, prefs: Preferences) {
        *self.prefs.lock().unwrap() = prefs;
    }

    pub fn chat(&self, id: &SessionId) -> Option<Arc<SessionHandle>> {
        self.chats.lock().unwrap().get(id).map(|c| c.handle.clone())
    }

    /// Puts a chat in the table and raises its bridge.
    ///
    /// The same id twice means the same session was opened again; the previous
    /// bridge is cancelled as the entry is replaced, or the window would get
    /// every event of that chat twice.
    pub fn open(&self, handle: Arc<SessionHandle>, app: &tauri::AppHandle) {
        let stop = CancellationToken::new();
        let id = handle.id().clone();
        tokio::spawn(crate::bridge::pump(
            app.clone(),
            handle.subscribe(),
            stop.clone(),
            crate::EVENT_CHANNEL,
            id.clone(),
        ));
        // The toast of §15 listens to the same stream separately: the bridge is about transport,
        // the watcher is about notifications, and folding them into one file would mean
        // mixing two responsibilities.
        tokio::spawn(crate::notify::watch(app.clone(), handle.subscribe(), stop.clone()));

        let chat = Chat {
            handle,
            bridge: stop,
        };
        if let Some(old) = self.chats.lock().unwrap().insert(id, chat) {
            old.bridge.cancel();
        }
    }

    /// Takes the chat out of the table. The handle comes back so the caller
    /// can stop the turn before letting go of it — dropping is what ends the
    /// session, and with it the background commands it started (§6.5).
    pub fn close(&self, id: &SessionId) -> Option<Arc<SessionHandle>> {
        let chat = self.chats.lock().unwrap().remove(id)?;
        chat.bridge.cancel();
        Some(chat.handle)
    }

    /// Empties the table the same way.
    pub fn close_all(&self) -> Vec<Arc<SessionHandle>> {
        let chats = std::mem::take(&mut *self.chats.lock().unwrap());
        chats
            .into_values()
            .map(|chat| {
                chat.bridge.cancel();
                chat.handle
            })
            .collect()
    }
}
