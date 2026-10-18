//! The sessions of one project (§9).
//!
//! The project keeps its settings, permission rules and shadow repository
//! under its own `.yaca/`; its history lives in the application's chats folder
//! instead — see [`location`] for where and why. Three files per session, all
//! of them append-only:
//!
//! ```text
//! <chats>/<project>-<hash>/<id>.jsonl           the feed, event per line
//! <chats>/<project>-<hash>/<id>.messages.jsonl  the model's history, as it was sent
//! <chats>/<project>-<hash>/<id>.meta.json       the index the panel reads
//! ```
//!
//! Only the third one is derived, and it is derived on purpose: an index that
//! cannot be deleted and rebuilt is a second version of the truth. Delete the
//! `meta` files of a project and the panel comes back the same, one rebuild
//! slower — see [`rebuild`].
//!
//! This is a plain API and not a [`Command`](crate::Command). The command
//! channel talks to a *running* session; renaming another one, searching the
//! project or deleting last week's work is not something the current turn has
//! an opinion about. §15's list stays what it is, and the shell reaches the
//! panel through here.

pub mod export;
pub mod history;
pub mod location;
pub mod meta;
pub mod recorder;
pub mod replay;
pub mod search;
pub mod transcript;

use std::path::{Path, PathBuf};

use crate::error::CoreError;
use crate::event::AgentEvent;
use crate::id::{SessionId, TurnId};
use crate::message::Message;
use crate::provider::registry::ANTHROPIC_ID;
use crate::session::log;

pub use history::Record;
pub use location::chats_dir;
pub use meta::SessionMeta;
pub use recorder::Recorder;
pub use search::{Match, MatchKind, SearchHit};

pub struct SessionStore {
    /// The project's own folder under the chats folder, where its journals
    /// are — not the project.
    root: PathBuf,
    /// The project itself, whose shadow store the saved checkpoint lists are
    /// checked against. A store made over a folder of journals alone has none.
    project: Option<PathBuf>,
}

impl SessionStore {
    /// The sessions `project` has under the chats folder `chats`
    /// ([`chats_dir`] in the applications, a temporary folder in tests).
    pub fn open(chats: &Path, project: &Path) -> Self {
        SessionStore {
            project: Some(project.to_path_buf()),
            ..SessionStore::at(location::project_dir(chats, project))
        }
    }

    /// The store over a folder of journals already located.
    pub(crate) fn at(dir: PathBuf) -> Self {
        SessionStore {
            root: dir,
            project: None,
        }
    }

    /// Every session of the project, most recently touched first.
    ///
    /// Grouping by day is the panel's job: a day is a local-calendar fact, and
    /// the interface is the side that knows the viewer's timezone.
    pub fn list(&self) -> Vec<SessionMeta> {
        let mut out: Vec<SessionMeta> = self
            .ids()
            .into_iter()
            .filter_map(|id| self.meta(&id))
            .collect();
        out.sort_by_key(|m| std::cmp::Reverse(m.updated_at));
        out
    }

    /// The index of one session, rebuilt from the journals if it is missing.
    pub fn meta(&self, id: &SessionId) -> Option<SessionMeta> {
        SessionMeta::load(&self.root, id).or_else(|| rebuild(&self.root, id))
    }

    pub fn records(&self, id: &SessionId) -> Vec<Record> {
        history::replay(&history::MessageLog::path(&self.root, id)).unwrap_or_default()
    }

    /// The conversation as a resumed session needs it.
    pub fn history(&self, id: &SessionId) -> Vec<Message> {
        history::messages(self.records(id))
    }

    pub fn events(&self, id: &SessionId) -> Vec<AgentEvent> {
        log::replay(&events_path(&self.root, id)).unwrap_or_default()
    }

    /// The feed as a reopened session may draw it — the human's side of
    /// [`SessionStore::history`]. See [`replay`] for what a replay leaves out.
    ///
    /// The session's saved checkpoints come with it: they are what the reopened
    /// session will hold, so its dividers are the ones that can be rolled back
    /// to.
    pub fn feed(&self, id: &SessionId) -> Vec<AgentEvent> {
        let held = crate::checkpoint::ledger::alive(
            &crate::checkpoint::ledger::path(&self.root, id),
            self.project.as_deref(),
        );
        replay::with_checkpoints(self.events(id), &held)
    }

    /// §9: rename. The new name sticks — a rebuilt index will not take it back.
    ///
    /// For a session that is not running. A running one keeps its index in
    /// memory and writes it whole at the end of a turn, over whatever is put
    /// here: it is renamed through `SessionHandle::rename`.
    pub fn rename(&self, id: &SessionId, title: &str) -> Result<(), CoreError> {
        let mut meta = self
            .meta(id)
            .ok_or_else(|| CoreError::Config(format!("session {id} not found")))?;
        meta.rename(title);
        meta.save(&self.root)?;
        Ok(())
    }

    /// §9: delete. All the files, and no complaint about the ones that were
    /// already gone — the goal was that the session not be there.
    ///
    /// The saved checkpoint list goes with them, which is what frees its
    /// snapshots: the next sweep no longer finds anything that names them.
    pub fn delete(&self, id: &SessionId) -> Result<(), CoreError> {
        for path in [
            events_path(&self.root, id),
            history::MessageLog::path(&self.root, id),
            SessionMeta::path(&self.root, id),
            crate::checkpoint::ledger::path(&self.root, id),
        ] {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(CoreError::Io(e)),
            }
        }
        // The drafts `ExtractMessage` left in the project: they belong to the
        // conversation they were cut from, and nothing else reads them.
        if let Some(drafts) = self.drafts_dir() {
            let _ = std::fs::remove_dir_all(drafts.join(id.to_string()));
        }
        Ok(())
    }

    /// The project's `.yaca/drafts`, where each session has a folder.
    fn drafts_dir(&self) -> Option<PathBuf> {
        Some(crate::project::dir(self.project.as_ref()?).join("drafts"))
    }

    /// Removes the draft folders of sessions that are gone: deleted before
    /// drafts went with them, or deleted from another copy of the app.
    /// A folder that does not look like a session's is left alone.
    pub fn sweep_drafts(&self) {
        let Some(drafts) = self.drafts_dir() else { return };
        let Ok(entries) = std::fs::read_dir(&drafts) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("ses_") {
                continue;
            }
            let id = SessionId::from(name);
            if entry.path().is_dir() && !events_path(&self.root, &id).exists() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    /// §9: duplicate, optionally up to a point.
    ///
    /// A copy, not a branch: the new session owns its files and the old one is
    /// untouched. `through` names the last turn to keep — everything the human
    /// said and the model answered after it is left behind, in both journals at
    /// once so that the feed and the conversation stay the same length.
    pub fn duplicate(
        &self,
        id: &SessionId,
        through: Option<&TurnId>,
    ) -> Result<SessionMeta, CoreError> {
        let source = self
            .meta(id)
            .ok_or_else(|| CoreError::Config(format!("session {id} not found")))?;

        let mut records = self.records(id);
        let mut events = self.events(id);
        if let Some(turn) = through {
            records = history::through(records, turn);
            events = events_through(events, turn);
        }

        let copy = SessionId::new();
        write_lines(
            &events_path(&self.root, &copy),
            events.iter().filter_map(|e| serde_json::to_string(e).ok()),
        )?;
        write_lines(
            &history::MessageLog::path(&self.root, &copy),
            records.iter().filter_map(|r| serde_json::to_string(r).ok()),
        )?;

        let mut meta = fold(copy, &source.model, &events, &records);
        // The title is the first message, and the first message came along.
        // Marking the copy is the panel's business, not a suffix glued on here.
        meta.title = source.title.clone();
        meta.title_custom = source.title_custom;
        meta.title_by_model = source.title_by_model;
        meta.forked_from = Some(source.id);
        meta.save(&self.root)?;
        Ok(meta)
    }

    /// §9: export to Markdown. Returns the document; where it is saved is the
    /// caller's question, and a core that opened a file dialog would be a core
    /// that grew a window.
    pub fn export_markdown(&self, id: &SessionId) -> Result<String, CoreError> {
        let meta = self
            .meta(id)
            .ok_or_else(|| CoreError::Config(format!("session {id} not found")))?;
        Ok(export::markdown(&meta, &self.records(id)))
    }

    /// §9: search by message text and by the paths a session touched.
    ///
    /// The index answers title and paths without opening anything. A session
    /// the index cannot answer for pays one read of its message log — which is
    /// why the cheap half is tried first, and why an empty query is not a
    /// search at all.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let Some(needle) = search::Needle::new(query) else {
            return Vec::new();
        };

        let mut out = Vec::new();
        for session in self.list() {
            if out.len() >= limit {
                break;
            }
            let mut matches = search::in_meta(&session, &needle);
            let room = search::MATCHES_PER_SESSION.saturating_sub(matches.len());
            if room > 0 {
                matches.extend(search::in_messages(
                    &self.records(&session.id),
                    &needle,
                    room,
                ));
            }
            if !matches.is_empty() {
                out.push(SearchHit { session, matches });
            }
        }
        out
    }

    /// Session ids the project has journals for.
    fn ids(&self) -> Vec<SessionId> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        entries
            .filter_map(|e| e.ok())
            .filter_map(|e| session_id_of(&e.file_name().to_string_lossy()))
            .collect()
    }
}

/// The index of a session, read back out of its journals.
///
/// This is what makes the sidecar disposable, and it is not a repair tool: a
/// session whose last turn was cut short by a crash comes back through here
/// with everything the journals hold.
pub fn rebuild(root: &Path, id: &SessionId) -> Option<SessionMeta> {
    let events = log::replay(&events_path(root, id)).unwrap_or_default();
    let records = history::replay(&history::MessageLog::path(root, id)).unwrap_or_default();
    if events.is_empty() && records.is_empty() {
        return None;
    }
    Some(fold(
        id.clone(),
        crate::usage::DEFAULT_MODEL,
        &events,
        &records,
    ))
}

/// The index as a function of the two journals — the one definition of what
/// the panel shows, shared by the live recorder and by a rebuild.
fn fold(id: SessionId, model: &str, events: &[AgentEvent], records: &[Record]) -> SessionMeta {
    let mut meta = SessionMeta::new(id, ANTHROPIC_ID.to_string(), model.to_string());
    for event in events {
        meta.absorb(event);
    }
    if let Some(Record::Prompt { text, .. }) =
        records.iter().find(|r| matches!(r, Record::Prompt { .. }))
    {
        meta.title_from_message(text);
    }
    if let Some(AgentEvent::TurnStarted { at, .. }) = events
        .iter()
        .find(|e| matches!(e, AgentEvent::TurnStarted { .. }))
    {
        meta.created_at = *at;
    }
    meta
}

/// `<id>.jsonl`, but not `<id>.messages.jsonl`.
fn session_id_of(file_name: &str) -> Option<SessionId> {
    let stem = file_name.strip_suffix(".jsonl")?;
    if stem.is_empty() || stem.ends_with(".messages") {
        return None;
    }
    Some(SessionId::from(stem))
}

fn events_path(root: &Path, id: &SessionId) -> PathBuf {
    root.join(format!("{id}.jsonl"))
}

/// The feed up to the end of `turn`. Cut at the same place the conversation is,
/// or the copy would show work its history does not remember doing.
fn events_through(events: Vec<AgentEvent>, turn: &TurnId) -> Vec<AgentEvent> {
    let end = events.iter().position(|e| match e {
        AgentEvent::TurnFinished { turn_id, .. } | AgentEvent::TurnCancelled { turn_id, .. } => {
            turn_id == turn
        }
        _ => false,
    });
    match end {
        Some(at) => events.into_iter().take(at + 1).collect(),
        // The turn never ended — a crash, or a session still running. Keeping
        // everything is the honest answer: there is no later turn to drop.
        None => events,
    }
}

fn write_lines(path: &Path, lines: impl Iterator<Item = String>) -> Result<(), CoreError> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path)?;
    for line in lines {
        writeln!(file, "{line}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{StopReason, TsMillis};
    use crate::message::ContentBlock;
    use crate::testutil::TempDir;

    /// Writes a session the way a run would have left it.
    fn write_session(dir: &TempDir, id: &str, prompt: &str, turns: &[&str]) -> SessionId {
        let id = SessionId::from(id);
        let meta = SessionMeta::new(id.clone(), "anthropic".into(), "claude-opus-5".into());
        let recorder = Recorder::open(dir.path(), meta).unwrap();

        for (index, turn) in turns.iter().enumerate() {
            let turn_id = TurnId::from(*turn);
            let text = if index == 0 {
                prompt.to_string()
            } else {
                format!("{prompt} again {index}")
            };
            recorder.prompt(&turn_id, &text);
            recorder.event(&AgentEvent::UserMessage {
                turn_id: turn_id.clone(),
                text: text.clone(),
                attachments: Vec::new(),
            });
            recorder.event(&AgentEvent::TurnStarted {
                turn_id: turn_id.clone(),
                at: TsMillis(1_700_000_000_000 + index as u64 * 1000),
            });
            recorder.message(&turn_id, &Message::user_text(text));
            recorder.message(
                &turn_id,
                &Message::assistant(vec![ContentBlock::Text {
                    text: format!("answer {index}"),
                }]),
            );
            recorder.event(&AgentEvent::TurnFinished {
                turn_id,
                stop_reason: StopReason::EndTurn,
                at: Default::default(),
            });
        }
        id
    }

    fn store(dir: &TempDir) -> SessionStore {
        SessionStore::at(dir.path().to_path_buf())
    }

    #[test]
    fn the_list_is_newest_first() {
        let dir = TempDir::new();
        write_session(&dir, "ses_a", "first", &["t1"]);
        write_session(&dir, "ses_b", "second", &["t1", "t2"]);

        let listed = store(&dir).list();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id.as_str(), "ses_b");
        assert_eq!(listed[0].title, "second");
        assert_eq!(listed[0].messages, 2);
    }

    #[test]
    fn the_index_can_be_deleted_and_comes_back() {
        let dir = TempDir::new();
        let id = write_session(&dir, "ses_a", "date parsing", &["t1"]);
        std::fs::remove_file(SessionMeta::path(dir.path(), &id)).unwrap();

        // An index that cannot be thrown away is a second version of the truth.
        let meta = store(&dir).meta(&id).unwrap();
        assert_eq!(meta.title, "date parsing");
        assert_eq!(meta.messages, 1);
    }

    #[test]
    fn the_message_log_is_not_mistaken_for_a_session() {
        assert_eq!(session_id_of("ses_a.jsonl").unwrap().as_str(), "ses_a");
        assert!(session_id_of("ses_a.messages.jsonl").is_none());
        assert!(session_id_of("ses_a.meta.json").is_none());
    }

    #[test]
    fn renaming_survives_a_rebuild() {
        let dir = TempDir::new();
        let id = write_session(&dir, "ses_a", "date parsing", &["t1"]);
        let s = store(&dir);
        s.rename(&id, "Dates and zones").unwrap();

        assert_eq!(s.meta(&id).unwrap().title, "Dates and zones");
        // The name is the human's now; a rebuild must not take it back.
        assert!(s.meta(&id).unwrap().title_custom);
    }

    #[test]
    fn deleting_takes_all_the_files() {
        let dir = TempDir::new();
        let id = write_session(&dir, "ses_a", "parsing", &["t1"]);
        let ledger = crate::checkpoint::ledger::path(dir.path(), &id);
        std::fs::write(&ledger, r#"{"points":[]}"#).unwrap();
        store(&dir).delete(&id).unwrap();

        assert!(store(&dir).list().is_empty());
        assert!(!events_path(dir.path(), &id).exists());
        assert!(!history::MessageLog::path(dir.path(), &id).exists());
        assert!(!ledger.exists(), "its snapshots must be free to go");
        // And deleting again is not an error: it is already gone.
        store(&dir).delete(&id).unwrap();
    }

    #[test]
    fn duplicating_to_a_point_cuts_both_journals_at_once() {
        let dir = TempDir::new();
        let id = write_session(&dir, "ses_a", "parsing", &["t1", "t2", "t3"]);
        let s = store(&dir);

        let copy = s.duplicate(&id, Some(&TurnId::from("t2"))).unwrap();
        assert_eq!(copy.forked_from.as_ref(), Some(&id));
        assert_eq!(copy.messages, 2, "two turns, third cut off");
        // The feed and the conversation must be the same length, or the copy
        // shows work its history does not remember doing.
        assert_eq!(s.history(&copy.id).len(), 4);
        assert!(s.events(&copy.id).iter().all(
            |e| !matches!(e, AgentEvent::TurnStarted { turn_id, .. } if turn_id.as_str() == "t3")
        ));

        // The original is untouched.
        assert_eq!(s.meta(&id).unwrap().messages, 3);
    }

    #[test]
    fn duplicating_whole_copies_everything() {
        let dir = TempDir::new();
        let id = write_session(&dir, "ses_a", "parsing", &["t1", "t2"]);
        let s = store(&dir);

        let copy = s.duplicate(&id, None).unwrap();
        assert_eq!(copy.messages, 2);
        assert_eq!(s.history(&copy.id).len(), s.history(&id).len());
    }

    #[test]
    fn search_finds_a_session_by_a_path_it_touched() {
        let dir = TempDir::new();
        let id = write_session(&dir, "ses_a", "parsing", &["t1"]);
        let s = store(&dir);
        let mut meta = s.meta(&id).unwrap();
        meta.note_file("src/segments.ts");
        meta.save(dir.path()).unwrap();

        let hits = s.search("segments", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matches[0].kind, MatchKind::Path);
    }

    #[test]
    fn search_reaches_into_the_conversation() {
        let dir = TempDir::new();
        write_session(&dir, "ses_a", "why date parsing fails", &["t1"]);
        write_session(&dir, "ses_b", "completely different", &["t1"]);

        let hits = store(&dir).search("answer 0", 10);
        assert_eq!(hits.len(), 2, "both sessions answered like this");

        let one = store(&dir).search("fails", 10);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].session.id.as_str(), "ses_a");
    }

    #[test]
    fn an_empty_query_returns_nothing_rather_than_everything() {
        let dir = TempDir::new();
        write_session(&dir, "ses_a", "parsing", &["t1"]);
        assert!(store(&dir).search("   ", 10).is_empty());
    }

    #[test]
    fn exporting_produces_the_conversation() {
        let dir = TempDir::new();
        let id = write_session(&dir, "ses_a", "fix date parsing", &["t1"]);
        let md = store(&dir).export_markdown(&id).unwrap();
        assert!(md.starts_with("# fix date parsing"));
        assert!(md.contains("answer 0"));
    }

    #[test]
    fn a_project_without_sessions_is_an_empty_list_and_not_an_error() {
        let dir = TempDir::new();
        assert!(store(&dir).list().is_empty());
        assert!(store(&dir).search("anything", 10).is_empty());
    }
}
