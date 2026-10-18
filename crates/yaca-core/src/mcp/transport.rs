//! How JSON-RPC messages reach a server and come back.
//!
//! The two the protocol defines — a child process spoken to over its standard
//! streams ([`stdio`]), and Streamable HTTP ([`http`]) — and a third for a
//! server that lives in the application itself ([`app`]). The client above
//! them sees none of this: it sends a message and reads whatever arrives.

pub(crate) mod app;
pub(crate) mod http;
pub(crate) mod stdio;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::mpsc;

use super::config::Launch;
use super::McpError;

/// What a transport hands up.
#[derive(Debug)]
pub(crate) enum Incoming {
    Message(Value),
    /// The server is gone, with why as far as it can be told. Nothing arrives
    /// after this.
    Closed(String),
}

pub(crate) type Inbox = mpsc::UnboundedSender<Incoming>;

#[async_trait]
pub(crate) trait Transport: Send + Sync {
    /// Sends one message. Answers arrive through the inbox, in any order.
    ///
    /// For HTTP this may not return until the server has answered, so a caller
    /// that wants to give up waits on this and on its own timer together.
    async fn send(&self, message: Value) -> Result<(), McpError>;

    /// The handshake settled on this protocol revision.
    fn negotiated(&self, _version: &str) {}
}

/// Opens the transport a declaration asks for.
pub(crate) async fn open(launch: Launch, inbox: Inbox) -> Result<Arc<dyn Transport>, McpError> {
    Ok(match launch {
        Launch::Stdio {
            program,
            args,
            env,
            cwd,
        } => Arc::new(stdio::Stdio::spawn(&program, &args, &env, &cwd, inbox)?),
        Launch::Http { url, headers } => Arc::new(http::Http::new(url, &headers, inbox)?),
        Launch::App(link) => Arc::new(app::App::open(&*link.server, inbox)),
    })
}
