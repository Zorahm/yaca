//! One connection from the TCP accept to its last message: TLS, the WebSocket, the
//! handshake, and then the three jobs that last as long as the client is there (the
//! writer, the chats watcher and the reader).
//!
//! The order is the security. Nothing from the peer is parsed before TLS has
//! finished, since the certificate is what the phone pinned, and nothing is
//! answered before the handshake has named a device.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use rustls::ServerConfig;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::WebSocketStream;
use tokio_util::sync::CancellationToken;

use crate::attach::Attachments;
use crate::chats::ChatsWatch;
use crate::error::Error;
use crate::handshake;
use crate::reader;
use crate::shared::Shared;
use crate::writer::{self, Outbox};

pub(crate) type Ws = WebSocketStream<TlsStream<TcpStream>>;

/// Covers the TLS handshake, the WebSocket upgrade and the first message. A peer
/// that stalls in any of them is dropped rather than held open.
pub(crate) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// The largest message or frame a client may send. A request is a few hundred
/// bytes, so the cap only stops a peer from making the server buffer gigabytes.
const MAX_MESSAGE_BYTES: usize = 1 << 20;

/// What the jobs of one connection share.
#[derive(Clone)]
pub(crate) struct Conn {
    pub(crate) shared: Arc<Shared>,
    pub(crate) outbox: Outbox,
    /// Cancelled when the connection ends, or when its device is revoked.
    pub(crate) token: CancellationToken,
    pub(crate) attachments: Arc<Attachments>,
}

/// Serves one accepted socket until it ends, the server stops, or its device is
/// revoked.
pub(crate) async fn serve(socket: TcpStream, peer: SocketAddr, shared: Arc<Shared>) {
    let token = shared.cancel.child_token();
    tokio::select! {
        () = token.cancelled() => {}
        () = run(socket, peer, &shared, &token) => {}
    }
    // Ends the writer and the watchers, whichever side finished first.
    token.cancel();
}

async fn run(socket: TcpStream, peer: SocketAddr, shared: &Arc<Shared>, token: &CancellationToken) {
    let mut ws = match timeout(HANDSHAKE_TIMEOUT, open(socket, Arc::clone(&shared.tls))).await {
        Ok(Ok(ws)) => ws,
        Ok(Err(e)) => {
            // Not a refusal: a peer that does not speak TLS or WebSocket is not
            // trying to pair, so it is not counted against the address.
            tracing::debug!(%peer, error = %e, "connection dropped before the websocket opened");
            return;
        }
        Err(_) => {
            tracing::debug!(%peer, "connection timed out before the websocket opened");
            return;
        }
    };
    let Some(_live) = handshake::run(&mut ws, shared, peer.ip(), token).await else {
        return;
    };

    let (sink, stream) = ws.split();
    let (outbox, messages) = writer::channel();
    let conn = Conn {
        shared: Arc::clone(shared),
        outbox,
        token: token.clone(),
        attachments: Arc::default(),
    };
    // The first list is queued before the reader starts, so it is the first thing
    // the phone hears after Welcome, ahead of any reply.
    let mut chats = ChatsWatch::new(shared.host.as_ref());
    if !chats.publish(&conn).await {
        return;
    }
    tokio::spawn(writer::run(sink, messages, token.clone()));
    tokio::spawn(chats.run(conn.clone()));
    reader::run(stream, &conn).await;
}

async fn open(socket: TcpStream, tls: Arc<ServerConfig>) -> Result<Ws, Error> {
    let tls = TlsAcceptor::from(tls).accept(socket).await?;
    let mut config = WebSocketConfig::default();
    config.max_message_size = Some(MAX_MESSAGE_BYTES);
    config.max_frame_size = Some(MAX_MESSAGE_BYTES);
    Ok(tokio_tungstenite::accept_async_with_config(tls, Some(config)).await?)
}
