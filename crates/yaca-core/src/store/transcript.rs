//! The conversation as it was said, numbered.
//!
//! The message log is written for the provider: it holds tool results, the
//! reasoning with its signatures, and after a compaction a second copy of every
//! turn the summary left in place. What the two sides said to each other is a
//! smaller thing, and it is what `SearchMessage` reads — what the human typed,
//! and the text of what the agent answered.
//!
//! A message's number is its place in that list. The log is only ever appended
//! to, so a number, once given, names the same message for as long as the
//! session lives: across a compaction, and across closing and reopening it.

use std::path::{Path, PathBuf};

use super::history::{self, MessageLog, Record};
use crate::id::SessionId;
use crate::message::{ContentBlock, Role};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// From 1, in the order things were said.
    pub number: usize,
    pub role: Role,
    pub text: String,
}

/// What was said, from the records of a session.
pub fn entries(records: &[Record]) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let mut say = |role: Role, text: String| {
        let number = out.len() + 1;
        out.push(Entry { number, role, text });
    };
    // Message lines still to pass over: the history a compaction wrote out
    // again (`Record::Reset`).
    let mut copies = 0usize;

    for record in records {
        match record {
            // What the human typed, not the message built from it: that one
            // may have a list of attachments folded in.
            Record::Prompt { text, .. } => say(Role::User, text.clone()),
            Record::Reset { kept, .. } => copies = *kept,
            Record::Message { .. } if copies > 0 => copies -= 1,
            Record::Message { message, .. } if message.role == Role::Assistant => {
                let text = message
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } if !text.trim().is_empty() => {
                            Some(text.as_str())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                // A response that was calls alone said nothing.
                if !text.is_empty() {
                    say(Role::Assistant, text);
                }
            }
            // The human's side of the log is tool results and the summary.
            Record::Message { .. } | Record::Prefix { .. } => {}
        }
    }
    out
}

/// A session's conversation, read from its journal when asked.
///
/// A handle rather than the list: a tool call late in a turn must see what was
/// said earlier in that same turn, and the log is where that is.
#[derive(Debug, Clone)]
pub struct Transcript {
    log: PathBuf,
    session: SessionId,
}

impl Transcript {
    pub fn new(root: &Path, session: SessionId) -> Self {
        Transcript {
            log: MessageLog::path(root, &session),
            session,
        }
    }

    pub fn session(&self) -> &SessionId {
        &self.session
    }

    pub fn entries(&self) -> std::io::Result<Vec<Entry>> {
        Ok(entries(&history::replay(&self.log)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::TurnId;
    use crate::message::Message;

    fn prompt(text: &str) -> Record {
        Record::Prompt {
            turn: TurnId::from("t"),
            text: text.into(),
        }
    }

    fn message(message: Message) -> Record {
        Record::Message {
            turn: TurnId::from("t"),
            message,
        }
    }

    fn reply(text: &str) -> Record {
        message(Message::assistant(vec![ContentBlock::Text { text: text.into() }]))
    }

    fn said(records: &[Record]) -> Vec<(usize, Role, String)> {
        entries(records)
            .into_iter()
            .map(|e| (e.number, e.role, e.text))
            .collect()
    }

    #[test]
    fn both_sides_are_numbered_in_the_order_they_spoke() {
        let records = [prompt("question"), message(Message::user_text("question")), reply("answer")];
        assert_eq!(
            said(&records),
            vec![
                (1, Role::User, "question".to_string()),
                (2, Role::Assistant, "answer".to_string()),
            ]
        );
    }

    #[test]
    fn calls_results_and_reasoning_are_not_messages() {
        let records = [
            prompt("read"),
            message(Message::assistant(vec![
                ContentBlock::Thinking {
                    thinking: "need to read".into(),
                    signature: None,
                },
                ContentBlock::ToolUse {
                    id: "c1".into(),
                    name: "Read".into(),
                    input: serde_json::json!({ "path": "a.rs" }),
                },
            ])),
            message(Message::tool_results(vec![ContentBlock::ToolResult {
                tool_use_id: "c1".into(),
                content: "fn main() {}".into(),
                is_error: false,
            }])),
            reply("read it"),
        ];
        assert_eq!(
            said(&records),
            vec![
                (1, Role::User, "read".to_string()),
                (2, Role::Assistant, "read it".to_string()),
            ]
        );
    }

    #[test]
    fn the_text_around_a_call_is_one_message() {
        let records = [message(Message::assistant(vec![
            ContentBlock::Text { text: "Looking first.".into() },
            ContentBlock::ToolUse {
                id: "c1".into(),
                name: "Read".into(),
                input: serde_json::json!({}),
            },
            ContentBlock::Text { text: "I see.".into() },
        ]))];
        assert_eq!(
            said(&records),
            vec![(1, Role::Assistant, "Looking first.\n\nI see.".to_string())]
        );
    }

    #[test]
    fn the_history_a_compaction_writes_again_gets_no_new_numbers() {
        let records = [
            prompt("one"),
            reply("answer one"),
            prompt("two"),
            reply("answer two"),
            Record::Reset {
                turn: TurnId::from("t"),
                kept: 3,
            },
            // The summary, then the turn that was kept.
            message(Message::user_text("summary")),
            message(Message::user_text("two")),
            reply("answer two"),
            prompt("three"),
            reply("answer three"),
        ];
        let numbers: Vec<_> = said(&records).into_iter().map(|(n, _, text)| (n, text)).collect();
        assert_eq!(
            numbers,
            vec![
                (1, "one".to_string()),
                (2, "answer one".to_string()),
                (3, "two".to_string()),
                (4, "answer two".to_string()),
                (5, "three".to_string()),
                (6, "answer three".to_string()),
            ]
        );
    }
}
