//! What the server needs from the application that hosts it.
//!
//! The server owns the socket and the protocol; the application owns the chats.
//! The desktop shell answers with the chats its window shows, so a turn started
//! at the desk is answered from the phone, and `yaca serve` answers with chats of
//! its own. Neither the server nor the core keeps a second table of who is open:
//! that would be a copy to drift from the one the window reads.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::watch;
use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::usage::ModelInfo;
use yaca_core::{SessionHandle, SessionMeta};

use crate::protocol::Startup;

/// A chat running on the host.
#[derive(Clone)]
pub struct LiveChat {
    pub handle: Arc<SessionHandle>,
    /// The project as the host's list names it.
    pub project: String,
}

/// Errors are English text for the human, carried to the client as
/// [`ErrorCode::Host`](crate::protocol::ErrorCode::Host).
#[async_trait]
pub trait Host: Send + Sync + 'static {
    /// The machine's name, for the phone's list of hosts.
    fn name(&self) -> String;

    /// The host's projects, most recent first.
    fn projects(&self) -> Result<Vec<String>, String>;

    /// The recorded chats of a project.
    fn sessions(&self, project: &str) -> Result<Vec<SessionMeta>, String>;

    /// The feed of a recorded chat, read from its journal. Blocking work: the
    /// server calls it off the async threads.
    fn feed(&self, project: &str, session: &SessionId) -> Result<Vec<AgentEvent>, String>;

    /// Every model the host's providers offer.
    fn models(&self) -> Result<Vec<ModelInfo>, String>;

    /// Opens a chat the way the host opens one of its own, and adds it to the
    /// host's live chats (which bumps [`Host::chats_changed`]). A `resume` of a
    /// chat that is already live hands back the live one.
    async fn open_chat(
        &self,
        project: &str,
        resume: Option<SessionId>,
        startup: Option<Startup>,
    ) -> Result<LiveChat, String>;

    /// The chats running right now, in the order they were opened.
    fn live_chats(&self) -> Vec<LiveChat>;

    fn chat(&self, session: &SessionId) -> Option<LiveChat>;

    /// Stops the turn and closes the chat on the host. Closing one that is not
    /// open is not an error.
    async fn close_chat(&self, session: &SessionId) -> Result<(), String>;

    /// Changes whenever a chat is opened or closed, by anyone. The value is
    /// only a counter: the list itself is [`Host::live_chats`].
    fn chats_changed(&self) -> watch::Receiver<u64>;
}

/// The machine's name for the phone's list of hosts: the environment's on
/// Windows and where a shell sets `HOSTNAME`, the kernel's file elsewhere. Both
/// hosts use it, so a machine is listed under one name whichever serves it.
pub fn machine_name() -> String {
    std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .and_then(non_empty)
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .and_then(non_empty)
        })
        .unwrap_or_else(|| "Yaca".to_string())
}

fn non_empty(text: String) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
