//! What the session panel shows about a session (§9).
//!
//! A sidecar next to the journals, `<id>.meta.json`, kept current as the
//! session runs. It holds nothing the journals do not: title, times, model,
//! cost, and the paths the session changed. That is deliberate — the sidecar is
//! an index, and an index that cannot be thrown away and rebuilt is a second
//! version of the truth waiting to disagree with the first.
//!
//! Times go out as milliseconds and no further. §9 groups the list by day, and
//! a day is a local-calendar fact: the interface has a calendar that knows the
//! viewer's timezone, and the core does not.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::event::TsMillis;
use crate::id::SessionId;
use crate::usage::TokenCounts;

/// How long a generated title may be before it is cut (§3).
pub const TITLE_MAX_CHARS: usize = 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub id: SessionId,
    /// Generated from the first message (§3), unless renamed.
    pub title: String,
    /// Renamed by hand. The first message no longer decides the title, and a
    /// rebuilt index must not quietly take the name back.
    #[serde(default)]
    pub title_custom: bool,
    pub created_at: TsMillis,
    pub updated_at: TsMillis,
    #[serde(default)]
    pub provider: String,
    pub model: String,
    /// Messages the human sent. The panel's second line, with time and cost.
    #[serde(default)]
    pub messages: u32,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub tokens: TokenCounts,
    /// Paths the session changed, relative to the project root.
    ///
    /// Changed, not read: §9 searches "по путям затронутых файлов", and a
    /// session that grepped forty files touched none of them.
    #[serde(default)]
    pub files: Vec<String>,
    /// The session this one was duplicated from (§9).
    #[serde(default)]
    pub forked_from: Option<SessionId>,
}

impl SessionMeta {
    pub fn new(id: SessionId, provider: String, model: String) -> Self {
        let now = TsMillis::now();
        SessionMeta {
            id,
            title: String::new(),
            title_custom: false,
            created_at: now,
            updated_at: now,
            provider,
            model,
            messages: 0,
            cost_usd: 0.0,
            tokens: TokenCounts::default(),
            files: Vec::new(),
            forked_from: None,
        }
    }

    pub fn path(root: &Path, id: &SessionId) -> PathBuf {
        super::dir(root).join(format!("{id}.meta.json"))
    }

    pub fn load(root: &Path, id: &SessionId) -> Option<SessionMeta> {
        let text = std::fs::read_to_string(SessionMeta::path(root, id)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Written aside and renamed: a half-written index read by the panel is
    /// worse than an index that is one turn behind.
    pub fn save(&self, root: &Path) -> std::io::Result<()> {
        let dir = super::dir(root);
        std::fs::create_dir_all(&dir)?;
        let target = SessionMeta::path(root, &self.id);
        let temp = dir.join(format!("{}.meta.json.tmp", self.id));
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::other(format!("не собрать meta: {e}")))?;
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, &target)
    }

    /// Records the first message as the title, unless a human has named it.
    pub fn title_from_message(&mut self, text: &str) {
        if self.title_custom || !self.title.is_empty() {
            return;
        }
        self.title = title_of(text);
    }

    pub fn note_file(&mut self, path: &str) {
        if !self.files.iter().any(|p| p == path) {
            self.files.push(path.to_string());
        }
    }

    /// Takes from one event whatever the index keeps, and says whether the
    /// event ended a turn — the moment the index is worth writing down.
    ///
    /// Lives here rather than in the recorder because the index is rebuilt
    /// from the journals too (`super::rebuild`), and two folds over the same
    /// event stream would drift apart on the first field added.
    pub fn absorb(&mut self, event: &crate::event::AgentEvent) -> bool {
        use crate::event::AgentEvent as E;
        match event {
            // Counted by what the human said rather than by turns: a message
            // sent while the agent worked joins the running turn (§7.4)
            // instead of opening one.
            E::UserMessage { .. } => self.messages += 1,
            E::TurnStarted { at, .. } => self.updated_at = *at,
            E::UsageUpdated { usage } => {
                self.tokens = usage.total;
                self.cost_usd = usage.cost_usd;
            }
            E::FileChanged { fact, .. } => self.note_file(&fact.path),
            E::StateChanged { state } => {
                self.provider = state.provider.clone();
                self.model = state.model.clone();
            }
            _ => {}
        }
        matches!(event, E::TurnFinished { .. } | E::TurnCancelled { .. })
    }
}

/// The first line of the message, trimmed to fit the header (§3).
///
/// The first line and not the first sentence: a task usually opens with one,
/// and a paragraph that wraps at sixty characters would be cut mid-word
/// wherever the sentence happened to be.
pub fn title_of(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");

    if line.chars().count() <= TITLE_MAX_CHARS {
        return line.to_string();
    }

    let cut: String = line.chars().take(TITLE_MAX_CHARS - 1).collect();
    // Back off to the last word boundary, unless that leaves almost nothing.
    let trimmed = match cut.rsplit_once(char::is_whitespace) {
        Some((head, _)) if head.chars().count() >= TITLE_MAX_CHARS / 2 => head.to_string(),
        _ => cut,
    };
    format!("{}…", trimmed.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn meta() -> SessionMeta {
        SessionMeta::new(SessionId::from("ses_test"), "anthropic".into(), "claude-opus-5".into())
    }

    #[test]
    fn a_message_that_joined_a_running_turn_is_counted_too() {
        use crate::event::{AgentEvent, StopReason};
        use crate::id::TurnId;

        let turn = TurnId::from("t1");
        let said = |text: &str| AgentEvent::UserMessage {
            turn_id: turn.clone(),
            text: text.into(),
            attachments: Vec::new(),
        };
        let mut m = meta();
        for event in [
            said("create notes.txt"),
            AgentEvent::TurnStarted {
                turn_id: turn.clone(),
                at: TsMillis(1),
            },
            // Sent while the agent worked, and folded into its turn (§7.4).
            said("and add a test"),
            AgentEvent::TurnFinished {
                turn_id: turn.clone(),
                stop_reason: StopReason::EndTurn,
            },
        ] {
            m.absorb(&event);
        }
        assert_eq!(m.messages, 2);
    }

    #[test]
    fn a_title_is_the_first_line_of_the_first_message() {
        let mut m = meta();
        m.title_from_message("почини разбор дат\n\nвот трейс:\n...");
        assert_eq!(m.title, "почини разбор дат");
    }

    #[test]
    fn a_long_first_line_is_cut_at_a_word() {
        let long = "перенеси типографику на токены приложения и заодно проверь \
                    контраст во всех четырёх акцентах";
        assert!(title_of(long).chars().count() <= TITLE_MAX_CHARS);
        assert!(title_of(long).ends_with('…'));
        assert!(!title_of(long).contains("акцент"));
    }

    #[test]
    fn a_renamed_session_keeps_its_name() {
        let mut m = meta();
        m.title = "разбор дат".into();
        m.title_custom = true;
        m.title_from_message("совсем про другое");
        // A rebuilt index must not quietly take the name back.
        assert_eq!(m.title, "разбор дат");
    }

    #[test]
    fn the_same_file_is_listed_once() {
        let mut m = meta();
        m.note_file("src/a.rs");
        m.note_file("src/a.rs");
        m.note_file("src/b.rs");
        assert_eq!(m.files, vec!["src/a.rs", "src/b.rs"]);
    }

    #[test]
    fn round_trip_through_disk() {
        let dir = TempDir::new();
        let mut m = meta();
        m.title = "разбор дат".into();
        m.cost_usd = 0.31;
        m.note_file("src/a.rs");
        m.save(dir.path()).unwrap();

        assert_eq!(SessionMeta::load(dir.path(), &m.id).unwrap(), m);
    }

    #[test]
    fn nothing_is_left_half_written() {
        let dir = TempDir::new();
        meta().save(dir.path()).unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(super::super::dir(dir.path()))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }
}
