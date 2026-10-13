//! The three files of a live session, written as it runs.
//!
//! The feed journal (§9), the model's history, and the index the panel reads.
//! One object owns all three because they have to agree: a session whose index
//! says four messages and whose journal holds six is a panel lying about state
//! (§0).
//!
//! The index is written at turn boundaries rather than per event. A turn writes
//! hundreds of events, and rewriting a JSON file for each of them would be an
//! index maintained by the disk instead of by the session. What a crash costs
//! is one turn of the index, and the index is rebuildable from the journals —
//! see [`super::rebuild`].

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::event::AgentEvent;
use crate::id::TurnId;
use crate::message::Message;
use crate::session::binding::Fingerprint;
use crate::session::log::EventLog;

use super::history::{MessageLog, Record};
use super::meta::SessionMeta;
use super::transcript::Transcript;

pub struct Recorder {
    root: PathBuf,
    events: EventLog,
    messages: MessageLog,
    meta: Mutex<SessionMeta>,
}

impl Recorder {
    /// Opens the three files of a session, new or resumed. Everything is
    /// appended to, so resuming needs no separate path.
    pub fn open(root: &Path, meta: SessionMeta) -> std::io::Result<Self> {
        let events = EventLog::open(root, &meta.id)?;
        let messages = MessageLog::open(root, &meta.id)?;
        let recorder = Recorder {
            root: root.to_path_buf(),
            events,
            messages,
            meta: Mutex::new(meta),
        };
        // The panel must see the session the moment it starts, not once it
        // ends: a session that crashed on its first turn still happened.
        recorder.flush();
        Ok(recorder)
    }

    pub fn meta(&self) -> SessionMeta {
        self.meta.lock().unwrap().clone()
    }

    /// One event: into the journal, and into whatever the index takes from it.
    pub fn event(&self, event: &AgentEvent) {
        // Not journaled: the pending hunks and the panel's view of a file are
        // answers about what is on disk now. A reopened session has neither,
        // and the journal would take a whole file for every panel opened. The
        // same goes for the `@` search: a file list from some other day is not
        // a thing the conversation said. Nor is the state of the MCP servers:
        // a reopened session connects to them anew.
        if !matches!(
            event,
            AgentEvent::FileViewLoaded { .. }
                | AgentEvent::FilesFound { .. }
                | AgentEvent::McpUpdated { .. }
                | AgentEvent::LspUpdated { .. }
        ) {
            self.events.append(event);
        }

        let boundary = self.meta.lock().unwrap().absorb(event);
        if boundary {
            self.flush();
        }
    }

    /// What the human typed. The title comes from the first one (§3).
    pub fn prompt(&self, turn: &TurnId, text: &str) {
        self.messages.append(&Record::Prompt {
            turn: turn.clone(),
            text: text.to_string(),
        });
        self.meta.lock().unwrap().title_from_message(text);
    }

    /// §9: a name typed by hand, for the session while it runs. Written down at
    /// once rather than at the turn's end: the panel reads the file, and the
    /// human has just watched the name change.
    pub fn rename(&self, title: &str) -> std::io::Result<()> {
        let meta = {
            let mut meta = self.meta.lock().unwrap();
            meta.rename(title);
            meta.clone()
        };
        meta.save(&self.root)
    }

    pub fn message(&self, turn: &TurnId, message: &Message) {
        self.messages.append(&Record::Message {
            turn: turn.clone(),
            message: message.clone(),
        });
    }

    /// The conversation as it was said, for the tools that look back into it.
    pub fn transcript(&self) -> Transcript {
        Transcript::new(&self.root, self.meta.lock().unwrap().id.clone())
    }

    /// Replaces the model's history wholesale — what compaction does (§5.11).
    ///
    /// Appended rather than rewritten, and read back as a replacement: the
    /// journal is append-only so that a crash cannot leave it truncated, and a
    /// session that was compacted reopens compacted.
    pub fn replaced_history(&self, turn: &TurnId, history: &[Message]) {
        self.messages.append(&Record::Reset {
            turn: turn.clone(),
            kept: history.len(),
        });
        for message in history {
            self.message(turn, message);
        }
    }

    /// Requests go out under a new prefix from here on (`session::binding`).
    pub fn prefix(&self, turn: &TurnId, fingerprint: &Fingerprint) {
        self.messages.append(&Record::Prefix {
            turn: turn.clone(),
            fingerprint: fingerprint.clone(),
        });
    }

    /// A failed index write is not worth a turn: the journals hold everything,
    /// and the index is rebuilt from them when it is missing or behind.
    pub fn flush(&self) {
        let meta = self.meta.lock().unwrap().clone();
        let _ = meta.save(&self.root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::{ChangeKind, FileDiff, FileFact};
    use crate::event::{StopReason, TsMillis};
    use crate::id::SessionId;
    use crate::testutil::TempDir;
    use crate::usage::{TokenCounts, UsageSnapshot};

    fn recorder(dir: &TempDir) -> Recorder {
        let meta = SessionMeta::new(SessionId::from("ses_rec"), "anthropic".into(), "claude-opus-5".into());
        Recorder::open(dir.path(), meta).unwrap()
    }

    fn staged(path: &str) -> AgentEvent {
        AgentEvent::FileChanged {
            call_id: crate::id::CallId::from("c1"),
            fact: FileFact {
                path: path.into(),
                kind: ChangeKind::Modified,
                moved_from: None,
                added: 2,
                removed: 1,
            },
            diff: FileDiff::default(),
        }
    }

    #[test]
    fn the_index_exists_from_the_first_moment() {
        let dir = TempDir::new();
        let r = recorder(&dir);
        // A session that crashed on its first turn still happened, and the
        // panel has to be able to say so.
        assert!(SessionMeta::load(dir.path(), &r.meta().id).is_some());
    }

    #[test]
    fn a_turn_fills_the_index_and_lands_it_on_disk() {
        let dir = TempDir::new();
        let r = recorder(&dir);

        r.prompt(&TurnId::from("t1"), "почини разбор дат\nвот трейс");
        r.event(&AgentEvent::UserMessage {
            turn_id: TurnId::from("t1"),
            text: "почини разбор дат\nвот трейс".into(),
            attachments: Vec::new(),
        });
        r.event(&AgentEvent::TurnStarted {
            turn_id: TurnId::from("t1"),
            at: TsMillis(1_700_000_000_000),
        });
        r.event(&staged("src/a.rs"));
        r.event(&staged("src/a.rs"));
        r.event(&AgentEvent::UsageUpdated {
            usage: UsageSnapshot {
                total: TokenCounts {
                    input: 100,
                    output: 20,
                    ..Default::default()
                },
                cost_usd: 0.31,
                ..Default::default()
            },
        });
        r.event(&AgentEvent::TurnFinished {
            turn_id: TurnId::from("t1"),
            stop_reason: StopReason::EndTurn,
            at: Default::default(),
        });

        let saved = SessionMeta::load(dir.path(), &r.meta().id).unwrap();
        assert_eq!(saved.title, "почини разбор дат");
        assert_eq!(saved.messages, 1);
        assert_eq!(saved.files, vec!["src/a.rs"]);
        assert_eq!(saved.cost_usd, 0.31);
        assert_eq!(saved.tokens.input, 100);
        assert_eq!(saved.updated_at, TsMillis(1_700_000_000_000));
    }
}
