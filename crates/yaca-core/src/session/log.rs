//! Session log: the event stream as it is, one event per line.
//!
//! These are not debug logs but the history store (§9). The interface is a pure
//! function of the event stream, so the recorded stream is the session itself:
//! to reopen it, it is enough to replay the file.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::event::AgentEvent;
use crate::id::SessionId;

pub struct EventLog {
    file: Mutex<File>,
    path: PathBuf,
}

impl EventLog {
    /// Opens `<dir>/<id>.jsonl` for appending, `dir` being the project's
    /// folder of journals (`store::location`).
    pub fn open(dir: &Path, id: &SessionId) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{id}.jsonl"));
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(EventLog {
            file: Mutex::new(file),
            path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many bytes the journal holds now. Every event is one whole line, so
    /// a length read between two appends is a boundary between events.
    pub fn len(&self) -> u64 {
        self.file
            .lock()
            .ok()
            .and_then(|file| file.metadata().ok())
            .map_or(0, |meta| meta.len())
    }

    /// A write failure must not bring the turn down: the log is useful, but not critical.
    pub fn append(&self, event: &AgentEvent) {
        let Ok(line) = serde_json::to_string(event) else {
            return;
        };
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// Replays a recorded session. Lines that did not parse are skipped:
/// the event format will change, and an old session should still open, even if
/// though not in full.
pub fn replay(path: &Path) -> std::io::Result<Vec<AgentEvent>> {
    let text = std::fs::read_to_string(path)?;
    Ok(parse(&text))
}

/// The first `len` bytes of a journal — the events up to a boundary taken from
/// [`EventLog::len`]. The journal is append-only, so those bytes do not change
/// while a session keeps writing past them.
pub fn replay_prefix(path: &Path, len: u64) -> std::io::Result<Vec<AgentEvent>> {
    use std::io::Read;

    let mut bytes = Vec::new();
    File::open(path)?.take(len).read_to_end(&mut bytes)?;
    Ok(parse(&String::from_utf8_lossy(&bytes)))
}

fn parse(text: &str) -> Vec<AgentEvent> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let mut event: AgentEvent = serde_json::from_str(l).ok()?;
            canonicalize_event(&mut event);
            Some(event)
        })
        .collect()
}

fn canonicalize_event(event: &mut AgentEvent) {
    match event {
        AgentEvent::ToolCallStarted { tool, .. } => {
            *tool = crate::legacy::canonical_tool_name(tool).to_string();
        }
        AgentEvent::PermissionRequested { request } => {
            for scope in &mut request.scopes {
                if let crate::permission::ScopeKey::Tool { name } = scope {
                    *name = crate::legacy::canonical_tool_name(name).to_string();
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::StopReason;
    use crate::id::TurnId;
    use crate::testutil::TempDir;

    #[test]
    fn recorded_events_are_read_back() {
        let dir = TempDir::new();
        let id = SessionId::from("ses_test");
        let log = EventLog::open(dir.path(), &id).unwrap();

        log.append(&AgentEvent::TurnStarted {
            turn_id: TurnId::from("t1"),
            at: crate::event::TsMillis(1),
        });
        log.append(&AgentEvent::TurnFinished {
            turn_id: TurnId::from("t1"),
            stop_reason: StopReason::EndTurn,
            at: Default::default(),
        });

        let back = replay(log.path()).unwrap();
        assert_eq!(back.len(), 2);
        assert!(matches!(back[1], AgentEvent::TurnFinished { .. }));
    }

    #[test]
    fn a_prefix_ends_where_the_length_said_and_not_after() {
        let dir = TempDir::new();
        let id = SessionId::from("ses_test");
        let log = EventLog::open(dir.path(), &id).unwrap();
        let started = AgentEvent::TurnStarted {
            turn_id: TurnId::from("t1"),
            at: crate::event::TsMillis(1),
        };

        log.append(&started);
        let boundary = log.len();
        log.append(&AgentEvent::TurnFinished {
            turn_id: TurnId::from("t1"),
            stop_reason: StopReason::EndTurn,
            at: Default::default(),
        });

        // The session wrote on after the boundary was taken; the prefix is
        // still exactly what was there.
        assert_eq!(replay_prefix(log.path(), boundary).unwrap(), vec![started]);
        assert_eq!(replay(log.path()).unwrap().len(), 2);
        assert_eq!(replay_prefix(log.path(), 0).unwrap(), vec![]);
    }

    #[test]
    fn unreadable_line_does_not_break_the_rest() {
        let dir = TempDir::new();
        let p = dir.write(
            "s.jsonl",
            "{\"type\":\"turnCancelled\",\"turnId\":\"t\"}\ngarbage\n",
        );
        let back = replay(&p).unwrap();
        assert_eq!(back.len(), 1);
    }

    #[test]
    fn old_tool_call_is_canonicalized_on_replay() {
        let dir = TempDir::new();
        let p = dir.write(
            "legacy.jsonl",
            r#"{"type":"toolCallStarted","turnId":"t1","callId":"c1","tool":"read_file","input":{}}"#,
        );
        let events = replay(&p).unwrap();
        assert_eq!(events.len(), 1);
        if let AgentEvent::ToolCallStarted { tool, .. } = &events[0] {
            assert_eq!(tool, "Read");
        } else {
            panic!("expected ToolCallStarted");
        }
    }
}
