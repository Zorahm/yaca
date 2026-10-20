//! The host behind a WebSocket.
//!
//! `yaca-host` runs the core and holds the chats; this is the door for an
//! interface that is not in the same process — Yaca on a phone, or on a
//! desktop that works on a remote machine. Nothing here knows what a chat is:
//! it authorizes a connection, forwards calls to [`Host::call`] and forwards
//! the host's events back.
//!
//! **One window owns the streams.** `attach_chats` re-binds every chat's
//! bridge and moves its epoch on, so a second client attaching would silently
//! cut the first one off. A connection that authorizes therefore replaces the
//! previous one, which is told so ([`wire::Bye::Replaced`]) and says it on
//! screen, instead of two devices each showing half of a conversation.
//!
//! **A dropped line costs nothing.** Chats live in the host, not in the
//! connection. Events emitted while nobody is connected are not queued; the
//! client that comes back attaches and gets the journal, which is the same
//! road a reloaded desktop window takes.

pub mod tokens;
pub mod wire;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use yaca_host::{Host, Shell};

use tokens::Tokens;
use wire::{Bye, Down, Up};

/// Longest a connection may stay quiet. Clients ping every twenty seconds, so
/// this is three missed pings: the line is dead and the socket is let go.
const IDLE: Duration = Duration::from_secs(60);

/// How long a connection has to authorize before it is dropped.
const AUTH_WITHIN: Duration = Duration::from_secs(10);

/// Pause before answering a wrong token, so guessing is slow.
const WRONG_TOKEN_DELAY: Duration = Duration::from_secs(1);

/// Messages waiting for a client. A client further behind than this is not
/// coming back to the stream in any useful way; it is cut off and catches up
/// from the journal.
const QUEUE: usize = 4096;

/// Largest message accepted. Pasted images arrive as base64.
const MAX_MESSAGE: usize = 64 * 1024 * 1024;

struct Conn {
    id: u64,
    tx: mpsc::Sender<Down>,
    cancel: CancellationToken,
}

/// Where the host's events go: to the one connection there is.
#[derive(Default)]
struct Hub {
    current: Mutex<Option<Conn>>,
    next: AtomicU64,
}

impl Hub {
    /// Makes this connection the current one. The previous one is told and let go.
    fn install(&self, tx: mpsc::Sender<Down>, cancel: CancellationToken) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let old = self.current.lock().unwrap().replace(Conn { id, tx, cancel });
        if let Some(old) = old {
            let _ = old.tx.try_send(Down::Bye { reason: Bye::Replaced });
            old.cancel.cancel();
        }
        id
    }

    fn release(&self, id: u64) {
        let mut current = self.current.lock().unwrap();
        if current.as_ref().is_some_and(|c| c.id == id) {
            *current = None;
        }
    }
}

impl Shell for Hub {
    fn emit(&self, channel: &str, payload: serde_json::Value) {
        let mut current = self.current.lock().unwrap();
        let Some(conn) = current.as_ref() else { return };
        let event = Down::Event {
            channel: channel.to_string(),
            payload,
        };
        if conn.tx.try_send(event).is_err() {
            // Full, or already gone. Either way a stream with a hole in it is
            // worse than no stream: cut it, the client re-attaches.
            conn.cancel.cancel();
            *current = None;
        }
    }
}

pub struct Server {
    host: Arc<Host>,
    hub: Arc<Hub>,
    tokens: Tokens,
}

impl Server {
    /// A server over the configuration directory's token file and a fresh host.
    pub fn new(config_dir: &std::path::Path) -> Arc<Server> {
        let hub = Arc::new(Hub::default());
        // `Host::new` starts the background update check on the current runtime.
        let host = Host::new(hub.clone());
        Arc::new(Server {
            host,
            hub,
            tokens: Tokens::in_dir(config_dir),
        })
    }
}

pub fn router(server: Arc<Server>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ws", get(upgrade))
        .with_state(server)
}

async fn health() -> impl IntoResponse {
    format!("yaca-server {}\n", env!("CARGO_PKG_VERSION"))
}

async fn upgrade(State(server): State<Arc<Server>>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.max_message_size(MAX_MESSAGE)
        .max_frame_size(MAX_MESSAGE)
        .on_upgrade(move |socket| connection(server, socket))
}

fn frame(down: &Down) -> Message {
    Message::Text(down.text().into())
}

async fn refuse(mut ws: WebSocket, reason: Bye) {
    let _ = ws.send(frame(&Down::Bye { reason })).await;
    let _ = ws
        .send(Message::Close(Some(CloseFrame {
            code: 4401,
            reason: "unauthorized".into(),
        })))
        .await;
}

async fn connection(server: Arc<Server>, mut ws: WebSocket) {
    // The first message authorizes, and nothing else is read before it.
    let first = tokio::time::timeout(AUTH_WITHIN, ws.recv()).await;
    let Ok(Some(Ok(Message::Text(text)))) = first else {
        return refuse(ws, Bye::Unauthorized).await;
    };
    let Ok(Up::Auth { token, client }) = serde_json::from_str::<Up>(&text) else {
        return refuse(ws, Bye::Unauthorized).await;
    };
    let Some(device) = server.tokens.check(&token) else {
        tokio::time::sleep(WRONG_TOKEN_DELAY).await;
        return refuse(ws, Bye::Unauthorized).await;
    };
    tracing::info!(%device, %client, "connected");

    let (tx, mut rx) = mpsc::channel::<Down>(QUEUE);
    let cancel = CancellationToken::new();
    let id = server.hub.install(tx.clone(), cancel.clone());
    if ws
        .send(frame(&Down::Ready {
            version: env!("CARGO_PKG_VERSION"),
        }))
        .await
        .is_err()
    {
        server.hub.release(id);
        return;
    }

    let (mut sink, mut stream) = ws.split();

    let writer_cancel = cancel.clone();
    let writer = tokio::spawn(async move {
        loop {
            tokio::select! {
                next = rx.recv() => match next {
                    Some(down) => {
                        if sink.send(frame(&down)).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                },
                _ = writer_cancel.cancelled() => {
                    // Whatever was queued first (the reason for closing) still goes out.
                    while let Ok(down) = rx.try_recv() {
                        if sink.send(frame(&down)).await.is_err() {
                            break;
                        }
                    }
                    break;
                }
            }
        }
        let _ = sink.close().await;
    });

    loop {
        let message = tokio::select! {
            _ = cancel.cancelled() => break,
            message = tokio::time::timeout(IDLE, stream.next()) => message,
        };
        let text = match message {
            Ok(Some(Ok(Message::Text(text)))) => text,
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) | Ok(Some(Err(_))) | Err(_) => break,
            // Binary, and the protocol's own ping/pong, carry nothing for us.
            Ok(Some(Ok(_))) => continue,
        };
        match serde_json::from_str::<Up>(&text) {
            Ok(Up::Call { id: call, cmd, args }) => {
                let host = server.host.clone();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let answer = host.call(&cmd, args).await;
                    let _ = tx.send(Down::result(call, answer)).await;
                });
            }
            Ok(Up::Ping) => {
                // A token revoked while the phone is connected ends the
                // connection at its next ping, not at its next reconnect.
                if server.tokens.check(&token).is_none() {
                    let _ = tx.try_send(Down::Bye { reason: Bye::Revoked });
                    break;
                }
                let _ = tx.try_send(Down::Pong);
            }
            Ok(Up::Auth { .. }) | Err(_) => break,
        }
    }

    server.hub.release(id);
    cancel.cancel();
    drop(tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), writer).await;
    tracing::info!(%device, "disconnected");
}
