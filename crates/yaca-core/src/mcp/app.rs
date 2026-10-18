//! Servers the application itself hands a session: no process, no port.
//!
//! The desktop window can do things the core cannot — show a web page, for
//! one — and offers them to the model as an MCP server of its own. It speaks
//! the same JSON-RPC as any other server, so its tools get everything a
//! server's tools get: the row in the feed, the question before a call that
//! changes something, the line in the MCP panel. The core never learns what
//! the server is about, and `yaca-cli` simply has none to give.
//!
//! The messages travel over a pair of channels instead of a pipe or HTTP.
//! Nothing outside the process can reach them, which is the point: a port on
//! `127.0.0.1` would let any local program drive what the window does.

use std::fmt;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;

use super::config::{McpServerSettings, McpSource, ServerSpec};

/// The server's end of one connection.
pub struct ServerEnd {
    /// The client's messages: requests, notifications, and answers to the
    /// server's own requests. Ends when the client lets go of the server.
    pub incoming: mpsc::UnboundedReceiver<Value>,
    /// The server's messages. Once it is dropped, the client takes the
    /// server for gone.
    pub outgoing: mpsc::UnboundedSender<Value>,
}

/// A server that lives in the application.
pub trait AppServer: Send + Sync {
    /// A connection begins, and `initialize` is the first thing to arrive on
    /// it. Called again with a fresh end on every reconnect.
    fn open(&self, end: ServerEnd);
}

/// One such server, as the application declares it to a session.
#[derive(Clone)]
pub struct AppServerSpec {
    /// The name in the tool names, `mcp__<name>__<tool>`, and in the panel.
    pub name: String,
    /// What the MCP panel shows where other servers have a command line.
    pub target: String,
    pub server: Arc<dyn AppServer>,
}

impl fmt::Debug for AppServerSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppServerSpec")
            .field("name", &self.name)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

/// What ties a declaration to its server.
///
/// Compared by identity: two links are the same only when they lead to the
/// same object, which is all "the same declaration" can mean for something
/// that is not written in a file.
#[derive(Clone)]
pub struct AppLink {
    pub(crate) target: String,
    pub(crate) server: Arc<dyn AppServer>,
}

impl PartialEq for AppLink {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.server, &other.server)
    }
}

impl Eq for AppLink {}

impl fmt::Debug for AppLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppLink").field("target", &self.target).finish_non_exhaustive()
    }
}

/// The application's servers as session declarations.
pub(crate) fn specs(servers: &[AppServerSpec]) -> Vec<ServerSpec> {
    servers
        .iter()
        .map(|app| ServerSpec {
            name: app.name.clone(),
            source: McpSource::App,
            // Timeouts and `disabled` keep their defaults: there is no file
            // for the human to have set them in.
            settings: McpServerSettings::default(),
            plugin: None,
            app: Some(AppLink {
                target: app.target.clone(),
                server: app.server.clone(),
            }),
        })
        .collect()
}
