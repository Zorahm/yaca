//! Attaching to a chat: the whole feed so far, then the chat's events as frames
//! until the client detaches, the chat ends, or the client falls behind.
//!
//! The reply is queued before the pump starts, so the frames follow the feed in
//! order and none can overtake it. Every attachment has an epoch, which its frames
//! carry: a client that attaches again drops the frames of the epoch it replaced.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::Attach;

use crate::calls::{chat_not_open, host_error};
use crate::connection::Conn;
use crate::frame::{self, Lag, PumpEnd};
use crate::protocol::{CallError, Reply, ServerMessage};

/// How often frames go out while a chat streams. The phone draws what it gets, and
/// a batch every 50 ms reads as motion without waking it for every delta.
pub(crate) const REMOTE_FRAME: Duration = Duration::from_millis(50);

/// The attachments of one connection, by chat. A slot outlives its attachment, so
/// the epoch keeps counting across detaches.
#[derive(Default)]
pub(crate) struct Attachments {
    slots: Mutex<HashMap<SessionId, Slot>>,
}

#[derive(Default)]
struct Slot {
    epoch: u32,
    /// Stops the pump of the current attachment. `None` when there is none.
    cancel: Option<CancellationToken>,
}

impl Attachments {
    /// Replaces the attachment of `session` with one stopped by `token`, and
    /// returns its epoch. The previous pump is stopped here: a second attach must
    /// not leave the first one sending frames under an epoch that is no longer
    /// current.
    fn restart(&self, session: &SessionId, token: CancellationToken) -> u32 {
        let mut slots = self.lock();
        let slot = slots.entry(session.clone()).or_default();
        if let Some(previous) = slot.cancel.replace(token) {
            previous.cancel();
        }
        slot.epoch += 1;
        slot.epoch
    }

    pub(crate) fn detach(&self, session: &SessionId) {
        if let Some(cancel) = self.lock().get_mut(session).and_then(|slot| slot.cancel.take()) {
            cancel.cancel();
        }
    }

    /// Forgets the attachment of `epoch` if it is still the current one. `false`
    /// when a newer attach has replaced it: the client has been given that one and
    /// is owed nothing for the old one.
    fn retire(&self, session: &SessionId, epoch: u32) -> bool {
        match self.lock().get_mut(session) {
            Some(slot) if slot.epoch == epoch => {
                slot.cancel = None;
                true
            }
            _ => false,
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<SessionId, Slot>> {
        self.slots.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Starts an attachment. The feed so far goes into the reply, and the pump that
/// follows is handed back in `pump` for the caller to start once the reply is queued.
pub(crate) async fn attach(
    conn: &Conn,
    session: &SessionId,
    pump: &mut Option<Started>,
) -> Result<Reply, CallError> {
    let live = conn.shared.host.chat(session).ok_or_else(chat_not_open)?;
    let token = conn.token.child_token();
    let epoch = conn.attachments.restart(session, token.clone());

    // Subscribed before the journal is read: an event that lands during the read is
    // kept for the pump instead of falling into the gap between the two.
    let Attach { events, replay } = live.handle.attach();
    let handle = live.handle.clone();
    let replayed = match tokio::task::spawn_blocking(move || handle.replay(&replay)).await {
        Ok(events) => events,
        Err(e) => {
            token.cancel();
            conn.attachments.retire(session, epoch);
            return Err(host_error(format!("could not read the chat's feed: {e}")));
        }
    };

    *pump = Some(Started {
        conn: conn.clone(),
        session: session.clone(),
        epoch,
        events,
        token,
    });
    Ok(Reply::Attached {
        session: session.clone(),
        epoch,
        events: frame::coalesce(replayed),
    })
}

/// An attachment whose pump has not started yet.
pub(crate) struct Started {
    conn: Conn,
    session: SessionId,
    epoch: u32,
    events: broadcast::Receiver<AgentEvent>,
    token: CancellationToken,
}

impl Started {
    pub(crate) fn spawn(self) {
        tokio::spawn(self.run());
    }

    async fn run(self) {
        let Started {
            conn,
            session,
            epoch,
            events,
            token,
        } = self;
        let overflowed = Arc::new(AtomicBool::new(false));
        let emit = {
            let outbox = conn.outbox.clone();
            let overflowed = Arc::clone(&overflowed);
            let token = token.clone();
            let session = session.clone();
            move |events: Vec<AgentEvent>| {
                if overflowed.load(Ordering::Relaxed) {
                    return;
                }
                let frame = ServerMessage::Frame {
                    session: session.clone(),
                    epoch,
                    events,
                };
                if !outbox.try_send(frame) {
                    // This client is not keeping up with the chat. Frames stop here
                    // and the client is told to attach again: a stream with a hole in
                    // it is worse than a whole feed fetched afresh.
                    overflowed.store(true, Ordering::Relaxed);
                    token.cancel();
                }
            }
        };

        let end = frame::pump(events, token, REMOTE_FRAME, Lag::Stop, emit).await;
        let overflowed = overflowed.load(Ordering::Relaxed);
        // Stopped by the client's own detach or attach, or by the connection ending:
        // the other side already knows, so there is nothing to tell.
        if end == PumpEnd::Cancelled && !overflowed {
            return;
        }
        if !conn.attachments.retire(&session, epoch) {
            return;
        }
        if end == PumpEnd::Lagged || overflowed {
            conn.outbox.send(ServerMessage::Resync { session }).await;
        }
    }
}
