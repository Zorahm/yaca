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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
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
    /// The model itself ended this command with `BashKill`. It already knows —
    /// the state line says so, and the turn goes on.
    pub stopped_by_model: bool,
}

/// Who ended a command that was stopped rather than left to finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopper {
    Human,
    Model,
}

/// A background command ended on its own while the model was not looking.
///
/// Held until the worker can hand it over: between a batch of calls and the
/// model's next step, or ahead of the next message of the human's. The model
/// is not woken for it — a turn costs tokens the human did not ask to spend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitNotice {
    pub id: ProcessId,
    pub command: String,
    pub code: Option<i32>,
    /// Where the whole output went, for a command that outgrew the foreground
    /// (see [`Processes::adopt`]). The model is pointed at it instead of at
    /// the buffer, which keeps only the last ten thousand lines.
    pub file: Option<PathBuf>,
}

struct Entry {
    command: String,
    cwd: String,
    state: ProcessState,
    started_at: TsMillis,
    finished_at: Option<TsMillis>,
    ring: Ring,
    pid: Option<u32>,
    /// Set by a stop before the kill lands, so the waiter reports `Stopped`
    /// rather than the exit code that killing produced. First stopper wins.
    stopper: Option<Stopper>,
    /// Whether the model has already been told this row was stopped by hand.
    stop_reported: bool,
    /// How far the model has read. Absolute line index, so it survives the
    /// buffer dropping lines underneath it.
    cursor: u64,
    /// The file the whole output is being written to, if there is one.
    file: Option<PathBuf>,
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
    /// Commands that ended by themselves and have not been reported to the
    /// model yet. At most one per command.
    notices: Mutex<Vec<ExitNotice>>,
    /// Bumped whenever a command ends, so a tool can wait for a stop to land
    /// instead of polling the table.
    ended: watch::Sender<u64>,
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
            notices: Mutex::new(Vec::new()),
            ended: watch::channel(0).0,
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
                    stopper: None,
                    stop_reported: false,
                    cursor: 0,
                    file: None,
                },
            );
        });
        self.announce();
        id
    }

    /// A command that started in the foreground and outgrew it takes its place
    /// in the panel with the output it has printed so far.
    ///
    /// `seen` is what the model has already been given in the result of the
    /// call that ran it, so the read cursor starts past it: a `BashOutput`
    /// that handed the same lines over again would read as the command
    /// repeating itself.
    pub fn adopt(
        &self,
        command: String,
        cwd: String,
        pid: Option<u32>,
        seen: Vec<String>,
    ) -> ProcessId {
        let id = self.register(command, cwd, pid);
        self.with_table(|table| {
            if let Some(entry) = table.get_mut(&id) {
                for line in seen {
                    entry.ring.push(line);
                }
                entry.cursor = entry.ring.next_index();
            }
        });
        self.announce();
        id
    }

    /// Names the file the command's whole output is written to. Set before the
    /// collector starts, so no ending can be reported without it.
    pub fn set_output_file(&self, id: ProcessId, file: PathBuf) {
        self.with_table(|table| {
            if let Some(entry) = table.get_mut(&id) {
                entry.file = Some(file);
            }
        });
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

    /// The command ended: by itself, or because somebody stopped it.
    pub fn finished(&self, id: ProcessId, code: Option<i32>) {
        let notice = self.with_table(|table| {
            let entry = table.get_mut(&id)?;
            entry.state = match entry.stopper {
                Some(_) => ProcessState::Stopped,
                None => ProcessState::Exited { code },
            };
            entry.finished_at = Some(TsMillis::now());
            entry.pid = None;
            // Only an ending nobody asked for is news. A stop was the stopper's
            // own act, and telling the model what it just did is noise.
            entry.stopper.is_none().then(|| ExitNotice {
                id,
                command: entry.command.clone(),
                code,
                file: entry.file.clone(),
            })
        });
        if let Some(notice) = notice {
            let mut notices = self.notices();
            notices.retain(|n| n.id != id);
            notices.push(notice);
        }
        self.0.ended.send_modify(|n| *n += 1);
        self.announce();
    }

    /// The panel's "Stop" (§6.5). Blocking on Windows, where killing a tree
    /// means running `taskkill`.
    pub fn stop(&self, id: ProcessId) {
        self.stop_by(id, Stopper::Human);
    }

    /// `BashKill`: the same kill, but the model is not told off for it.
    pub fn kill(&self, id: ProcessId) {
        self.stop_by(id, Stopper::Model);
    }

    fn stop_by(&self, id: ProcessId, by: Stopper) {
        let pid = self.with_table(|table| {
            let entry = table.get_mut(&id)?;
            if entry.state != ProcessState::Running {
                return None;
            }
            entry.stopper.get_or_insert(by);
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

    /// Waits until the command is no longer running. `false` — it still is.
    pub async fn wait_ended(&self, id: ProcessId, limit: Duration) -> bool {
        // Subscribed before the check: an ending between the two is seen as a
        // change rather than slept through.
        let mut rx = self.0.ended.subscribe();
        let waiting = async {
            while self
                .summary(id)
                .is_some_and(|s| s.state == ProcessState::Running)
            {
                if rx.changed().await.is_err() {
                    break;
                }
            }
        };
        tokio::time::timeout(limit, waiting).await.is_ok()
    }

    /// Puts a finished command back in the panel under its old number, running
    /// again. The buffer is kept — what the old run printed and the model has
    /// not read yet is exactly what explains why it was restarted — and a
    /// marker line tells the two runs apart.
    ///
    /// Same number rather than a new row: the model that said "restart 3"
    /// should not have to learn that 3 is now 4, and the panel should not
    /// collect a dead row per restart.
    pub fn restart(&self, id: ProcessId, pid: Option<u32>) -> bool {
        let restarted = self.with_table(|table| {
            let Some(entry) = table.get_mut(&id) else {
                return false;
            };
            if entry.state == ProcessState::Running {
                return false;
            }
            entry.ring.push("[перезапуск]".to_string());
            entry.state = ProcessState::Running;
            entry.started_at = TsMillis::now();
            entry.finished_at = None;
            entry.pid = pid;
            entry.stopper = None;
            entry.stop_reported = false;
            true
        });
        if !restarted {
            return false;
        }
        // The old run's ending is history now, not news.
        self.notices().retain(|n| n.id != id);
        self.announce();
        if *self.watched() == Some(id) {
            self.watch(Some(id));
        }
        true
    }

    /// Endings the model has not heard about, taken out of the queue.
    pub fn take_notices(&self) -> Vec<ExitNotice> {
        std::mem::take(&mut *self.notices())
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

    pub fn summary(&self, id: ProcessId) -> Option<ProcessSummary> {
        self.with_table(|table| table.get(&id).map(|e| e.summary(id)))
    }

    pub fn summaries(&self) -> Vec<ProcessSummary> {
        self.with_table(|table| table.iter().map(|(id, e)| e.summary(*id)).collect())
    }

    /// Everything printed since the model last asked (`bash_output`).
    pub fn read(&self, id: ProcessId, max_bytes: usize) -> Option<ProcessRead> {
        let read = self.with_table(|table| {
            let entry = table.get_mut(&id)?;
            let (lines, missed) = entry.ring.since(entry.cursor);
            let count = lines.len() as u32;
            let (text, _truncated) = output::clamp(&lines, max_bytes);
            entry.cursor = entry.ring.next_index();
            let stopped = entry.state == ProcessState::Stopped;
            let stopped_by_human = stopped
                && entry.stopper == Some(Stopper::Human)
                && !std::mem::replace(&mut entry.stop_reported, true);
            let stopped_by_model = stopped && entry.stopper == Some(Stopper::Model);
            Some(ProcessRead {
                state: entry.state.clone(),
                text,
                missed,
                lines: count,
                stopped_by_human,
                stopped_by_model,
            })
        });
        // Reading a command that has ended is hearing about it: a notice for
        // the same ending would tell the model what it just read.
        if read
            .as_ref()
            .is_some_and(|r| r.state != ProcessState::Running)
        {
            self.notices().retain(|n| n.id != id);
        }
        read
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

    fn notices(&self) -> std::sync::MutexGuard<'_, Vec<ExitNotice>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
/// [`display_cwd`] for a session's workspace. A directory in an additional root
/// is shown absolute: relative to the primary root it would name a different
/// place.
pub fn display_cwd_in(ws: &crate::workspace::Workspace, cwd: &Path) -> String {
    if ws.in_additional(cwd) {
        return ws.display(cwd);
    }
    display_cwd(ws.root(), cwd)
}

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
    fn an_ending_nobody_asked_for_is_held_as_news_once() {
        let (procs, _rx) = registry();
        let id = procs.register("npm run dev".into(), ".".into(), None);
        procs.finished(id, Some(1));

        let notices = procs.take_notices();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].id, id);
        assert_eq!(notices[0].code, Some(1));
        assert!(procs.take_notices().is_empty(), "news is handed over once");
    }

    #[test]
    fn a_stop_is_not_news_to_whoever_stopped() {
        let (procs, _rx) = registry();
        let by_human = procs.register("a".into(), ".".into(), None);
        let by_model = procs.register("b".into(), ".".into(), None);
        procs.stop(by_human);
        procs.kill(by_model);
        procs.finished(by_human, None);
        procs.finished(by_model, None);

        assert!(procs.take_notices().is_empty());
        let read = procs.read(by_model, 100).unwrap();
        assert!(read.stopped_by_model && !read.stopped_by_human);
        let read = procs.read(by_human, 100).unwrap();
        assert!(read.stopped_by_human && !read.stopped_by_model);
    }

    #[test]
    fn reading_an_ended_command_is_hearing_about_it() {
        let (procs, _rx) = registry();
        let id = procs.register("npm test".into(), ".".into(), None);
        procs.finished(id, Some(0));

        procs.read(id, 100).unwrap();
        assert!(procs.take_notices().is_empty());
    }

    #[test]
    fn the_first_stopper_keeps_the_credit() {
        let (procs, _rx) = registry();
        let id = procs.register("npm run dev".into(), ".".into(), None);
        procs.kill(id);
        procs.stop(id);
        procs.finished(id, None);

        assert!(procs.read(id, 100).unwrap().stopped_by_model);
    }

    #[tokio::test]
    async fn waiting_for_an_ending_sees_it_and_times_out_without_one() {
        let (procs, _rx) = registry();
        let id = procs.register("npm run dev".into(), ".".into(), None);

        assert!(!procs.wait_ended(id, Duration::from_millis(50)).await);

        let later = procs.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            later.finished(id, Some(0));
        });
        assert!(procs.wait_ended(id, Duration::from_secs(5)).await);
    }

    #[test]
    fn a_restart_reuses_the_row_and_forgets_the_old_ending() {
        let (procs, _rx) = registry();
        let id = procs.register("npm run dev".into(), ".".into(), None);
        procs.output(id, vec!["old".into()]);
        procs.finished(id, Some(1));

        assert!(procs.restart(id, None));
        let summary = procs.summary(id).unwrap();
        assert_eq!(summary.state, ProcessState::Running);
        assert!(summary.finished_at.is_none());
        assert!(procs.take_notices().is_empty());
        assert!(procs.read(id, 100).unwrap().text.starts_with("old
"));

        // A running row cannot be restarted out from under itself.
        assert!(!procs.restart(id, None));
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
