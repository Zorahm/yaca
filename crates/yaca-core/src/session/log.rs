//! Журнал сессии: поток событий как есть, по событию на строку.
//!
//! Это не логи для отладки, а хранилище истории (§9). Интерфейс — чистая
//! функция от потока событий, поэтому записанный поток и есть сессия:
//! чтобы открыть её заново, достаточно проиграть файл.

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
    /// Открывает `<root>/.yaca/sessions/<id>.jsonl` на дозапись.
    pub fn open(root: &Path, id: &SessionId) -> std::io::Result<Self> {
        let dir = crate::store::dir(root);
        std::fs::create_dir_all(&dir)?;
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

    /// Сбой записи не должен ронять турн: журнал полезен, но не критичен.
    pub fn append(&self, event: &AgentEvent) {
        let Ok(line) = serde_json::to_string(event) else {
            return;
        };
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// Проиграть записанную сессию. Строки, которые не разобрались, пропускаются:
/// формат событий будет меняться, и старая сессия должна открываться, пусть
/// и не целиком.
pub fn replay(path: &Path) -> std::io::Result<Vec<AgentEvent>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let mut event: AgentEvent = serde_json::from_str(l).ok()?;
            canonicalize_event(&mut event);
            Some(event)
        })
        .collect())
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
    fn записанное_читается_обратно() {
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
        });

        let back = replay(log.path()).unwrap();
        assert_eq!(back.len(), 2);
        assert!(matches!(back[1], AgentEvent::TurnFinished { .. }));
    }

    #[test]
    fn нечитаемая_строка_не_ломает_остальные() {
        let dir = TempDir::new();
        let p = dir.write(
            "s.jsonl",
            "{\"type\":\"turnCancelled\",\"turnId\":\"t\"}\nмусор\n",
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
