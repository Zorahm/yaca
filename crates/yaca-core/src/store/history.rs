//! The model's side of a session: `<id>.messages.jsonl`.
//!
//! The feed journal is not enough to reopen a session, and the difference is
//! not a detail. Events carry what the human sees; the model saw something
//! else. A `bash` result reaches the feed whole and the model head-and-tail
//! within a byte budget, and a thinking block carries a `signature` that never
//! appears in the feed at all and that the API rejects a conversation for
//! losing. Rebuilding history from events would produce a plausible
//! conversation that the provider refuses, or worse, accepts.
//!
//! So the messages are written as they are built, exactly as they go out.
//!
//! Alongside them goes what the human actually typed. The message the model
//! receives may have an attachment list folded into it (§7.2), and the session
//! title comes from the message, not from the fold — §9 searches the same text.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::id::{SessionId, TurnId};
use crate::message::{strip_thinking, ContentBlock, Message};
use crate::session::binding::Fingerprint;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Record {
    /// What the human typed, before attachments were folded in.
    #[serde(rename_all = "camelCase")]
    Prompt { turn: TurnId, text: String },
    /// A message exactly as it goes to the provider.
    #[serde(rename_all = "camelCase")]
    Message { turn: TurnId, message: Message },
    /// Everything before this line is no longer the conversation: history was
    /// replaced whole, which is what compaction does (§5.11).
    ///
    /// A marker rather than a rewrite. The file is append-only so that a crash
    /// cannot leave it truncated, and what the human said before the summary is
    /// still theirs to search (§9).
    ///
    /// `kept` is how many of the message lines that follow are the history the
    /// session went on with, written out again: the summary and the turns it
    /// left as they were. They are the conversation for the provider, and
    /// copies for anyone numbering what was said (`store::transcript`). Zero
    /// in a journal written before the count was.
    #[serde(rename_all = "camelCase")]
    Reset {
        turn: TurnId,
        #[serde(default)]
        kept: usize,
    },
    /// Requests go out under a different prefix from here on, and the
    /// reasoning in the messages above was bound to the old one
    /// (`session::binding`). Replaying makes the cut the live session made.
    #[serde(rename_all = "camelCase")]
    Prefix {
        turn: TurnId,
        fingerprint: Fingerprint,
    },
}

impl Record {
    pub fn turn(&self) -> &TurnId {
        match self {
            Record::Prompt { turn, .. }
            | Record::Message { turn, .. }
            | Record::Reset { turn, .. }
            | Record::Prefix { turn, .. } => turn,
        }
    }
}

pub struct MessageLog {
    file: Mutex<File>,
}

impl MessageLog {
    /// `dir` is the project's folder of journals ([`super::location`]).
    pub fn open(dir: &Path, id: &SessionId) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(MessageLog::path(dir, id))?;
        Ok(MessageLog {
            file: Mutex::new(file),
        })
    }

    pub fn path(dir: &Path, id: &SessionId) -> PathBuf {
        dir.join(format!("{id}.messages.jsonl"))
    }

    /// A failed write does not bring the turn down, the same as the feed
    /// journal. It does cost the session its future: what is not written
    /// cannot be reopened, which is why this is the only place it can fail
    /// quietly and the panel shows a shorter session rather than a wrong one.
    pub fn append(&self, record: &Record) {
        let Ok(line) = serde_json::to_string(record) else {
            return;
        };
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// Reads the session back. Lines that do not parse are skipped: the message
/// format will change, and an old session should reopen even if not whole.
pub fn replay(path: &Path) -> std::io::Result<Vec<Record>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let mut record: Record = serde_json::from_str(l).ok()?;
            canonicalize_record(&mut record);
            Some(record)
        })
        .collect())
}

fn canonicalize_record(record: &mut Record) {
    if let Record::Message { message, .. } = record {
        for block in &mut message.content {
            if let ContentBlock::ToolUse { name, .. } = block {
                *name = crate::legacy::canonical_tool_name(name).to_string();
            }
        }
    }
}

/// The conversation as the provider needs it: messages only, and only those
/// after the last reset — a compacted session reopens compacted.
pub fn messages(records: Vec<Record>) -> Vec<Message> {
    let start = records
        .iter()
        .rposition(|r| matches!(r, Record::Reset { .. }))
        .map_or(0, |i| i + 1);

    let mut out = Vec::new();
    for record in records.into_iter().skip(start) {
        match record {
            Record::Message { message, .. } => out.push(message),
            // The same function the live session cut with, or the history
            // reopens different from the one later reasoning was bound to.
            Record::Prefix { .. } => strip_thinking(&mut out),
            Record::Prompt { .. } | Record::Reset { .. } => {}
        }
    }
    out
}

/// The prefix the session last sent under. `None` for a session recorded
/// before prefixes were written down.
///
/// Looked for past the last reset as well: compaction replaces messages, not
/// the prompt or the tools.
pub fn prefix(records: &[Record]) -> Option<Fingerprint> {
    records.iter().rev().find_map(|r| match r {
        Record::Prefix { fingerprint, .. } => Some(fingerprint.clone()),
        _ => None,
    })
}

/// Everything up to and including `turn` — the cut "duplicate to point"
/// makes (§9).
pub fn through(records: Vec<Record>, turn: &TurnId) -> Vec<Record> {
    let mut out = Vec::new();
    let mut past = false;
    for record in records {
        if past && record.turn() != turn {
            break;
        }
        past = record.turn() == turn;
        out.push(record);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::ContentBlock;
    use crate::testutil::TempDir;

    fn prompt(turn: &str, text: &str) -> Record {
        Record::Prompt {
            turn: TurnId::from(turn),
            text: text.into(),
        }
    }

    fn reply(turn: &str, text: &str) -> Record {
        Record::Message {
            turn: TurnId::from(turn),
            message: Message::assistant(vec![ContentBlock::Text { text: text.into() }]),
        }
    }

    #[test]
    fn a_thinking_signature_survives_the_round_trip() {
        let dir = TempDir::new();
        let id = SessionId::from("ses_t");
        let log = MessageLog::open(dir.path(), &id).unwrap();

        // The API rejects a conversation whose thinking block lost its
        // signature, so this is the field the whole file exists for.
        log.append(&Record::Message {
            turn: TurnId::from("t1"),
            message: Message::assistant(vec![ContentBlock::Thinking {
                thinking: "pondering".into(),
                signature: Some("sig-abc".into()),
            }]),
        });

        let back = replay(&MessageLog::path(dir.path(), &id)).unwrap();
        let Record::Message { message, .. } = &back[0] else {
            panic!("not a message: {:?}", back[0]);
        };
        assert_eq!(
            message.content[0],
            ContentBlock::Thinking {
                thinking: "pondering".into(),
                signature: Some("sig-abc".into()),
            }
        );
    }

    #[test]
    fn the_provider_gets_messages_and_not_prompts() {
        let records = vec![prompt("t1", "what is here"), reply("t1", "here is what")];
        assert_eq!(messages(records).len(), 1);
    }

    #[test]
    fn a_compacted_session_reopens_compacted() {
        let records = vec![
            prompt("t1", "one"),
            reply("t1", "long conversation"),
            Record::Reset {
                turn: TurnId::from("t2"),
                kept: 1,
            },
            reply("t2", "summary"),
        ];
        // What the human said before the summary is still theirs to search,
        // but the provider must not see the history the summary replaced.
        let back = messages(records);
        assert_eq!(back.len(), 1);
        assert_eq!(
            back[0].content[0],
            ContentBlock::Text {
                text: "summary".into()
            }
        );
    }

    fn reasoned(turn: &str, thought: &str, text: &str) -> Record {
        Record::Message {
            turn: TurnId::from(turn),
            message: Message::assistant(vec![
                ContentBlock::Thinking {
                    thinking: thought.into(),
                    signature: Some("sig".into()),
                },
                ContentBlock::Text { text: text.into() },
            ]),
        }
    }

    /// Opaque outside `session::binding`, so built the way a journal line is.
    fn fingerprint(value: &str) -> Fingerprint {
        serde_json::from_value(serde_json::json!(value)).unwrap()
    }

    fn prefix_marker(turn: &str, value: &str) -> Record {
        Record::Prefix {
            turn: TurnId::from(turn),
            fingerprint: fingerprint(value),
        }
    }

    fn thoughts(messages: &[Message]) -> Vec<&str> {
        messages
            .iter()
            .flat_map(|m| &m.content)
            .filter_map(|b| match b {
                ContentBlock::Thinking { thinking, .. } => Some(thinking.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn reasoning_above_a_prefix_change_does_not_come_back() {
        let records = vec![
            prefix_marker("t1", "aaa"),
            reasoned("t1", "under aaa", "one"),
            prefix_marker("t2", "bbb"),
            reasoned("t2", "under bbb", "two"),
        ];
        let back = messages(records);
        // The live session cut at the marker; the reopened one must match it.
        assert_eq!(thoughts(&back), ["under bbb"]);
        assert_eq!(back.len(), 2, "the answers themselves stay");
    }

    #[test]
    fn the_last_prefix_is_found_past_a_reset() {
        let records = vec![
            prefix_marker("t1", "aaa"),
            reply("t1", "long"),
            Record::Reset {
                turn: TurnId::from("t2"),
                kept: 1,
            },
            reply("t2", "summary"),
        ];
        assert_eq!(prefix(&records), Some(fingerprint("aaa")));
        assert_eq!(prefix(&[reply("t1", "old session")]), None);
    }

    #[test]
    fn cutting_at_a_turn_keeps_that_turn_whole() {
        let records = vec![
            prompt("t1", "one"),
            reply("t1", "first"),
            prompt("t2", "two"),
            reply("t2", "second"),
            prompt("t3", "three"),
        ];
        let cut = through(records, &TurnId::from("t2"));
        assert_eq!(cut.len(), 4);
        assert_eq!(cut.last().unwrap().turn().as_str(), "t2");
    }

    #[test]
    fn an_unreadable_line_does_not_lose_the_rest() {
        let dir = TempDir::new();
        let p = dir.write(
            "s.messages.jsonl",
            "{\"kind\":\"prompt\",\"turn\":\"t1\",\"text\":\"one\"}\ngarbage\n",
        );
        assert_eq!(replay(&p).unwrap().len(), 1);
    }

    #[test]
    fn old_tool_use_name_is_canonicalized_on_replay() {
        let dir = TempDir::new();
        let p = dir.write(
            "legacy.messages.jsonl",
            r#"{"kind":"message","turn":"t1","message":{"role":"assistant","content":[{"kind":"tool_use","id":"c1","name":"read_file","input":{}}]}}"#,
        );
        let records = replay(&p).unwrap();
        assert_eq!(records.len(), 1);
        if let Record::Message { message, .. } = &records[0] {
            if let ContentBlock::ToolUse { name, .. } = &message.content[0] {
                assert_eq!(name, "Read");
            } else {
                panic!("expected ToolUse");
            }
        } else {
            panic!("expected Message");
        }
    }
}
