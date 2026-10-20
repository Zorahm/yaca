//! What the host holds: the chats that are open right now.
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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use yaca_core::id::SessionId;
use yaca_core::{Attach, Replay, SessionHandle};
use tokio_util::sync::CancellationToken;

use crate::prefs::PrefsHandle;
use crate::Shell;

/// A live chat: the core session and the bridge pumping its events out.
struct Chat {
    handle: Arc<SessionHandle>,
    /// The project as the window named it when it opened the chat. A window that
    /// was reloaded asks for it back, and the session's own spelling of the path
    /// is not necessarily the one the window files its chats under.
    project: String,
    /// Which bridge of this chat is current. Counted up each time the window
    /// attaches again; see [`crate::bridge`].
    epoch: u32,
    /// When it was opened, counting from the first. A table keyed by id has no
    /// order of its own, and a window that attaches again should list its chats
    /// the way it did before.
    seq: u64,
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
    opened: AtomicU64,
    /// Preferences as they are right now (§10).
    ///
    /// A copy of what is on disk, kept because the toast watcher asks about
    /// notifications on every event and reading a file there would put the
    /// disk in the path of a permission request. The file stays the truth;
    /// this is refreshed by the same command that writes it.
    prefs: PrefsHandle,
    /// The last answer of the update check, so a window opened later (the
    /// settings one) sees what the first window was told.
    update: Mutex<Option<yaca_core::update::UpdateCheck>>,
}

impl AppState {
    pub fn new() -> Self {
        AppState::default()
    }

    pub fn prefs(&self) -> &PrefsHandle {
        &self.prefs
    }

    pub fn update(&self) -> Option<yaca_core::update::UpdateCheck> {
        self.update.lock().unwrap().clone()
    }

    pub fn set_update(&self, check: yaca_core::update::UpdateCheck) {
        *self.update.lock().unwrap() = Some(check);
    }

    pub fn chat(&self, id: &SessionId) -> Option<Arc<SessionHandle>> {
        self.chats.lock().unwrap().get(id).map(|c| c.handle.clone())
    }

    /// Puts a chat in the table and raises its bridge.
    ///
    /// The same id twice means the same session was opened again; the previous
    /// bridge is cancelled as the entry is replaced, or the window would get
    /// every event of that chat twice.
    pub fn open(&self, handle: Arc<SessionHandle>, project: String, shell: &Arc<dyn Shell>) {
        let stop = CancellationToken::new();
        let id = handle.id().clone();
        tokio::spawn(crate::bridge::pump(
            shell.clone(),
            handle.subscribe(),
            stop.clone(),
            crate::EVENT_CHANNEL,
            id.clone(),
            0,
        ));
        // The toast of §15 listens to the same stream separately: the bridge is about transport,
        // the watcher is about notifications, and folding them into one file would mean
        // mixing two responsibilities.
        shell.watch_chat(handle.subscribe(), stop.clone(), self.prefs.clone());

        let chat = Chat {
            handle,
            project,
            epoch: 0,
            seq: self.opened.fetch_add(1, Ordering::Relaxed),
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

    /// Puts a new bridge in place of the old one for every chat that is open.
    ///
    /// The webview reloads and the sessions do not: they go on with their turns,
    /// their commands and their questions, and the window that comes back has
    /// lost only its feed. This is what hands it a way to get the feed back: each
    /// chat is stood at a point in its stream, and the bridge carries everything
    /// after that point while [`Rebound::replay`] is everything before.
    ///
    /// The old bridge is cancelled first and the epoch moves on, so a frame of
    /// the old one that is still on its way is told apart from the new ones and
    /// not folded twice.
    pub fn rebind_all(&self, shell: &Arc<dyn Shell>) -> Vec<Rebound> {
        let mut chats = self.chats.lock().unwrap();
        let mut in_order: Vec<(&SessionId, &mut Chat)> = chats.iter_mut().collect();
        in_order.sort_by_key(|(_, chat)| chat.seq);
        in_order
            .into_iter()
            .map(|(id, chat)| {
                chat.bridge.cancel();
                let stop = CancellationToken::new();
                chat.bridge = stop.clone();
                chat.epoch += 1;

                let Attach { events, replay } = chat.handle.attach();
                tokio::spawn(crate::bridge::pump(
                    shell.clone(),
                    events,
                    stop.clone(),
                    crate::EVENT_CHANNEL,
                    id.clone(),
                    chat.epoch,
                ));
                // Watching the toasts starts over with the bridge: the one that
                // was cancelled took its own stop token with it.
                shell.watch_chat(chat.handle.subscribe(), stop, self.prefs.clone());

                Rebound {
                    handle: chat.handle.clone(),
                    project: chat.project.clone(),
                    epoch: chat.epoch,
                    replay,
                }
            })
            .collect()
    }
}

/// A chat that has a new bridge and has not yet told the window what came before it.
pub struct Rebound {
    pub handle: Arc<SessionHandle>,
    pub project: String,
    pub epoch: u32,
    pub replay: Replay,
}
