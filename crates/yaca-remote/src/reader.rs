//! The client's messages after the handshake: requests, handled one at a time.

use futures_util::stream::SplitStream;
use futures_util::StreamExt;
use tokio_tungstenite::tungstenite::Message;

use crate::calls;
use crate::connection::{Conn, Ws};
use crate::protocol::{CallError, CallResult, ClientMessage, ErrorCode, ServerMessage};

/// Reads until the client closes, the socket fails, or the connection is stopped.
/// Requests are awaited here, not spawned: a second request waits for the first,
/// which is the ordering the protocol promises.
pub(crate) async fn run(mut stream: SplitStream<Ws>, conn: &Conn) {
    loop {
        let next = tokio::select! {
            () = conn.token.cancelled() => break,
            next = stream.next() => next,
        };
        match next {
            Some(Ok(Message::Text(text))) => {
                if !request(conn, text.as_str()).await {
                    break;
                }
            }
            Some(Ok(Message::Close(_))) | None => break,
            // Binary frames, pings and pongs carry nothing the protocol uses, and
            // tungstenite answers the pings on its own.
            Some(Ok(_)) => {}
            Some(Err(e)) => {
                tracing::debug!(error = %e, "connection read failed");
                break;
            }
        }
    }
}

/// Handles one text message. `false` when the connection can no longer be answered.
async fn request(conn: &Conn, text: &str) -> bool {
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(ClientMessage::Request { id, call }) => calls::answer(conn, id, call).await,
        // A handshake message has no id to answer, so there is nothing to tell the
        // client; it is dropped.
        Ok(_) => {
            tracing::debug!("a handshake message after the handshake, ignored");
            true
        }
        Err(e) => match request_id(text) {
            Some(id) => {
                let message = format!("the request could not be read: {e}");
                let error = CallError {
                    code: ErrorCode::BadRequest,
                    message,
                };
                conn.outbox
                    .send(ServerMessage::Response {
                        id,
                        result: CallResult::Error { error },
                    })
                    .await
            }
            None => {
                tracing::debug!(error = %e, "a message that is not a request, ignored");
                true
            }
        },
    }
}

/// The `id` of a message that did not parse as a request, so the client still
/// learns which of its requests failed.
fn request_id(text: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()?
        .get("id")?
        .as_u64()
}
