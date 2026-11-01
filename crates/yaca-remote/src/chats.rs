//! The live chats as the phone sees them: the list it is told whenever it changes,
//! and one attention watcher per chat that runs, for as long as the connection does.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use yaca_core::id::SessionId;
use yaca_core::SessionHandle;

use crate::attention;
use crate::connection::Conn;
use crate::host::{Host, LiveChat};
use crate::protocol::{ChatSummary, ServerMessage};

/// What the phone is told about one live chat. The one place that builds it, so
/// the list, the open reply and any later change agree.
pub(crate) fn summary(live: &LiveChat) -> ChatSummary {
    ChatSummary {
        id: live.handle.id().clone(),
        project: live.project.clone(),
        state: live.handle.state(),
        meta: live.handle.meta(),
    }
}

pub(crate) struct ChatsWatch {
    changes: watch::Receiver<u64>,
    attention: HashMap<SessionId, Watched>,
}

/// An attention watcher and the handle it watches.
struct Watched {
    handle: Arc<SessionHandle>,
    stop: CancellationToken,
}

impl ChatsWatch {
    /// Subscribes before the first snapshot is taken: a change that lands between
    /// the two would otherwise never be published.
    pub(crate) fn new(host: &dyn Host) -> Self {
        Self {
            changes: host.chats_changed(),
            attention: HashMap::new(),
        }
    }

    /// Sends the current list of live chats and brings the attention watchers in
    /// line with it. `false` when the connection is gone.
    pub(crate) async fn publish(&mut self, conn: &Conn) -> bool {
        let live = conn.shared.host.live_chats();
        self.follow(&live, conn);
        let chats = live.iter().map(summary).collect();
        conn.outbox.send(ServerMessage::ChatsChanged { chats }).await
    }

    /// Stops the watchers of chats that closed and starts one for each chat that
    /// opened. A watcher is tied to the handle it watches, so a chat closed and
    /// opened again under the same id gets a fresh watcher.
    fn follow(&mut self, live: &[LiveChat], conn: &Conn) {
        self.attention.retain(|id, watched| {
            let running = live
                .iter()
                .any(|chat| chat.handle.id() == id && Arc::ptr_eq(&chat.handle, &watched.handle));
            if !running {
                watched.stop.cancel();
            }
            running
        });
        for chat in live {
            let id = chat.handle.id();
            if self.attention.contains_key(id) {
                continue;
            }
            let stop = conn.token.child_token();
            // Subscribed before the list that tells the phone about this chat goes
            // out, so a turn that starts once the phone knows of the chat is seen.
            let events = chat.handle.subscribe();
            tokio::spawn(attention::watch(chat.clone(), events, conn.outbox.clone(), stop.clone()));
            self.attention.insert(
                id.clone(),
                Watched {
                    handle: Arc::clone(&chat.handle),
                    stop,
                },
            );
        }
    }

    /// Publishes again on every change of the host's chats, until the connection
    /// ends or the host is gone.
    pub(crate) async fn run(mut self, conn: Conn) {
        loop {
            tokio::select! {
                () = conn.token.cancelled() => break,
                changed = self.changes.changed() => {
                    // The sender lives as long as the host does: an error means the
                    // host is gone and nothing more will change.
                    if changed.is_err() {
                        break;
                    }
                }
            }
            if !self.publish(&conn).await {
                break;
            }
        }
    }
}
