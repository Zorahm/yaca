//! Identifiers. Newtypes over a string: they cannot be mixed up in Rust, and in TS
//! they are a plain `string`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident, $prefix:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
        #[ts(export)]
        pub struct $name(pub String);

        impl $name {
            /// A fresh identifier with a readable prefix — the log shows what it is.
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, uuid::Uuid::new_v4().simple()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// Session — one conversation in the project.
    SessionId,
    "ses"
);
string_id!(
    /// Turn — one run of the agent from the human's message to its stop.
    TurnId,
    "turn"
);
string_id!(
    /// A block of text or reasoning inside the model's answer.
    BlockId,
    "blk"
);
string_id!(
    /// A tool call. Matches the provider's `tool_use.id` when the provider gives one.
    CallId,
    "call"
);
string_id!(
    /// A permission request (§5.7). Not used in slice 1, but part of the contract.
    RequestId,
    "req"
);
string_id!(
    /// Checkpoint (§5.8).
    CheckpointId,
    "ckpt"
);
string_id!(
    /// A message the human sent during a turn that has not yet been taken into
    /// it (§7.4). Needed to cancel exactly this message.
    QueuedId,
    "qmsg"
);

/// Milliseconds since the epoch. A plain `number` in TS; fits within 2^53 until the 287th millennium.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
