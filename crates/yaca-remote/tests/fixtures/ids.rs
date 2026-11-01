//! Fixed identifiers and the one instant every fixture is built on. The files
//! are committed, so nothing in the fixtures may come from a clock or a random
//! source: a run that changes a file has changed a Rust type, and nothing else.

use yaca_core::event::TsMillis;
use yaca_core::id::{BlockId, CallId, CheckpointId, QueuedId, RequestId, SessionId, TurnId};

/// The project name every chat and summary of the fixtures carries.
pub const PROJECT: &str = "yaca-demo";

/// The base instant of every timestamp in the fixtures, in epoch milliseconds.
pub const BASE_MS: u64 = 1_767_000_000_000;

pub fn at(offset_ms: u64) -> TsMillis {
    TsMillis(BASE_MS + offset_ms)
}

pub fn session(n: u32) -> SessionId {
    format!("s-{n}").into()
}

pub fn turn(n: u32) -> TurnId {
    format!("turn-{n}").into()
}

pub fn block(n: u32) -> BlockId {
    format!("blk-{n}").into()
}

pub fn call(n: u32) -> CallId {
    format!("call-{n}").into()
}

pub fn request(n: u32) -> RequestId {
    format!("req-{n}").into()
}

pub fn checkpoint(n: u32) -> CheckpointId {
    format!("ckpt-{n}").into()
}

pub fn queued(n: u32) -> QueuedId {
    format!("qmsg-{n}").into()
}
