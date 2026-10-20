//! The host: everything between the core and whatever shows it.
//!
//! The core is headless; a window or a phone is not. What stood between them
//! lived in the Tauri shell, which made the window the only way to reach the
//! core. It lives here now, with no window in it: the table of open chats, the
//! bridge that batches the event stream, the settings, and the commands the
//! interface calls. Two things sit on top — the desktop shell, which calls
//! [`Host::call`] in-process, and `yaca-server`, which calls it for a client on
//! another machine. Both get the same commands under the same names, and the
//! same events through [`Shell::emit`].
//!
//! What only a desktop window has — the browser column, OS toasts — is not
//! here. The shell supplies it through the hooks of [`Shell`], and a host
//! without a window simply leaves them as they are.

mod bridge;
mod chats;
mod dispatch;
mod files;
mod mcp;
mod plugins;
mod prefs;
mod project;
mod providers;
mod routing;
mod sessions;
mod settings;
mod state;
mod update;

use std::any::Any;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;
use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::provider::registry;
use yaca_core::SessionConfig;

pub use bridge::EventFrame;
pub use chats::{AttachedChat, OpenedChat, Startup};
pub use files::{DirEntry, DirListing};
pub use prefs::{Language, Preferences, PrefsHandle, Theme, PREFS_CHANNEL};
pub use providers::PROVIDERS_CHANNEL;
pub use update::UPDATE_CHANNEL;

/// Channel that event batches travel through. One name for everything: the interface is
/// a pure function of a single stream, and splitting channels by event type
/// would spread that function across several subscribers.
pub const EVENT_CHANNEL: &str = "yaca://events";

/// What the host needs from whoever carries it.
///
/// `emit` is the only required method: a payload on a named channel, to every
/// listener there is. The rest are what a desktop window adds around a chat and
/// a bare server has no use for, so each has an empty default.
pub trait Shell: Send + Sync + 'static {
    /// Delivers `payload` on `channel` to every listener. Nobody listening is
    /// not an error: the journal is what a late listener catches up from.
    fn emit(&self, channel: &str, payload: serde_json::Value);

    /// Before a chat's session is made: add to its configuration what only this
    /// shell can offer (the desktop's browser column is an MCP server of its
    /// own). Whatever is returned comes back to [`Shell::chat_opened`].
    fn prepare_chat(&self, _config: &mut SessionConfig) -> Option<Box<dyn Any + Send>> {
        None
    }

    /// After the chat exists, with what [`Shell::prepare_chat`] returned.
    fn chat_opened(&self, _id: &SessionId, _prepared: Option<Box<dyn Any + Send>>) {}

    /// The chat is closed or deleted and nothing of it should outlive it.
    fn chat_closed(&self, _id: &SessionId) {}

    /// A second subscriber to a chat's stream, for things that are not
    /// transport: the desktop's toast on a permission request. Ends with `stop`.
    fn watch_chat(&self, _events: Receiver<AgentEvent>, _stop: CancellationToken, _prefs: PrefsHandle) {}
}

impl dyn Shell {
    /// `emit` for anything serializable. A payload that does not serialize is
    /// dropped: every type sent here is plain data, so it would be a bug, not a
    /// condition to recover from.
    pub fn emit_value<T: Serialize>(&self, channel: &str, payload: &T) {
        if let Ok(value) = serde_json::to_value(payload) {
            self.emit(channel, value);
        }
    }
}

/// The host: open chats, preferences and the commands over them.
pub struct Host {
    pub(crate) shell: Arc<dyn Shell>,
    pub(crate) state: state::AppState,
}

impl Host {
    /// Makes a host and starts what runs on its own: the background update check.
    pub fn new(shell: Arc<dyn Shell>) -> Arc<Host> {
        let host = Arc::new(Host {
            shell,
            state: state::AppState::new(),
        });
        // Before the first window draws anything: the toast watcher asks about
        // preferences on every event.
        if let Ok(dir) = config_dir() {
            host.state
                .prefs()
                .set(settings::AppSettings::load(&dir).prefs().clone());
        }
        update::watch(host.clone());
        host
    }

    /// The chat a command is addressed to.
    pub(crate) fn chat(&self, session: &str) -> Result<Arc<yaca_core::SessionHandle>, String> {
        self.state
            .chat(&SessionId::from(session.to_owned()))
            .ok_or_else(|| format!("chat {session} is not open"))
    }
}

/// Directory of global settings.
///
/// The core computes it, not the shell: `yaca-cli` reads the same `providers.json`,
/// and two different notions of where it lies would mean that a gateway
/// configured in the window does not answer from the CLI.
pub(crate) fn config_dir() -> Result<PathBuf, String> {
    registry::config_dir().map_err(|e| e.to_string())
}

/// The only spelling of a project path.
///
/// Everything that comes from outside — from the folder dialog, from the settings
/// list, from the window — is brought to the same form that `Workspace::open`
/// brings a session's root to. Otherwise `D:\Kod` and `\\?\D:\Kod` would be two
/// different projects: one with the chats, the other with the journals.
pub(crate) fn project_path(path: &str) -> Result<PathBuf, String> {
    let canon = std::fs::canonicalize(path).map_err(|e| format!("could not open {path}: {e}"))?;
    if !canon.is_dir() {
        return Err(format!("{} — not a directory", canon.display()));
    }
    Ok(canon)
}
