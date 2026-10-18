//! A server in the application's own process ([`crate::mcp::app`]): messages
//! go over channels, whole, with nothing to frame or parse.

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

use super::{Inbox, Incoming, Transport};
use crate::mcp::app::{AppServer, ServerEnd};
use crate::mcp::McpError;

/// Why the client lost the server: the application dropped its end.
const GONE: &str = "приложение закрыло сервер";

pub(crate) struct App {
    to_server: mpsc::UnboundedSender<Value>,
    /// Carries the server's messages to the client. Stopped with the
    /// transport, which also drops `to_server` — the server's sign to stop.
    reader: AbortHandle,
}

impl App {
    pub(crate) fn open(server: &dyn AppServer, inbox: Inbox) -> App {
        let (to_server, incoming) = mpsc::unbounded_channel();
        let (outgoing, mut from_server) = mpsc::unbounded_channel();
        server.open(ServerEnd { incoming, outgoing });
        let reader = tokio::spawn(async move {
            while let Some(message) = from_server.recv().await {
                if inbox.send(Incoming::Message(message)).is_err() {
                    return;
                }
            }
            let _ = inbox.send(Incoming::Closed(GONE.into()));
        })
        .abort_handle();
        App { to_server, reader }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

#[async_trait]
impl Transport for App {
    async fn send(&self, message: Value) -> Result<(), McpError> {
        self.to_server
            .send(message)
            .map_err(|_| McpError::Closed(GONE.into()))
    }
}
