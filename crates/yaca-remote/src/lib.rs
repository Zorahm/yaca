//! The remote server: how a phone reaches a Yaca that is running somewhere else.
//!
//! The phone is a client and nothing more. The agent works where the code is,
//! on the machine that hosts the sessions; the phone watches the feed, writes to
//! the chat and answers what the agent asks. So the server speaks the core's own
//! vocabulary (`Command` in, `AgentEvent` out) and adds only the shell's part:
//! which projects and chats exist, opening and attaching, and who may ask.
//!
//! Not in the core: the core knows nothing about who is looking at it, and a
//! listening socket is exactly that kind of knowledge. Two applications host it,
//! the desktop shell (the same chats the window shows) and `yaca serve` (no
//! window at all), each through its own [`Host`].
//!
//! Anyone who can talk to this server can run commands on the host, since that
//! is what the agent does. Hence TLS with a certificate the phone pins from the
//! pairing code, a device token checked before anything else is read, and the
//! server off unless the human turns it on.

pub mod addresses;
pub mod devices;
pub mod error;
mod files;
pub mod frame;
pub mod host;
pub mod pairing;
pub mod protocol;
mod secret;
pub mod server;
pub mod settings;
pub mod tls;

mod attach;
mod attention;
mod calls;
mod chats;
mod connection;
mod handshake;
mod limit;
mod reader;
mod shared;
mod writer;

pub use error::Error;
pub use host::{Host, LiveChat};
pub use protocol::PROTOCOL_VERSION;
pub use server::{RemoteServer, ServerOptions};
