//! What goes over the socket.
//!
//! JSON text frames, one message each, tagged by `t`. The calls are the host's
//! own: a command name and the object of arguments the interface passes to
//! `invoke`. The events are the host's too, on the channel names the desktop
//! uses, so the interface cannot tell the two transports apart.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Client to server.
#[derive(Debug, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Up {
    /// The first message, and the only one accepted before the connection is
    /// authorized. `client` is a label for the log ("android", "desktop").
    Auth {
        token: String,
        #[serde(default)]
        client: String,
    },
    Call {
        id: u64,
        cmd: String,
        #[serde(default)]
        args: Value,
    },
    /// Keeps the connection alive across NATs that forget a quiet socket, and
    /// tells the client a dead one from a silent one.
    Ping,
}

/// Server to client.
#[derive(Debug, Serialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Down {
    Ready {
        version: &'static str,
    },
    Result {
        id: u64,
        ok: bool,
        /// The answer, when `ok`.
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        /// The text to show, when not.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Event {
        channel: String,
        payload: Value,
    },
    Pong,
    /// The server is closing the connection and says why, so the client can
    /// tell "wrong token" and "another device took over" from a dropped line.
    Bye {
        reason: Bye,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bye {
    Unauthorized,
    /// Another client authorized; one window at a time owns the chats' streams.
    Replaced,
    /// The client read too slowly and the queue to it filled up. It reconnects
    /// and catches up from the journal, as after any dropped line.
    TooSlow,
    /// Revoked while connected.
    Revoked,
}

impl Down {
    pub fn result(id: u64, answer: Result<Value, String>) -> Down {
        match answer {
            Ok(value) => Down::Result {
                id,
                ok: true,
                value: Some(value),
                error: None,
            },
            Err(error) => Down::Result {
                id,
                ok: false,
                value: None,
                error: Some(error),
            },
        }
    }

    pub fn text(&self) -> String {
        // Plain data; a failure here would be a bug in the types above.
        serde_json::to_string(self).unwrap_or_else(|_| "{\"t\":\"pong\"}".into())
    }
}
