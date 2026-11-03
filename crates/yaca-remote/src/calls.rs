//! The requests a connected client makes, and what the host answers.
//!
//! Requests are handled one at a time, in arrival order, and each gets exactly one
//! response. That is what lets a client send a `SendMessage` and then a `Cancel`
//! and trust that the cancel finds the message already taken in by the core.

use std::sync::Arc;

use yaca_core::id::SessionId;
use yaca_core::Command;

use crate::attach::{self, Started};
use crate::chats::summary;
use crate::connection::Conn;
use crate::host::Host;
use crate::protocol::{Call, CallError, CallResult, ErrorCode, Reply, ServerMessage};

/// Answers one request. `false` when the connection is gone and nothing more can be
/// sent on it.
pub(crate) async fn answer(conn: &Conn, id: u64, call: Call) -> bool {
    let mut pump = None;
    let result = match dispatch(conn, call, &mut pump).await {
        Ok(reply) => CallResult::Ok { reply },
        Err(error) => CallResult::Error { error },
    };
    if !conn
        .outbox
        .send(ServerMessage::Response { id, result })
        .await
    {
        return false;
    }
    // Only now: an attachment's frames must come after its reply, never before it.
    if let Some(started) = pump {
        started.spawn();
    }
    true
}

async fn dispatch(conn: &Conn, call: Call, pump: &mut Option<Started>) -> Result<Reply, CallError> {
    let host = &conn.shared.host;
    match call {
        Call::ListProjects => blocking(host, |h| h.projects())
            .await
            .map(|projects| Reply::Projects { projects }),
        Call::ListSessions { project } => blocking(host, move |h| h.sessions(&project))
            .await
            .map(|sessions| Reply::Sessions { sessions }),
        Call::ListModels => blocking(host, |h| h.models())
            .await
            .map(|models| Reply::Models { models }),
        Call::ListChats => Ok(Reply::Chats {
            chats: host.live_chats().iter().map(summary).collect(),
        }),
        Call::SessionFeed { project, session } => {
            blocking(host, move |h| h.feed(&project, &session))
                .await
                .map(|events| Reply::Feed { events })
        }
        Call::OpenChat {
            project,
            resume,
            startup,
        } => {
            let live = host
                .open_chat(&project, resume, startup)
                .await
                .map_err(host_error)?;
            Ok(Reply::Opened {
                chat: Box::new(summary(&live)),
            })
        }
        Call::Attach { session } => attach::attach(conn, &session, pump).await,
        // Detaching what is not attached is not an error: the answer is the same.
        Call::Detach { session } => {
            conn.attachments.detach(&session);
            Ok(Reply::Done)
        }
        Call::CloseChat { session } => {
            host.close_chat(&session).await.map_err(host_error)?;
            Ok(Reply::Done)
        }
        Call::Send { session, command } => send(host, &session, command).await,
    }
}

/// Runs a host read that may touch the disk, off the async threads.
async fn blocking<T, F>(host: &Arc<dyn Host>, read: F) -> Result<T, CallError>
where
    T: Send + 'static,
    F: FnOnce(&dyn Host) -> Result<T, String> + Send + 'static,
{
    let host = Arc::clone(host);
    let joined = tokio::task::spawn_blocking(move || read(host.as_ref())).await;
    joined
        .map_err(|e| host_error(e.to_string()))?
        .map_err(host_error)
}

async fn send(
    host: &Arc<dyn Host>,
    session: &SessionId,
    command: Command,
) -> Result<Reply, CallError> {
    let live = host.chat(session).ok_or_else(chat_not_open)?;
    live.handle
        .send(command)
        .await
        .map_err(|e| host_error(e.to_string()))?;
    Ok(Reply::Done)
}

/// The error for a chat the host does not have running.
pub(crate) fn chat_not_open() -> CallError {
    CallError {
        code: ErrorCode::ChatNotOpen,
        message: "the chat is not open on this host".into(),
    }
}

/// A failure of the host, carried to the client with the host's own words.
pub(crate) fn host_error(message: String) -> CallError {
    CallError {
        code: ErrorCode::Host,
        message,
    }
}
