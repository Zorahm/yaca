//! Background commands and the process panel (§6.5).
//!
//! The registry lives in the core, not in the interface, and holds the ring
//! buffer itself. Ten thousand lines is a bound on memory, not a rule about
//! drawing, and the core is the only place that knows whether a command is
//! still running — so the panel is a function of what the core sends, the way
//! the change bar is.

pub mod kill;
pub mod output;
pub mod shell;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::event::{AgentEvent, EventSink, TsMillis};
use output::{Ring, RING_LINES};

/// Number of a background command inside the session (§6.5).
///
/// Small and sequential rather than a uuid: it is shown to the human in the
/// panel and travels to the model in the tool result, and `3` reads better and
/// costs less than `proc_1f2c9a…`. Uniqueness is only ever needed inside one
/// session, which is exactly the registry's lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProcessId(#[ts(type = "number")] pub u32);

impl std::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The `состояние` column of §6.5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProcessState {
    Running,
    /// Ended on its own. `code` is `None` when a signal ended it.
    #[serde(rename_all = "camelCase")]
    Exited {
        code: Option<i32>,
    },
    /// Ended by the panel's "Stop" button.
    Stopped,
}

/// One row of the process panel (§6.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSummary {
    pub id: ProcessId,
    pub command: String,
    /// Working directory, relative to the project root.
    pub cwd: String,
    pub state: ProcessState,
    pub started_at: TsMillis,
    /// `None` while running. The `время работы` column is the difference
    /// between these two, counted by the interface — a core that ticked a
    /// clock event every second would say nothing new each time.
    pub finished_at: Option<TsMillis>,
    /// Lines currently in the buffer.
    pub lines: u32,
    /// Lines pushed out of it.
    #[ts(type = "number")]
    pub dropped: u64,
}

/// What `bash_output` hands the model.
pub struct ProcessRead {
    pub state: ProcessState,
    pub text: String,
    /// Lines the buffer dropped before the model got to them.
    pub missed: u64,
    pub lines: u32,
    /// First read since the human pressed "Stop" on this row. The model is
    /// told to stop rather than handed the output, and the turn ends there.
    ///
    /// True exactly once: on every later read the process is simply a finished
    /// one. Otherwise a command stopped this morning would end every turn that
    /// glanced at it for the rest of the day.
    pub stopped_by_human: bool,
}

struct Entry {
    command: String,
    cwd: String,
    state: ProcessState,
    started_at: TsMillis,
    finished_at: Option<TsMillis>,
    ring: Ring,
    pid: Option<u32>,
    /// Set by "Stop" before the kill lands, so the waiter reports `Stopped`
    /// rather than the exit code that killing produced.
    stopping: bool,
    /// Whether the model has already been told this row was stopped by hand.
    stop_reported: bool,
    /// How far the model has read. Absolute line index, so it survives the
    /// buffer dropping lines underneath it.
    cursor: u64,
}

impl Entry {
    fn summary(&self, id: ProcessId) -> ProcessSummary {
        ProcessSummary {
            id,
            command: self.command.clone(),
            cwd: self.cwd.clone(),
            state: self.state.clone(),
            started_at: self.started_at,
            finished_at: self.finished_at,
            lines: self.ring.len() as u32,
            dropped: self.ring.dropped(),
        }
    }
}

struct Inner {
    events: EventSink,
    next: AtomicU32,
    table: Mutex<BTreeMap<ProcessId, Entry>>,
    /// The row selected in the panel, if any.
    watched: Mutex<Option<ProcessId>>,
}

/// The session's background commands. Cloneable handle: the dispatcher answers
/// "Stop" while a turn is running, and the turn starts new ones.
#[derive(Clone)]
pub struct Processes(Arc<Inner>);

impl Processes {
    pub fn new(events: EventSink) -> Self {
        Processes(Arc::new(Inner {
            events,
            next: AtomicU32::new(1),
            table: Mutex::new(BTreeMap::new()),
            watched: Mutex::new(None),
        }))
    }

    /// A freshly spawned command takes its place in the panel.
    pub fn register(&self, command: String, cwd: String, pid: Option<u32>) -> ProcessId {
        let id = ProcessId(self.0.next.fetch_add(1, Ordering::Relaxed));
        self.with_table(|table| {
            table.insert(
                id,
                Entry {
                    command,
                    cwd,
                    state: ProcessState::Running,
                    started_at: TsMillis::now(),
                    finished_at: None,
                    ring: Ring::new(RING_LINES),
                    pid,
                    stopping: false,
                    stop_reported: false,
                    cursor: 0,
                },
            );
        });
        self.announce();
        id
    }

    /// Output arrived. It always reaches the buffer; it reaches the feed only
    /// for the row the human is looking at.
    pub fn output(&self, id: ProcessId, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }
        let mut chunk = String::new();
        self.with_table(|table| {
            let Some(entry) = table.get_mut(&id) else {
                return;
            };
            for line in lines {
                chunk.push_str(&line);
                chunk.push('\n');
                entry.ring.push(line);
            }
        });

        if *self.watched() == Some(id) {
            self.0.events.emit(AgentEvent::ProcessOutput { id, chunk });
        }
    }

    /// The command ended by itself.
    pub fn finished(&self, id: ProcessId, code: Option<i32>) {
        self.with_table(|table| {
            let Some(entry) = table.get_mut(&id) else {
                return;
            };
            entry.state = if entry.stopping {
                ProcessState::Stopped
            } else {
                ProcessState::Exited { code }
            };
            entry.finished_at = Some(TsMillis::now());
            entry.pid = None;
        });
        self.announce();
    }

    /// The panel's "Stop" (§6.5). Blocking on Windows, where killing a tree
    /// means running `taskkill`.
    pub fn stop(&self, id: ProcessId) {
        let pid = self.with_table(|table| {
            let entry = table.get_mut(&id)?;
            if entry.state != ProcessState::Running {
                return None;
            }
            entry.stopping = true;
            // Taken, not copied: a pid is only safe to signal once, and a
            // second "Stop" after the process died would hit whatever the OS
            // handed the number to next.
            //
            // Signalling a process that just ended is safe here for a reason
            // worth stating: the task collecting its output still holds the
            // `Child`, and neither Windows nor Unix reuses a number while a
            // handle to it is open. The entry loses its pid at the same moment
            // that task reaps it.
            entry.pid.take()
        });

        if let Some(pid) = pid {
            kill::tree(pid);
        }
        // The state itself is set when the waiter sees the process go, so the
        // panel never shows `stopped` for something still winding down.
    }

    /// A row was selected in the panel: its whole buffer goes out once, and
    /// live output follows from there (§6.5).
    pub fn watch(&self, id: Option<ProcessId>) {
        *self.watched() = id;
        let Some(id) = id else {
            return;
        };
        let Some((text, dropped)) =
            self.with_table(|table| table.get(&id).map(|e| (e.ring.text(), e.ring.dropped())))
        else {
            return;
        };
        self.0
            .events
            .emit(AgentEvent::ProcessOutputReset { id, text, dropped });
    }

    pub fn summaries(&self) -> Vec<ProcessSummary> {
        self.with_table(|table| table.iter().map(|(id, e)| e.summary(*id)).collect())
    }

    /// Everything printed since the model last asked (`bash_output`).
    pub fn read(&self, id: ProcessId, max_bytes: usize) -> Option<ProcessRead> {
        self.with_table(|table| {
            let entry = table.get_mut(&id)?;
            let (lines, missed) = entry.ring.since(entry.cursor);
            let count = lines.len() as u32;
            let (text, _truncated) = output::clamp(&lines, max_bytes);
            entry.cursor = entry.ring.next_index();
            let stopped_by_human = entry.state == ProcessState::Stopped
                && !std::mem::replace(&mut entry.stop_reported, true);
            Some(ProcessRead {
                state: entry.state.clone(),
                text,
                missed,
                lines: count,
                stopped_by_human,
            })
        })
    }

    pub fn is_empty(&self) -> bool {
        self.with_table(|table| table.is_empty())
    }

    pub fn downgrade(&self) -> WeakProcesses {
        WeakProcesses(Arc::downgrade(&self.0))
    }

    fn announce(&self) {
        let processes = self.summaries();
        self.0
            .events
            .emit(AgentEvent::ProcessesUpdated { processes });
    }

    /// A poisoned lock here would mean a panic inside one of these short
    /// bodies, none of which can leave the table half-written; losing the
    /// process panel over it would be the worse outcome.
    fn with_table<T>(&self, f: impl FnOnce(&mut BTreeMap<ProcessId, Entry>) -> T) -> T {
        let mut guard = self
            .0
            .table
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut guard)
    }

    fn watched(&self) -> std::sync::MutexGuard<'_, Option<ProcessId>> {
        self.0
            .watched
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// A registry handle that does not keep the session alive.
///
/// The task collecting a background command's output holds one of these. A
/// strong handle would keep the session's processes alive for as long as
/// `npm run dev` runs — that is, forever — and [`Inner::drop`], the thing that
/// kills them when the session ends, would never fire.
#[derive(Clone)]
pub struct WeakProcesses(std::sync::Weak<Inner>);

impl WeakProcesses {
    pub fn upgrade(&self) -> Option<Processes> {
        self.0.upgrade().map(Processes)
    }
}

impl Drop for Inner {
    /// A session that ends takes its background commands with it. Nothing
    /// started from a window that is gone should outlive it.
    fn drop(&mut self) {
        let table = self
            .table
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for entry in table.values_mut() {
            if let (ProcessState::Running, Some(pid)) = (&entry.state, entry.pid.take()) {
                kill::tree(pid);
            }
        }
    }
}

/// The working directory as the panel and the permission request show it:
/// relative to the project, absolute only when it lies outside.
pub fn display_cwd(root: &Path, cwd: &Path) -> String {
    match cwd.strip_prefix(root) {
        Ok(rel) if rel.as_os_str().is_empty() => ".".to_string(),
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => cwd.to_string_lossy().replace('\\', "/"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    fn registry() -> (Processes, broadcast::Receiver<AgentEvent>) {
        let (tx, rx) = broadcast::channel(64);
        (Processes::new(EventSink::new(tx)), rx)
    }

    fn drain(rx: &mut broadcast::Receiver<AgentEvent>) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            out.push(e);
        }
        out
    }

    #[test]
    fn registering_announces_the_whole_panel() {
        let (procs, mut rx) = registry();
        let id = procs.register("npm run dev".into(), ".".into(), None);

        let events = drain(&mut rx);
        let AgentEvent::ProcessesUpdated { processes } = &events[0] else {
            panic!("expected the panel, got {:?}", events[0]);
        };
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].id, id);
        assert_eq!(processes[0].state, ProcessState::Running);
        assert!(processes[0].finished_at.is_none());
    }

    #[test]
    fn output_of_an_unwatched_process_stays_in_its_buffer() {
        let (procs, mut rx) = registry();
        let id = procs.register("npm run dev".into(), ".".into(), None);
        drain(&mut rx);

        procs.output(id, vec!["ready".into()]);
        assert!(
            drain(&mut rx).is_empty(),
            "unwatched output must not reach the feed"
        );

        // Selecting the row shows what was collected while nobody looked.
        procs.watch(Some(id));
        let events = drain(&mut rx);
        assert!(matches!(
            &events[0],
            AgentEvent::ProcessOutputReset { text, .. } if text == "ready\n"
        ));

        procs.output(id, vec!["compiled".into()]);
        let events = drain(&mut rx);
        assert!(matches!(
            &events[0],
            AgentEvent::ProcessOutput { chunk, .. } if chunk == "compiled\n"
        ));
    }

    #[test]
    fn the_model_reads_each_line_once() {
        let (procs, _rx) = registry();
        let id = procs.register("npm test".into(), ".".into(), None);

        procs.output(id, vec!["one".into(), "two".into()]);
        let first = procs.read(id, 1000).unwrap();
        assert_eq!(first.text, "one\ntwo\n");
        assert_eq!(first.lines, 2);
        assert_eq!(first.missed, 0);

        // Nothing new: the model gets nothing rather than the same output twice.
        let second = procs.read(id, 1000).unwrap();
        assert_eq!(second.text, "");
        assert_eq!(second.lines, 0);

        procs.output(id, vec!["three".into()]);
        assert_eq!(procs.read(id, 1000).unwrap().text, "three\n");
    }

    #[test]
    fn stopping_before_the_waiter_reports_wins_over_the_exit_code() {
        let (procs, _rx) = registry();
        let id = procs.register("sleep 100".into(), ".".into(), None);

        procs.stop(id);
        // Killing produces an exit code of its own; the panel should say the
        // human stopped it, not that it exited with 137.
        procs.finished(id, Some(137));

        let summary = procs.summaries().pop().unwrap();
        assert_eq!(summary.state, ProcessState::Stopped);
        assert!(summary.finished_at.is_some());
    }

    #[test]
    fn a_process_that_ended_reports_its_code() {
        let (procs, _rx) = registry();
        let id = procs.register("cargo test".into(), ".".into(), None);
        procs.finished(id, Some(1));

        assert_eq!(
            procs.summaries()[0].state,
            ProcessState::Exited { code: Some(1) }
        );
    }

    #[test]
    fn the_working_directory_reads_as_a_project_path() {
        let root = Path::new("/p");
        assert_eq!(display_cwd(root, Path::new("/p")), ".");
        assert_eq!(
            display_cwd(root, Path::new("/p/crates/core")),
            "crates/core"
        );
        assert_eq!(display_cwd(root, Path::new("/other")), "/other");
    }
}
