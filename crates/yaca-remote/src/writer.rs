//! The one task that writes to a client's socket, and the queue that feeds it.
//!
//! Everything the server sends goes through one queue, so messages leave in the
//! order they were queued (a reply before the frames that follow it) and only this
//! task touches the sink. The queue is bounded: a client that stops reading fills
//! it, and the sender finds out instead of the server buffering without limit.

use std::time::Duration;

use futures_util::{Sink, SinkExt};
use tokio::sync::mpsc;
use tokio::time::{interval_at, timeout, Instant};
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use crate::protocol::ServerMessage;

pub(crate) const OUTBOX_CAPACITY: usize = 1024;
/// A ping every 30 s keeps a quiet connection alive through NAT mappings and a
/// phone's radio sleeping. A dead peer shows as a failed write.
const PING_EVERY: Duration = Duration::from_secs(30);
/// Bounds the close frame, so a peer that has stopped reading cannot hold the task.
const CLOSE_WAIT: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub(crate) struct Outbox(mpsc::Sender<ServerMessage>);

pub(crate) fn channel() -> (Outbox, mpsc::Receiver<ServerMessage>) {
    let (tx, rx) = mpsc::channel(OUTBOX_CAPACITY);
    (Outbox(tx), rx)
}

impl Outbox {
    /// Queues a message, waiting for room. `false` when the connection is gone.
    pub(crate) async fn send(&self, message: ServerMessage) -> bool {
        self.0.send(message).await.is_ok()
    }

    /// Queues a message only if there is room. `false` when the queue is full or
    /// the connection is gone; in both cases the caller must not wait, because the
    /// wait would hold up everything else behind this client.
    pub(crate) fn try_send(&self, message: ServerMessage) -> bool {
        self.0.try_send(message).is_ok()
    }
}

/// Writes queued messages until the connection ends. A failed write ends the
/// connection: the socket is no longer usable, and the jobs that feed the queue
/// stop with it.
pub(crate) async fn run<S>(
    mut sink: S,
    mut messages: mpsc::Receiver<ServerMessage>,
    token: CancellationToken,
) where
    S: Sink<Message> + Unpin,
{
    let mut ping = interval_at(Instant::now() + PING_EVERY, PING_EVERY);
    loop {
        tokio::select! {
            () = token.cancelled() => {
                let _ = timeout(CLOSE_WAIT, sink.send(Message::Close(None))).await;
                break;
            }
            message = messages.recv() => {
                let Some(message) = message else { break };
                let Some(frame) = encode(&message) else { continue };
                if sink.send(frame).await.is_err() {
                    break;
                }
            }
            _ = ping.tick() => {
                if sink.send(Message::Ping(Default::default())).await.is_err() {
                    break;
                }
            }
        }
    }
    token.cancel();
}

/// One server message as a WebSocket text frame. The handshake writes before the
/// writer task exists, and uses this too, so both paths send the same bytes.
pub(crate) fn encode(message: &ServerMessage) -> Option<Message> {
    match serde_json::to_string(message) {
        Ok(text) => Some(Message::text(text)),
        Err(e) => {
            tracing::error!(error = %e, "could not encode a message for the client");
            None
        }
    }
}
