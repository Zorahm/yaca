//! Subagents sent in the background: the agent goes on while they work.
//!
//! A `Task` call with `background` answers at once, and the subagent runs
//! beside the turn rather than inside one of its batches. Its report reaches
//! the agent at the next point the model is spoken to — with the results of a
//! batch, or, when the agent has nothing left to do, as the message it was
//! waiting for ([`Worker::await_news`]).
//!
//! They do not outlive the turn. The busy window, `Esc`, the commit of an
//! isolated chat and the journal are all tied to the turn, and work that goes
//! on after the feed said "done" would be outside every one of them. A turn
//! with subagents still out waits for them, and a turn that ends any other
//! way stops them first ([`Background::stop`]).
//!
//! The `Task` row stays open until the subagent is done: the feed reads a
//! subagent's state from its row, and a row closed when the tool returned
//! would show finished work over rows that keep arriving under it.

use std::sync::Mutex;
use std::time::Instant;

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::error::ToolError;
use crate::event::{AgentEvent, ToolMeta};
use crate::id::{CallId, TurnId};
use crate::message::{ContentBlock, Message, Role};
use crate::tool::ToolOutput;

use super::inbox::Inbox;
use super::subagent::Subagents;
use super::turn::Worker;

/// What a background subagent came back with, for the agent to read.
pub(super) struct Report {
    /// The `Task` call that sent it: the name the agent knows it by.
    row: CallId,
    /// The report, or why there is none.
    outcome: Result<String, String>,
}

impl Report {
    fn for_model(self) -> ContentBlock {
        let text = match self.outcome {
            Ok(report) => format!(
                "[background subagent {} has reported]\n{report}",
                self.row
            ),
            Err(why) => format!(
                "[background subagent {} failed and wrote no report]\n{why}\nWhat it \
                 changed before failing is in the working tree: look before going on.",
                self.row
            ),
        };
        ContentBlock::Text { text }
    }
}

#[derive(Default)]
struct State {
    /// Sent and not yet handed to the agent, the finished ones included.
    out: usize,
    finished: Vec<Report>,
}

/// The background subagents of the running turn.
#[derive(Default)]
pub(super) struct Background {
    state: Mutex<State>,
    /// Signalled each time one finishes.
    finished: Notify,
}

impl Background {
    /// One more is out. Counted before it starts, so a turn that looks
    /// straight after sending it already sees it.
    fn sent(&self) {
        self.state.lock().unwrap().out += 1;
    }

    fn finish(&self, report: Report) {
        self.state.lock().unwrap().finished.push(report);
        self.finished.notify_one();
    }

    /// Whether the agent is still owed a report.
    pub(super) fn pending(&self) -> bool {
        self.state.lock().unwrap().out > 0
    }

    /// The reports that are in, taken.
    fn take(&self) -> Vec<Report> {
        let mut state = self.state.lock().unwrap();
        let reports = std::mem::take(&mut state.finished);
        state.out -= reports.len();
        reports
    }

    /// Waits until `enough` says so of the state, looking again each time a
    /// subagent finishes.
    async fn wait(&self, enough: impl Fn(&State) -> bool) {
        loop {
            // Asked for before the check: one that finishes between the check
            // and the wait has then already left its signal.
            let signal = self.finished.notified();
            if enough(&self.state.lock().unwrap()) {
                return;
            }
            signal.await;
        }
    }

    /// Waits for a report to be in.
    async fn ready(&self) {
        self.wait(|state| !state.finished.is_empty()).await;
    }

    /// Ends the turn's subagents: stops those still working, waits for their
    /// rows to close, and drops reports nobody will read.
    ///
    /// Before the turn's own last event, so the feed never shows a turn that
    /// is over with rows still running in it.
    pub(super) async fn stop(&self, cancel: &CancellationToken) {
        if !self.pending() {
            return;
        }
        cancel.cancel();
        self.wait(|state| state.finished.len() == state.out).await;
        drop(self.take());
    }
}

impl Subagents {
    /// Sends a subagent on `task` and returns without waiting for it.
    pub(crate) fn send(&self, task: String, cancel: &CancellationToken) -> ToolOutput {
        let agents = self.clone();
        let cancel = cancel.clone();
        let shared = self.sender.shared.clone();
        shared.background.sent();
        tokio::spawn(async move {
            let started = Instant::now();
            let outcome = agents.run(task, &cancel).await;
            let report = agents.close_row(outcome, started.elapsed().as_millis() as u32);
            shared.background.finish(report);
        });

        ToolOutput::new(format!(
            "Subagent {} is working in the background. Its report will be given to you \
             when it is ready, with no call from you: go on with your own work \
             meanwhile, and leave what you handed over to it. If nothing is left for \
             you to do until it reports, end your message and you will be woken with \
             the report.",
            self.row
        ))
        .detached()
    }

    /// The end of the `Task` row, which `calls` left open for a detached
    /// call: the same events it sends for one that returns in place.
    fn close_row(&self, outcome: Result<String, ToolError>, duration_ms: u32) -> Report {
        let call_id = self.row.clone();
        let emit = |event| self.sender.emit(event);
        let outcome = match outcome {
            Ok(report) => {
                let meta = ToolMeta {
                    duration_ms,
                    ..ToolOutput::new(report.clone()).meta
                };
                emit(AgentEvent::ToolCallProgress {
                    call_id: call_id.clone(),
                    chunk: report.clone(),
                });
                emit(AgentEvent::ToolCallFinished { call_id, meta });
                Ok(report)
            }
            // The turn was stopped, and nobody reads what this says.
            Err(err @ (ToolError::Cancelled | ToolError::Interrupted)) => {
                emit(AgentEvent::ToolCallDenied {
                    call_id,
                    reason: None,
                    stopped: true,
                });
                Err(err.for_model())
            }
            Err(err) => {
                emit(AgentEvent::ToolCallFailed {
                    call_id,
                    message: err.to_string(),
                });
                Err(err.for_model())
            }
        };
        Report {
            row: self.row.clone(),
            outcome,
        }
    }
}

impl Worker {
    /// The reports that came in since the model last heard anything, as
    /// blocks it reads with its next input.
    pub(super) fn reports(&mut self) -> Vec<ContentBlock> {
        let reports = self.shared.background.take();
        if !reports.is_empty() {
            // What they spent after the last batch was counted.
            self.settle_subagents();
        }
        reports.into_iter().map(Report::for_model).collect()
    }

    /// The agent has nothing left to do and subagents are still out: waits,
    /// and returns what woke it as the message the model reads next.
    ///
    /// A report wakes it, and so does the human (§7.4): what they say in the
    /// meantime is said to the agent, and must not wait for a subagent that
    /// may take minutes. `None` once the turn is stopped.
    pub(super) async fn await_news(
        &mut self,
        turn_id: &TurnId,
        cancel: &CancellationToken,
        inbox: &mut Inbox,
    ) -> Option<Message> {
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => return None,
                _ = self.shared.background.ready() => {}
                _ = inbox.message_waiting() => {}
            }

            let mut content = self.reports();
            content.extend(self.process_note());
            for queued in inbox.messages(&self.shared.queue) {
                let said = self.hear(turn_id, queued.text, queued.attachments).await;
                content.extend(said.content);
            }
            // Empty when what woke it was a message cancelled since.
            if !content.is_empty() {
                return Some(Message {
                    role: Role::User,
                    content,
                });
            }
        }
    }
}
