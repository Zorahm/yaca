//! Running tools: from the permission question to the line in the feed.
//!
//! Permission is asked before the run, not around it: §5.7 exists precisely
//! so that the human decides before anything happens. The question itself lives in
//! [`asking`](super::asking).
//!
//! A model response may contain several calls, and it asks for them in a batch precisely
//! so that they run together. Which ones may run together is decided by `Tool::concurrent`;
//! the rest run one after another. See [`Caller::run_batch`].
//!
//! The one making calls is a [`Caller`], not the worker, because the agent is
//! not the only one who makes them: a subagent's calls go down the same road —
//! the same question to the human, the same checkpoint, the same row with its
//! diff. A second road for them would be a second set of rules about what may
//! touch the working tree.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use serde_json::Value;

use crate::error::ToolError;
use crate::event::{AgentEvent, ToolMeta};
use crate::hooks::{self, Finished, PreOutcome};
use crate::id::{CallId, TurnId};
use crate::message::ContentBlock;
use crate::instructions::Instructions;
use crate::tool::memory::store::MemoryIndex;
use crate::skills::Listing;
use crate::intent::Intent;
use crate::provider::Provider;
use crate::tool::{take_description, Progress, ToolCtx, ToolRegistry};
use crate::usage::ModelInfo;
use crate::verify;
use crate::workspace::Workspace;

use super::accumulator::{Garbled, PendingCall};
use super::asking::{Answer, Gate, Queue};
use super::plugin_hooks::PluginPre;
use super::changeset;
use super::checkpoints;
use super::questions::Questions;
use super::subagent::Subagents;
use super::Shared;

/// What the model is told when the human stops a call.
///
/// English, like the system prompt, and blunt on purpose. A model that reads
/// only "cancelled" treats it as a transient failure and tries the same thing
/// again; the point of stopping was that the person wants a word first.
const STOPPED_BY_HUMAN: &str = "The user doesn't want to proceed with this tool use. \
     The tool use was rejected (eg. if it was a file edit, the new content was NOT \
     written to the file). STOP what you are doing and wait for the user to tell you \
     how to proceed.\n\nNote: the user's next message may contain a correction or \
     preference. Pay close attention — if they explain what went wrong or how they'd \
     prefer you to work, follow it from now on.";

/// Whoever is making calls: the agent in its turn, or a subagent inside one.
#[derive(Clone)]
pub(super) struct Caller {
    pub(super) shared: Arc<Shared>,
    pub(super) registry: Arc<ToolRegistry>,
    /// Whose registry of read files the calls check (§8). A subagent has its
    /// own: what it read, the agent has not.
    pub(super) ws: Arc<Workspace>,
    pub(super) provider: Arc<dyn Provider>,
    pub(super) instructions: Option<Arc<Instructions>>,
    /// The memory index and skills listing the agent's prompt carries. A
    /// subagent gets the same two, so it knows what the project keeps.
    pub(super) memory: Option<Arc<MemoryIndex>>,
    pub(super) skills: Option<Arc<Listing>>,
    /// What the agent's prompt says about the MCP servers. A subagent has the
    /// same tools, so it is told the same.
    pub(super) mcp_notes: Option<Arc<str>>,
    /// The `Task` row these calls are drawn under. `None` for the agent's own.
    pub(super) under: Option<CallId>,
    /// The model a named agent asked for.
    pub(super) model: ModelPick,
}

/// Which model a caller's requests go to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum ModelPick {
    /// The session's, or for a subagent the subagent model from settings.
    #[default]
    Default,
    /// The conversation's own model (`model: inherit`).
    Session,
    Exact(String),
}

/// A call that has been dealt with.
struct Ran {
    /// What the model reads as the call's result.
    result: ContentBlock,
    /// Whether the human was asked about it (§5.7's counter).
    asked: bool,
    /// Images the call read, as blocks for the model.
    shown: Vec<ContentBlock>,
}

impl Caller {
    /// The model these calls are made by: the session's, or the one the
    /// project gives its subagents.
    pub(super) fn model(&self) -> ModelInfo {
        let session = self.shared.state.lock().unwrap().model.clone();
        let id = match (&self.model, &self.under) {
            (ModelPick::Exact(id), _) => id.as_str(),
            (ModelPick::Session, _) | (ModelPick::Default, None) => &session,
            (ModelPick::Default, Some(_)) => self.shared.settings.subagent_model_or(&session),
        };
        self.shared.model_info(id)
    }

    /// Which tools exist at all in the current mode (§7.1).
    pub(super) fn intent(&self) -> Intent {
        self.shared.state.lock().unwrap().mode.intent()
    }

    pub(super) fn emit(&self, event: AgentEvent) {
        self.shared.emit(event);
    }

    /// A batch of calls from one model response (§5.4).
    ///
    /// Consecutive calls marked [`Tool::concurrent`] go off to run
    /// together; the rest run one at a time, because they need order: the checkpoint
    /// is taken before a command, the §5.7 question is shown one at a time, and an edit
    /// lands on top of the previous one.
    ///
    /// Only consecutive calls run at once. Letting reads jump ahead of their neighbours
    /// would reorder the run the model wrote in its own order.
    ///
    /// Results are collected back in their original order. They are linked
    /// by `tool_use_id`, but the order is what the model reads its own turn in.
    ///
    /// [`Tool::concurrent`]: crate::tool::Tool::concurrent
    pub(super) async fn run_batch(
        &self,
        turn_id: &TurnId,
        calls: &[PendingCall],
        cancel: &CancellationToken,
    ) -> Vec<ContentBlock> {
        let mut done: Vec<(usize, Ran)> = Vec::with_capacity(calls.len());
        let mut asked = 0u32;
        let mut at = 0;

        while at < calls.len() {
            let run = self.concurrent_run(&calls[at..]);
            if run > 1 {
                let together = (at..at + run).map(|index| async move {
                    let queue = Queue {
                        asked,
                        rest: &calls[index + 1..],
                    };
                    let ran = self
                        .run_call(turn_id, calls[index].clone(), cancel, queue)
                        .await;
                    (index, ran)
                });
                // None of them asks for permission — otherwise they would not be here —
                // so `asked` does not move.
                done.extend(futures_util::future::join_all(together).await);
                at += run;
            } else {
                let queue = Queue {
                    asked,
                    rest: &calls[at + 1..],
                };
                let ran = self
                    .run_call(turn_id, calls[at].clone(), cancel, queue)
                    .await;
                asked += u32::from(ran.asked);
                done.push((at, ran));
                at += 1;
            }
        }

        done.sort_by_key(|(index, _)| *index);
        // The check (`verify.rs`) is attached to the last successful change of
        // the batch, once for the whole batch. Found before `done` is consumed
        // below, and by position in it, which is also the position of its
        // result.
        let check = done
            .iter()
            .rposition(|(index, ran)| {
                let succeeded = matches!(
                    ran.result,
                    ContentBlock::ToolResult {
                        is_error: false,
                        ..
                    }
                );
                succeeded && verify::changes_files(&calls[*index].name)
            })
            .map(|at| (at, CallId::from(calls[done[at].0].id.clone())));

        // Every result first, then what the calls gave the model to look at:
        // a provider wants the results at the head of the message, and takes
        // an image only outside them (`ContentBlock::Image`).
        let mut results = Vec::with_capacity(done.len());
        let mut shown = Vec::new();
        for (_, ran) in done {
            results.push(ran.result);
            shown.extend(ran.shown);
        }

        // Plan mode has no writing tools, so the intent check only matters if
        // that ever changes. Not run once the human has stopped the turn: the
        // loop is ending and nobody will read the result.
        if let Some((at, row)) = check {
            if let Some(settings) = self.shared.settings.verify.as_ref() {
                if self.intent() == Intent::Agent && !cancel.is_cancelled() {
                    let verdict =
                        verify::run(settings, self.ws.as_ref(), self.shared.shell, cancel).await;
                    self.emit(AgentEvent::ToolCallProgress {
                        call_id: row,
                        chunk: verify::for_human(settings.command(), &verdict),
                    });
                    if let Some(note) = verify::for_model(settings.command(), &verdict) {
                        if let ContentBlock::ToolResult { content, .. } = &mut results[at] {
                            content.push_str(&note);
                        }
                    }
                }
            }
        }

        results.extend(shown);
        results
    }

    /// One call, from the question about it to its result.
    async fn run_call(
        &self,
        turn_id: &TurnId,
        call: PendingCall,
        cancel: &CancellationToken,
        queue: Queue<'_>,
    ) -> Ran {
        let mut shown = Vec::new();
        let (result, asked) = self.run_one(turn_id, call, cancel, queue, &mut shown).await;
        Ran {
            result,
            asked,
            shown,
        }
    }

    /// How many calls from the start of the list may run at once.
    ///
    /// An unknown tool is not concurrent: nothing is known about it, and
    /// [`Caller::run_one`] answers it with a line saying so.
    fn concurrent_run(&self, calls: &[PendingCall]) -> usize {
        calls
            .iter()
            .take_while(|call| {
                self.registry
                    .get(&call.name, self.intent())
                    .is_some_and(|tool| tool.concurrent())
            })
            .count()
    }

    /// `shown` collects what the call gives the model to look at. An argument
    /// rather than part of what is returned, because of the dozen ways out of
    /// here only one has anything to show.
    async fn run_one(
        &self,
        turn_id: &TurnId,
        call: PendingCall,
        cancel: &CancellationToken,
        queue: Queue<'_>,
        shown: &mut Vec<ContentBlock>,
    ) -> (ContentBlock, bool) {
        let call_id = CallId::from(call.id.clone());
        let mut input = call.input;
        let tool = self.registry.get(&call.name, self.intent());
        // Before the first event of the call, and before the checks that may
        // fail it: a silent call stays silent whichever way it ends.
        if tool.as_ref().is_some_and(|tool| tool.silent()) {
            self.shared.silent.lock().unwrap().insert(call_id.0.clone());
        }
        // The caption is the feed's concern; the model does not need it back. Unless the
        // tool has an argument of that name itself: then it is the tool's.
        let description = match &tool {
            Some(tool) if tool.keeps_description() => None,
            _ => take_description(&mut input),
        };

        // Arguments that never arrived whole: there is nothing to run, but
        // the call is in the history and is answered like any other.
        if let Some(kind) = call.garbled {
            let err = garbled(kind);
            let result = self.failed_unrun(turn_id, call_id, &call.name, description, input, err);
            return (result, false);
        }

        let Some(tool) = tool else {
            let err = ToolError::failed(format!("unknown tool: {}", call.name));
            let result = self.failed_unrun(turn_id, call_id, &call.name, description, input, err);
            return (result, false);
        };

        let target = tool.target(&input);
        self.emit(AgentEvent::ToolCallStarted {
            turn_id: turn_id.clone(),
            call_id: call_id.clone(),
            tool: call.name.clone(),
            description,
            target,
            input: input.clone(),
            parent: self.under.clone(),
        });

        // §7.1: a tool offered in Plan may still refuse this call — Bash does,
        // for anything that could change a file. Refused before the question: a
        // question about an action Plan does not allow would ask for something
        // that cannot happen.
        if self.intent() == Intent::Plan {
            if let Err(reason) = tool.plan_check(self.ws.as_ref(), &input) {
                self.emit(AgentEvent::ToolCallRejected {
                    call_id,
                    reason: reason.clone(),
                });
                return (tool_result(&call.id, reason, true), false);
            }
        }

        // One at a time across the whole session, not only within a batch:
        // subagents run side by side, and everything `Tool::concurrent` says
        // about two writers in one batch holds for two writers in two
        // conversations. Held from the question to the result, so the feed
        // has one request block open (§5.7) and the snapshot before a command
        // (§5.8) is of a tree nobody else is writing to.
        let _lane = if tool.concurrent() {
            None
        } else {
            tokio::select! {
                _ = cancel.cancelled() => {
                    return (self.stopped_by_human(&call.id, call_id, cancel), false);
                }
                lane = self.shared.lane.lock() => Some(lane),
            }
        };

        // What the model is told about arguments a hook rewrote.
        let mut rewritten: Vec<String> = Vec::new();
        // Hooks answer before the permission question (§5.7): a call a hook
        // refuses was never going to run, so the human is not asked about it. After
        // the lane, so a hook that reads the tree sees what the call would see.
        match hooks::before(
            self.shared.settings.hooks(),
            self.ws.as_ref(),
            self.shared.shell,
            &call.name,
            &input,
            cancel,
        )
        .await
        {
            PreOutcome::Go(None) => {}
            PreOutcome::Go(Some(updated)) => {
                self.emit(AgentEvent::ToolCallProgress {
                    call_id: call_id.clone(),
                    chunk: "Project hook changed the call's arguments\n".to_string(),
                });
                rewritten.push(super::plugin_hooks::rewritten_note("The project's", &updated));
                input = updated;
            }
            PreOutcome::Refused(refusal) => {
                self.emit(AgentEvent::ToolCallRejected {
                    call_id,
                    reason: refusal.for_human,
                });
                return (tool_result(&call.id, refusal.for_model, true), false);
            }
            PreOutcome::Stopped => {
                return (self.stopped_by_human(&call.id, call_id, cancel), false);
            }
        }

        // The plugins' hooks after the project's: the project's guard has the
        // first word. A plugin can refuse a call or ask for the question; it
        // cannot answer the question for the human.
        let (mut pre_context, force_ask) = match self.plugin_pre(&call_id, &call.name, &mut input, cancel).await {
            PluginPre::Go(context) => (context, false),
            PluginPre::Ask(context) => (context, true),
            PluginPre::Blocked { for_human, for_model } => {
                self.emit(AgentEvent::ToolCallRejected {
                    call_id,
                    reason: for_human,
                });
                return (tool_result(&call.id, for_model, true), false);
            }
        };
        rewritten.append(&mut pre_context);
        let pre_context = rewritten;
        if cancel.is_cancelled() {
            return (self.stopped_by_human(&call.id, call_id, cancel), false);
        }

        let mut asked_here = false;
        if let Some(need) = tool.permission(self.ws.as_ref(), &input) {
            let mut gate = match self.gate(&need) {
                Gate::Pass if force_ask => Gate::Ask,
                gate => gate,
            };
            // Auto: a model reads the call before the human is bothered. Its
            // "safe" removes the question; anything else leaves it.
            if matches!(gate, Gate::Ask)
                && !force_ask
                && self.reviewable(&need)
                && self.review_approves(&need, cancel).await
            {
                gate = Gate::Pass;
            }
            match gate {
                Gate::Pass => {}
                Gate::Refused(reason) => {
                    return (self.refused(&call.id, call_id, reason, true), false);
                }
                Gate::Ask => {
                    asked_here = true;
                    match self.ask(&call_id, &need, cancel, queue).await {
                        Answer::Allowed => {}
                        Answer::Denied { reason, standing } => {
                            return (self.refused(&call.id, call_id, reason, standing), true);
                        }
                        Answer::Cancelled => {
                            return (self.stopped_by_human(&call.id, call_id, cancel), true);
                        }
                    }
                }
            }
        }

        // §5.8: a snapshot before every permitted command. After the gate,
        // because a refused command changes nothing and needs no point; before
        // the run, because a point taken afterwards is a point of the damage.
        if let Some(reason) = tool.checkpoint(&input) {
            checkpoints::create(&self.shared, reason).await;
        }

        let ctx = ToolCtx {
            ws: self.ws.clone(),
            processes: self.shared.processes.clone(),
            events: self.shared.tool_events(),
            progress: Progress::new(call_id.clone(), self.shared.events.clone()),
            cancel: cancel.clone(),
            // A subagent does not send another: the agent is the one who sees
            // the whole task and decides how to split it.
            agents: self
                .under
                .is_none()
                .then(|| Subagents::new(self.clone(), turn_id.clone(), call_id.clone())),
            // A question belongs to the call that asks it, a subagent's too.
            questions: Some(Questions::new(self.shared.clone(), call_id.clone())),
            vision: self.shared.sees(&self.model()),
            shell: self.shared.shell,
            respect_robots: self.shared.respect_robots,
            title_locked: self
                .shared
                .recorder
                .as_ref()
                .is_some_and(|r| !r.meta().title_open_to_model()),
            transcript: self.shared.recorder.as_ref().map(|r| r.transcript()),
            lsp: Some(self.shared.lsp.clone()),
            skills: self.shared.skills.clone(),
        };

        // Copied only when a post hook will read it: most calls have none, and
        // a large write should not be copied for nothing.
        let for_hooks = (hooks::wants_post(self.shared.settings.hooks(), &call.name)
            || self.shared.plugins.hooks.has(crate::plugin::hooks::HookEvent::PostToolUse)
            || !pre_context.is_empty())
        .then(|| input.clone());

        // §6.3: what the file held before a writer touched it, so one hunk of
        // the write can be reverted later. Taken after the gate, like the
        // snapshot: a refused write changes nothing and needs no baseline.
        let before = match tool.target(&input) {
            Some(target) if changeset::keeps(tool.name()) => {
                changeset::before(&self.shared, target).await
            }
            _ => None,
        };

        let started = std::time::Instant::now();
        let outcome = tool.run(&ctx, input).await;
        let duration_ms = started.elapsed().as_millis() as u32;

        match outcome {
            Ok(mut out) => {
                let changed = out.changed.is_some();
                // What the language server makes of the file just written,
                // in the same result: the model learns what it broke without
                // asking. Before the feed gets the text, so the row shows it.
                if let Some(change) = &out.changed {
                    if change.fact.kind != crate::change::ChangeKind::Deleted {
                        if let Ok(abs) = self.ws.resolve_new(&change.fact.path) {
                            let note = self
                                .shared
                                .lsp
                                .after_edit(&abs, &change.fact.path, cancel)
                                .await;
                            if let Some(note) = note {
                                out.text.push_str("\n\n");
                                out.text.push_str(&note);
                            }
                        }
                    }
                }
                // The same for each file of a call that changed many. The
                // server was told the new text by the tool itself, so this
                // reads what it has published rather than waiting for it.
                for file in &out.edited {
                    let note = self
                        .shared
                        .lsp
                        .after_edit(&file.abs, &file.change.fact.path, cancel)
                        .await;
                    if let Some(note) = note {
                        out.text.push_str("\n\n");
                        out.text.push_str(&note);
                    }
                }
                // The feed gets the text exactly once. A streaming tool has already handed
                // it over along the way — the command writes to the feed while it runs (§0) —
                // the session hands it to everyone else from here.
                // A detached call has said nothing for the human yet: its text
                // is a note to the model, and its row is not this call's to end.
                if !out.detached && !ctx.progress.used() && !out.text.is_empty() {
                    self.emit(AgentEvent::ToolCallProgress {
                        call_id: call_id.clone(),
                        chunk: out.text.clone(),
                    });
                }
                // File action: the line gets its counters and diff.
                if let Some(change) = out.changed {
                    self.emit(AgentEvent::FileChanged {
                        call_id: call_id.clone(),
                        fact: change.fact,
                        diff: change.diff,
                    });
                }
                if let (true, Some(before)) = (changed, before) {
                    changeset::after(&self.shared, before).await;
                }
                for file in std::mem::take(&mut out.edited) {
                    self.emit(AgentEvent::FileChanged {
                        call_id: call_id.clone(),
                        fact: file.change.fact.clone(),
                        diff: file.change.diff,
                    });
                    self.shared.changes.record(
                        file.change.fact.path,
                        file.abs,
                        Some(file.before),
                        Some(file.after),
                    );
                }
                let text = self
                    .after_hooks(&call.name, for_hooks, false, out.text, &call_id, &pre_context, cancel)
                    .await;
                if !out.detached {
                    self.emit(AgentEvent::ToolCallFinished {
                        call_id,
                        meta: ToolMeta {
                            duration_ms,
                            ..out.meta
                        },
                    });
                }
                for (name, picture) in out.pictures {
                    shown.extend(ContentBlock::picture(&name, picture));
                }
                (tool_result(&call.id, text, false), asked_here)
            }
            Err(ToolError::Rejected(reason)) => {
                // A tripped guard is not a failure (§5.9).
                self.emit(AgentEvent::ToolCallRejected {
                    call_id,
                    reason: reason.clone(),
                });
                (tool_result(&call.id, reason, true), asked_here)
            }
            // Both mean the same thing to the model and to the feed: the
            // person stopped this. They differ only in who noticed first —
            // `Cancelled` comes from the turn's token, `Interrupted` from a
            // tool that found its process killed from the panel.
            Err(ToolError::Cancelled | ToolError::Interrupted) => {
                (self.stopped_by_human(&call.id, call_id, cancel), asked_here)
            }
            Err(err @ ToolError::Failed { .. }) => {
                // The feed gets the line, the model gets the line and the detail:
                // the output of a failed command in an action's caption would be a wall of text,
                // and without it the model has nothing to fix.
                let text = self
                    .after_hooks(&call.name, for_hooks, true, err.for_model(), &call_id, &pre_context, cancel)
                    .await;
                self.emit(AgentEvent::ToolCallFailed {
                    call_id,
                    message: err.to_string(),
                });
                (tool_result(&call.id, text, true), asked_here)
            }
        }
    }

    /// A call that fails before any tool runs: its row in the feed, and the
    /// error for the model.
    fn failed_unrun(
        &self,
        turn_id: &TurnId,
        call_id: CallId,
        tool: &str,
        description: Option<String>,
        input: serde_json::Value,
        err: ToolError,
    ) -> ContentBlock {
        let result = tool_result(&call_id.0, err.for_model(), true);
        self.emit(AgentEvent::ToolCallStarted {
            turn_id: turn_id.clone(),
            call_id: call_id.clone(),
            tool: tool.to_string(),
            description,
            target: None,
            input,
            parent: self.under.clone(),
        });
        self.emit(AgentEvent::ToolCallFailed {
            call_id,
            message: err.to_string(),
        });
        result
    }

    /// The post hooks of a call that ran. What they say goes into the call's row
    /// and onto the end of the text the model reads. A hook cannot change whether
    /// the call is an error: the flag is the tool's own, passed through.
    ///
    /// The project's hooks first, then the plugins', then what the plugins'
    /// `PreToolUse` hooks wanted the model to read with the result.
    #[allow(clippy::too_many_arguments)]
    async fn after_hooks(
        &self,
        tool: &str,
        input: Option<Value>,
        failed: bool,
        text: String,
        call_id: &CallId,
        pre_context: &[String],
        cancel: &CancellationToken,
    ) -> String {
        let Some(input) = input else {
            return text;
        };
        let text = self.project_after_hooks(tool, &input, failed, text, call_id, cancel).await;
        let mut text = self.plugin_post(call_id, tool, &input, failed, text, cancel).await;
        for context in pre_context {
            text.push_str(context);
        }
        text
    }

    async fn project_after_hooks(
        &self,
        tool: &str,
        input: &Value,
        failed: bool,
        text: String,
        call_id: &CallId,
        cancel: &CancellationToken,
    ) -> String {
        let finished = Finished {
            failed,
            result: &text,
        };
        match hooks::after(
            self.shared.settings.hooks(),
            self.ws.as_ref(),
            self.shared.shell,
            tool,
            input,
            finished,
            cancel,
        )
        .await
        {
            None => text,
            Some(note) => {
                self.emit(AgentEvent::ToolCallProgress {
                    call_id: call_id.clone(),
                    chunk: note.for_human,
                });
                text + &note.for_model
            }
        }
    }

    /// The human stopped this call: one row in the feed, one sentence to the
    /// model, and the turn over.
    ///
    /// Cancelling the token is what ends the loop. The rest of the batch still
    /// goes through `run_call` and still answers — the API needs a result for
    /// every `tool_use` — but each answers at once with the same refusal, and
    /// `run_turn` stops before asking the model anything again.
    fn stopped_by_human(
        &self,
        id: &str,
        call_id: CallId,
        cancel: &CancellationToken,
    ) -> ContentBlock {
        self.emit(AgentEvent::ToolCallDenied {
            call_id,
            reason: None,
            stopped: true,
        });
        cancel.cancel();
        tool_result(id, STOPPED_BY_HUMAN.to_string(), true)
    }

    /// The human refused this call: one row in the feed, and the refusal
    /// for the model. Unlike a stop, the turn goes on — refusing one action
    /// is no reason to end a turn that may still have other work in it.
    fn refused(
        &self,
        id: &str,
        call_id: CallId,
        reason: Option<String>,
        standing: bool,
    ) -> ContentBlock {
        let text = refusal_for_model(reason.as_deref(), standing);
        self.emit(AgentEvent::ToolCallDenied {
            call_id,
            reason,
            stopped: false,
        });
        tool_result(id, text, true)
    }
}

/// What the model reads when the human refused a call.
///
/// The note is the point (§5.7): refused without one, the agent simply tries
/// again. A standing refusal says that it is standing, and says it about
/// other roads too — after `write_file` was refused, a local model went for
/// `echo … > README.md` to write the same file.
fn refusal_for_model(reason: Option<&str>, standing: bool) -> String {
    let mut text =
        String::from("The user denied this tool use: nothing it would have done was done.");
    if standing {
        text.push_str(
            " They denied every similar call for the rest of the session. Do not try to \
             get the same result another way, such as a different tool or a shell command.",
        );
    }
    if let Some(why) = reason {
        text.push_str(" Their reason: ");
        text.push_str(why);
    }
    text
}

/// Why a call with unusable arguments was not run.
///
/// The feed gets one line; the model gets what to do next, in English like
/// the rest of what it reads. A call cut by the output cap would be cut again
/// if repeated as it was, so the way around the cap is spelled out.
fn garbled(kind: Garbled) -> ToolError {
    match kind {
        Garbled::Cut => ToolError::failed_with(
            "response was cut off by output limit, call was not run",
            "Your response reached its output limit while the arguments of this call \
             were still being written, so it was not run and nothing changed. Make the \
             call again with less in it: write a large file in parts — create it with \
             the first part, then add the rest with Edit — and change an existing file \
             with Edit rather than rewriting it.",
        ),
        Garbled::Invalid => ToolError::failed_with(
            "call arguments are not valid JSON, call was not run",
            "The arguments of this call were not valid JSON, so it was not run and \
             nothing changed. Make the call again with its arguments as one JSON object.",
        ),
    }
}

fn tool_result(tool_use_id: &str, content: String, is_error: bool) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: tool_use_id.to_string(),
        content,
        is_error,
    }
}
