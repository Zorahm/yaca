//! Keeping replayed thinking valid when the request prefix moves.
//!
//! A thinking block's signature binds it to the conversation that produced it:
//! the system prompt, the tool set and every message before it. Claude Fable
//! 5.1 answers a request that replays a block under a changed prefix with a
//! 400 — on accounts created from 2026-08-31, and later models are expected to
//! do so for everyone — and a retry sends the same body and fails the same way.
//!
//! Yaca's prefix does move. The prompt and the tools follow the mode (plan has
//! neither the writing tools nor the text about them), the prompt embeds the
//! project root and `AGENTS.md`, and an update rewrites both. So before each
//! request the prefix is fingerprinted, and when it differs from the one the
//! history's reasoning was produced under, every thinking block goes. All of
//! them predate the change, which makes them a leading run — the one removal
//! the API accepts — and the blocks produced from then on stay valid.
//!
//! Only on a change, not on every resume: a session reopened with nothing moved
//! keeps its reasoning, and where something did move, the prompt cache went
//! with it, so the drop costs no cache. The API's own escape,
//! `prefix_mismatch_behavior: "drop_block"`, leaves the stale blocks in the
//! history and drops every block after the first stale one on every request —
//! the new ones included, for the rest of the session. Its beta header is also
//! not something every Anthropic-shaped proxy accepts.
//!
//! The cut is written to the journal where it happened, so a reopened session
//! makes the same cut at the same place (`store::history::messages`).
//! Compaction moves the prefix of the turns it keeps the same way, and cuts
//! their reasoning itself (`compact::rebuilt`).

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::id::TurnId;
use crate::message::strip_thinking;
use crate::provider::ToolSchema;

use super::turn::Worker;

/// The part of a request's prefix that is not the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fingerprint(String);

pub fn fingerprint(system: &str, tools: &[ToolSchema]) -> Fingerprint {
    let mut tools: Vec<_> = tools
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
            })
        })
        .collect();
    // The API binds the tools as a set, so their order is not a change.
    tools.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));

    let mut hasher = blake3::Hasher::new();
    hasher.update(system.as_bytes());
    hasher.update(&[0]);
    hasher.update(serde_json::Value::Array(tools).to_string().as_bytes());
    Fingerprint(hasher.finalize().to_hex().to_string())
}

impl Worker {
    /// Called with the prefix a request is about to go out under.
    pub(super) fn rebind(&mut self, turn_id: &TurnId, fingerprint: Fingerprint) {
        if self.prefix.as_ref() == Some(&fingerprint) {
            return;
        }
        // An unknown prefix counts as a change. A new session has nothing to
        // strip, and a session recorded before prefixes were written down went
        // out under one nobody can check — a 400 ends it, lost reasoning does
        // not.
        strip_thinking(&mut self.history);
        if let Some(recorder) = &self.shared.recorder {
            recorder.prefix(turn_id, &fingerprint);
        }
        self.prefix = Some(fingerprint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, description: &str) -> ToolSchema {
        ToolSchema {
            name: name.into(),
            description: description.into(),
            input_schema: json!({ "type": "object" }),
        }
    }

    #[test]
    fn the_same_prefix_gives_the_same_fingerprint() {
        let tools = [tool("Read", "Reads a file.")];
        assert_eq!(fingerprint("prompt", &tools), fingerprint("prompt", &tools));
    }

    #[test]
    fn the_prompt_and_each_tool_are_part_of_it() {
        let tools = [tool("Read", "Reads a file.")];
        let base = fingerprint("prompt", &tools);
        assert_ne!(base, fingerprint("prompt, edited", &tools));
        assert_ne!(base, fingerprint("prompt", &[tool("Read", "Reads it.")]));
        assert_ne!(base, fingerprint("prompt", &[]));
    }

    #[test]
    fn tool_order_is_not_a_change() {
        let a = tool("a", "A.");
        let b = tool("b", "B.");
        assert_eq!(
            fingerprint("p", &[a.clone(), b.clone()]),
            fingerprint("p", &[b, a])
        );
    }
}
