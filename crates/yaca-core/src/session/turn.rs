//! The agent's turn: from the human's message to a stop.
//!
//! The worker owns the history alone — it is not handed to anyone under a mutex —
//! and for the same reason lives in a separate task from the dispatcher.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::ProviderError;
use crate::event::{AgentEvent, StopReason, TsMillis};
use crate::id::TurnId;
use crate::instructions::Instructions;
use crate::message::{ContentBlock, Message, Role};
use crate::picture;
use crate::provider::{Provider, ProviderRequest, ProviderStream};
use crate::slash;
use crate::usage::{Pricing, UsageMeter};
use crate::workspace::Workspace;

use crate::checkpoint::CheckpointReason;

use super::accumulator::{Accumulator, RouteDecision};
use super::calls::{Caller, ModelPick};
use super::binding::{self, Fingerprint};
use super::guard::{self, Flow, RepeatGuard};
use super::inbox::Inbox;
use super::questions::Questions;
use super::{checkpoints, compact, environment, Job, Shared, ToolRegistry};

/// Cap on one response. Streaming removes the question of timeouts, but the window
/// is finite, and there is no reason to give the model all 128k for one reply.
const MAX_OUTPUT_TOKENS: u32 = 64_000;

/// How a turn came to its end.
enum End {
    Finished(StopReason),
    Cancelled,
    Failed(ProviderError),
    /// The ceiling on steps in the settings.
    OutOfSteps(u32),
    /// The model answered with no text and no call: reasoning alone, or nothing.
    Silent(StopReason),
}

pub(super) struct Worker {
    pub(super) ws: Arc<Workspace>,
    pub(super) provider: Arc<dyn Provider>,
    /// The core's tools and those of the MCP servers, as of the start of the
    /// turn ([`Worker::take_mcp`]).
    pub(super) registry: Arc<ToolRegistry>,
    /// The state of the MCP servers `registry` was built from. `None` until
    /// the first turn.
    pub(super) mcp_seen: Option<u64>,
    /// What the system prompt says about the MCP servers, taken together with
    /// their tools.
    pub(super) mcp_notes: Option<Arc<str>>,
    /// The project's `AGENTS.md`, read once when the session starts. The
    /// system prompt is the cached prefix of every request, and a file read per
    /// message would move that prefix on every edit.
    pub(super) instructions: Option<Arc<Instructions>>,
    /// The project's memory index, read once when the session starts: the
    /// system prompt is the cached prefix, and a note written mid-session
    /// must not move it.
    pub(super) memory: Option<Arc<crate::tool::memory::store::MemoryIndex>>,
    /// The project's skills list, read once when the session starts, for the
    /// same reason as the memory index: it is part of the cached prefix.
    pub(super) skills: Option<Arc<crate::skills::Listing>>,
    /// The named subagents' catalog, for the system prompt (`agents`).
    pub(super) agents_section: Option<Arc<str>>,
    /// What the plugins' `SessionStart` hooks said, for the system prompt
    /// (`plugin_hooks`).
    pub(super) plugin_context: Option<Arc<str>>,
    pub(super) history: Vec<Message>,
    /// What the reasoning in `history` was produced under (`binding`).
    /// `None` until the first request, or for a session recorded before
    /// prefixes were written down.
    pub(super) prefix: Option<Fingerprint>,
    pub(super) usage: UsageMeter,
    pub(super) shared: Arc<Shared>,
    /// History has been replaced by a summary, so `history[0]` is that summary
    /// and §5.11 allows it to be rewritten.
    pub(super) compacted: bool,
    /// How much of `history` the provider's last figure covers. Whatever lies
    /// past it — results of calls, a message from the human — is estimated
    /// until the next answer measures it (`compaction`).
    pub(super) measured: usize,
}

impl Worker {
    pub(super) async fn run(mut self, rx: mpsc::Receiver<Job>) {
        self.session_start().await;
        let mut inbox = Inbox::new(rx);
        while let Some(job) = inbox.next().await {
            let token = CancellationToken::new();
            match job {
                // Taken off the queue here, not when it was sent. A message
                // cancelled while it waited is gone from the queue, and then
                // the worker is not busy for it at all.
                Job::Message { id } => {
                    let Some(message) = self.shared.queue.take(&id) else {
                        continue;
                    };
                    self.start(token.clone());
                    let subject = message.text.clone();
                    self.run_turn(message.text, message.attachments, token, &mut inbox)
                        .await;
                    self.save_isolated_work(subject).await;
                }
                Job::Compact => {
                    self.start(token.clone());
                    self.compact(&token, None).await;
                }
                Job::EditSummary { text } => {
                    self.start(token);
                    self.edit_summary(text);
                }
            }

            self.set_busy(false);
        }
        // The handle is gone: nobody will send this session anything again.
        self.session_end("exit").await;
    }

    /// The cancel token of the job now running, and the busy window it opens.
    fn start(&self, token: CancellationToken) {
        *self.shared.cancel.lock().unwrap() = token;
        self.set_busy(true);
    }

    /// Commits what the turn changed to the chat's branch (`isolation`). A
    /// no-op in the project's own tree.
    ///
    /// Inside the busy window: a chat that reads "done" while its work is not
    /// yet on the branch hands the human a merge of nothing.
    async fn save_isolated_work(&self, subject: String) {
        if self.shared.isolation.is_none() {
            return;
        }
        let tree = self.ws.root().to_path_buf();
        let outcome =
            tokio::task::spawn_blocking(move || crate::isolation::commit_all(&tree, &subject))
                .await;
        let failure = match outcome {
            Ok(Ok(_)) => return,
            Ok(Err(err)) => err,
            Err(_) => "the commit task did not finish".to_string(),
        };
        self.emit(AgentEvent::Error {
            code: "isolation_commit_failed".into(),
            message: format!("Chat edits were not saved to its branch: {failure}"),
            retryable: false,
        });
    }

    fn set_busy(&self, busy: bool) {
        self.shared.state.lock().unwrap().busy = busy;
        self.shared.emit_state();
    }

    pub(super) fn emit(&self, event: AgentEvent) {
        self.shared.emit(event);
    }

    async fn run_turn(
        &mut self,
        text: String,
        attachments: Vec<String>,
        cancel: CancellationToken,
        inbox: &mut Inbox,
    ) {
        let turn_id = TurnId::new();
        let end = self
            .steps(&turn_id, text, attachments, &cancel, inbox)
            .await;

        // Before the turn's last event: background subagents do not outlive
        // the turn (`background`), and their rows close ahead of it.
        self.shared.background.stop(&cancel).await;
        self.settle_subagents();

        match end {
            End::Finished(stop_reason) => self.emit(AgentEvent::TurnFinished {
                turn_id,
                stop_reason,
                at: TsMillis::now(),
            }),
            End::Cancelled => self.emit(AgentEvent::TurnCancelled {
                turn_id,
                at: TsMillis::now(),
            }),
            End::Failed(err) => self.fail_turn(turn_id, err),
            End::OutOfSteps(max_steps) => self.stop_at_ceiling(turn_id, max_steps),
            End::Silent(stop_reason) => self.stop_silent(turn_id, stop_reason),
        }
    }

    /// The turn itself: requests and batches of calls until something ends it.
    async fn steps(
        &mut self,
        turn_id: &TurnId,
        text: String,
        attachments: Vec<String>,
        cancel: &CancellationToken,
        inbox: &mut Inbox,
    ) -> End {
        let Some(mut opening) = self.hear_checked(turn_id, text, attachments, cancel).await else {
            // A plugin's hook kept the message from the model, and said why.
            return End::Finished(StopReason::EndTurn);
        };
        // Ahead of what the human said: it is what happened before they spoke.
        if let Some(note) = self.process_note() {
            opening.content.insert(0, note);
        }
        // Only the first message of a session, and only when there is no
        // history at all: it is the one message whose text stays put for the
        // whole conversation, so the note costs nothing in cache. A resumed
        // session already has it in the journal (`environment`).
        if self.history.is_empty() {
            let text = environment::note(
                self.ws.root(),
                self.shared.isolation.as_ref(),
                &self.ws.additional_text(),
            )
            .await;
            opening.content.insert(0, ContentBlock::Text { text });
        }
        self.emit(AgentEvent::TurnStarted {
            turn_id: turn_id.clone(),
            at: TsMillis::now(),
        });
        self.remember(turn_id, opening);

        // Servers still starting are waited for here, each for no longer than
        // its own startup deadline: their tools go in the head of the request.
        self.shared.mcp.settled(cancel).await;
        self.take_mcp();

        // Set once a request refused as too long has been compacted and sent
        // again, until one gets through.
        let mut rescued = false;
        // Answers received in this turn, against the ceiling in the settings.
        // A request that was refused and sent again is one answer, not two.
        let max_steps = self.shared.settings.max_steps();
        let mut steps = 0u32;
        let mut repeats = RepeatGuard::default();
        let mut stop_returns = 0u32;
        // The route last shown to the human in this turn (`show_route`). Local
        // to the turn, so the next message starts without one and shows its own.
        let mut shown_route: Option<RouteDecision> = None;
        loop {
            if cancel.is_cancelled() {
                return End::Cancelled;
            }
            if steps >= max_steps {
                return End::OutOfSteps(max_steps);
            }

            // Before every request, not only before the human's message: a
            // turn of many calls fills the window between its steps (§6.1.1).
            self.compact_if_full(turn_id, cancel).await;

            let request = self.build_request(turn_id);
            let mut stream = match self.open_stream(request, cancel).await {
                Ok(s) => s,
                // The provider measured what the estimate did not. Compacted
                // and sent again, once: refused again, what is left does not
                // fit either, and the provider's words are the answer.
                Err(err) if err.is_context_overflow() && !rescued => {
                    rescued = true;
                    if compact::plan(&self.history).is_some()
                        && self.compact(cancel, Some(turn_id)).await
                    {
                        continue;
                    }
                    return End::Failed(err);
                }
                Err(err) => return End::Failed(err),
            };
            rescued = false;

            let mut acc = Accumulator::new(turn_id.clone());
            loop {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return End::Cancelled,
                    item = stream.next() => match item {
                        None => break,
                        Some(Ok(event)) => {
                            for out in acc.push(event) {
                                self.emit(out);
                            }
                            // Before the text that follows it in the stream: the
                            // human sees which model answered before what it said.
                            self.show_route(acc.route(), &mut shown_route);
                        }
                        Some(Err(err)) => return End::Failed(err),
                    },
                }
            }

            let done = acc.finish();
            steps += 1;

            let pricing = self.pricing_of(done.routed.as_ref());
            self.usage.record(done.usage, pricing);

            let spoke = done
                .blocks
                .iter()
                .any(|b| matches!(b, ContentBlock::Text { text } if !text.trim().is_empty()));
            if !done.blocks.is_empty() {
                self.remember(turn_id, Message::assistant(done.blocks));
            }
            // The figure just recorded covers everything sent and answered.
            self.measured = self.history.len();
            // After the answer is in the history: the figure counts it, and
            // the split of the figure (`window`) has to count it too.
            self.emit(AgentEvent::UsageUpdated {
                usage: self.usage_snapshot(),
            });

            // Calls run whatever the stop reason says. A `tool_use` left
            // without its result makes the provider reject this conversation
            // on every request from here on, and the journal carries the hole
            // into a reopened session. Local OpenAI-compatible servers end a
            // message of calls with "stop", and a response cut at the output
            // cap ends with its last call half-written — that one is answered
            // with an error rather than run (`calls::garbled`).
            if done.calls.is_empty() {
                // Nothing left to do is not the end while subagents are still
                // out: their reports are what the agent sent them for.
                if !self.shared.background.pending() {
                    // Reasoning with no answer after it ends the response, and
                    // a turn that closed on it would look like a finished one.
                    if !spoke {
                        return End::Silent(done.stop_reason);
                    }
                    if let Some(back) = self.stop_hooks(&mut stop_returns, cancel).await {
                        self.remember(turn_id, back);
                        continue;
                    }
                    return End::Finished(done.stop_reason);
                }
                match self.await_news(turn_id, cancel, inbox).await {
                    Some(news) => self.remember(turn_id, news),
                    None => return End::Cancelled,
                }
                continue;
            }

            let results = self
                .caller()
                .run_batch(turn_id, &done.calls, cancel)
                .await;
            self.settle_subagents();
            // All results in one message: splitting them across several —
            // would quietly teach the model not to call tools in parallel.
            let mut message = Message::tool_results(results);

            // The same call with the same answer, again and again: put to the
            // human before the model is asked anything else. Not after a
            // stop — nothing is asked of the model any more in this turn.
            if let Some(repeat) = repeats.observe(&done.calls, &message.content) {
                if !cancel.is_cancelled() {
                    let questions = Questions::new(self.shared.clone(), repeat.call_id.clone());
                    let (flow, note) = guard::put_to_human(questions, &repeat, cancel).await;
                    // After the results, never before (see below).
                    message.content.push(note);
                    if matches!(flow, Flow::Stop) {
                        cancel.cancel();
                    }
                }
            }

            // §7.4: what the human sent meanwhile joins here, after the batch
            // and before the model's next step, in the same message — a
            // second user message in a row is one the provider may refuse.
            // Not after a cancel: the model is asked nothing more in this
            // turn, so the message would sit in the history unanswered; it
            // stays queued and gets a turn of its own.
            if !cancel.is_cancelled() {
                // After the results, never before: a provider wants every
                // `tool_result` first in the message that answers the calls.
                message.content.extend(self.reports());
                if let Some(note) = self.process_note() {
                    message.content.push(note);
                }
                for queued in inbox.messages(&self.shared.queue) {
                    let heard = self.hear_checked(turn_id, queued.text, queued.attachments, cancel);
                    if let Some(said) = heard.await {
                        message.content.extend(said.content);
                    }
                }
            }
            self.remember(turn_id, message);
        }
    }

    /// Shows the model a routing provider picked, unless this turn has already
    /// shown that same choice.
    ///
    /// Once per change and not per request: a turn of many calls makes a
    /// request for every step, and each would repeat the same chip in the feed.
    /// `shown` is the turn's memory of what it last showed, so a choice that
    /// goes away and comes back is shown again.
    fn show_route(&self, route: Option<&RouteDecision>, shown: &mut Option<RouteDecision>) {
        let Some(route) = route else {
            return;
        };
        if shown.as_ref() == Some(route) {
            return;
        }
        self.emit(AgentEvent::RouteDecided {
            route: route.route.clone(),
            provider: route.provider.clone(),
            model: route.model.clone(),
            matched: route.matched.clone(),
        });
        *shown = Some(route.clone());
    }

    /// The price of one request: by the model that answered when the request
    /// was routed (see `Shared::pricing_of`), otherwise by the session's model,
    /// which is `unknown_model` (free) if that is not listed either.
    fn pricing_of(&self, routed: Option<&(String, String)>) -> Pricing {
        let session = self.shared.state.lock().unwrap().model.clone();
        self.shared
            .pricing_of(routed, self.shared.model_info(&session).pricing)
    }

    /// Background commands that ended on their own since the model last heard
    /// anything, as a note it reads with its next input.
    ///
    /// Delivered at the next point the model is being spoken to anyway — after
    /// a batch of calls, or with the human's next message — and not by waking
    /// it: an idle agent starting a turn nobody asked for spends the human's
    /// money on a guess about what they want done with a crashed server.
    pub(super) fn process_note(&self) -> Option<ContentBlock> {
        let notices = self.shared.processes.take_notices();
        if notices.is_empty() {
            return None;
        }
        let mut text = String::from("[background commands ended]
");
        for n in notices {
            let how = match n.code {
                Some(0) => "exited successfully".to_string(),
                Some(code) => format!("failed with exit code {code}"),
                None => "was killed by a signal".to_string(),
            };
            text.push_str(&format!(
                "- command {} (`{}`) {how} on its own.
",
                n.id, n.command
            ));
            // A command that outgrew the foreground has its whole output on
            // disk; the buffer behind BashOutput keeps only the tail.
            if let Some(file) = &n.file {
                text.push_str(&format!(
                    "  Its whole output is in the file {} — read it with Read.\n",
                    self.ws.display(file)
                ));
            }
        }
        text.push_str("Read what it printed with BashOutput, or start it again with BashRestart.");
        Some(ContentBlock::Text { text })
    }

    /// Something the human said: the point to come back to, the card in the
    /// feed, the line in the journal, and the message the model reads.
    ///
    /// The same whether it opens a turn or joins a running one (§7.4): each
    /// message gets its own point, and `Esc Esc` means "back to before the
    /// last thing I said" either way.
    pub(super) async fn hear(&self, turn_id: &TurnId, text: String, attachments: Vec<String>) -> Message {
        // §13, `Esc Esc`: "roll back to the previous message". The point has
        // to exist before the turn touches anything, and §5.1 hangs its
        // "roll back to before this message" on the same one.
        checkpoints::create(&self.shared, CheckpointReason::Message).await;

        // §5.1, ahead of §15's `TurnStarted` when the message opens a turn:
        // the card the human sees comes from the stream like everything else,
        // so a session replayed from its journal shows the question and not
        // only the answer.
        self.emit(AgentEvent::UserMessage {
            turn_id: turn_id.clone(),
            text: text.clone(),
            attachments: attachments.clone(),
        });

        // §9: what the human typed, before attachments are folded in. The
        // title comes from here, and so does the text §9 searches.
        if let Some(recorder) = &self.shared.recorder {
            recorder.prompt(turn_id, &text);
        }
        self.user_message(text, attachments).await
    }

    /// Into the history, and into the file the history is reopened from (§9).
    ///
    /// One place, because the two must not diverge: a session whose journal is
    /// missing a `tool_result` reopens into a conversation the provider
    /// rejects outright.
    pub(super) fn remember(&mut self, turn_id: &TurnId, message: Message) {
        if let Some(recorder) = &self.shared.recorder {
            recorder.message(turn_id, &message);
        }
        self.history.push(message);
    }

    async fn user_message(&self, text: String, attachments: Vec<String>) -> Message {
        // §7.3: what the human typed is a name, and what the model gets is the
        // prompt the project keeps under it — the same split the attachments
        // below make. The feed and the journal keep the typed line, so a
        // reopened session shows `/review` and not what it stood for. A skill
        // answers to `/name` the same way, after the commands. An MCP server's
        // prompt is `/mcp__server__prompt`, a name nothing else can have.
        let text = match self.shared.mcp.expand_prompt(&text).await {
            Some(Ok(expanded)) => expanded,
            Some(Err(why)) => {
                self.emit(AgentEvent::Error {
                    code: "mcp_prompt".into(),
                    message: why.clone(),
                    retryable: false,
                });
                format!("{text}

[The MCP prompt this names could not be fetched: {why}]")
            }
            None => slash::expand(self.ws.home(), &text)
                .or_else(|| slash::expand_in(&self.shared.plugins.commands, &text))
                .or_else(|| crate::skills::activate::expand(&self.shared.skills, &text))
                .unwrap_or(text),
        };

        if attachments.is_empty() {
            return Message::user_text(text);
        }

        let mut head = String::from("The user attached files:\n");
        let mut pictures = Vec::new();
        for raw in &attachments {
            let Some(path) = self.attached(raw) else {
                head.push_str(&format!("- {raw} (not found)\n"));
                continue;
            };
            let name = self.ws.display(&path);
            if !picture::is_picture(&path) {
                // §7.2: an attached file counts as read.
                self.ws.mark_read(&path);
                head.push_str(&format!("- {name}\n"));
                continue;
            }
            // An attached image is the image itself, not a path to go and
            // open: the human put it in front of the model by attaching it.
            match tokio::task::spawn_blocking(move || picture::load(&path)).await {
                Ok(Ok(picture)) => {
                    head.push_str(&format!("- {name} (image, shown below)\n"));
                    pictures.push((name, picture));
                }
                Ok(Err(err)) => head.push_str(&format!("- {name} ({err})\n")),
                Err(_) => head.push_str(&format!("- {name} (image could not be read)\n")),
            }
        }
        head.push('\n');

        let mut content = vec![ContentBlock::Text { text: head + &text }];
        for (name, picture) in pictures {
            content.extend(ContentBlock::picture(&name, picture));
        }
        Message {
            role: Role::User,
            content,
        }
    }

    /// Where an attachment is.
    ///
    /// Inside the project for a file, as for everything the agent reads
    /// (§8). An image may be anywhere: a screenshot lives on the desktop or
    /// in a temp folder, and the human pointing at it is the permission. The
    /// boundary is still held for what the agent reads on its own —
    /// `ReadImage` stays inside the project.
    fn attached(&self, raw: &str) -> Option<PathBuf> {
        if let Ok(path) = self.ws.resolve_existing(raw) {
            return Some(path);
        }
        let path = Path::new(raw);
        (path.is_absolute() && picture::is_picture(path)).then(|| path.to_path_buf())
    }

    /// Takes the MCP servers' tools as they are now, if anything about the
    /// servers changed since they were last taken.
    ///
    /// Once per turn and not per request: the tools and the system prompt are
    /// the cached head of every request, and a server that reports a changed
    /// list mid-turn would move it between two steps of one piece of work. A
    /// server that dies mid-turn keeps its tools until the next one, and a call
    /// to it fails with the reason.
    fn take_mcp(&mut self) {
        let now = self.shared.mcp.generation();
        if self.mcp_seen == Some(now) {
            return;
        }
        self.mcp_seen = Some(now);
        let mut more = self.shared.mcp.tools();
        // Offered only where there is a server to ask: a tool that can only
        // answer "no servers" is a line in every request for nothing.
        if !self.shared.lsp.names().is_empty() {
            more.push(Arc::new(crate::tool::lsp::LspQuery));
            more.push(Arc::new(crate::tool::rename::Rename));
        }
        self.registry = Arc::new(self.shared.tools(more));
        self.mcp_notes = self.shared.mcp.prompt_section().map(Arc::from);
    }

    fn build_request(&mut self, turn_id: &TurnId) -> ProviderRequest {
        let state = self.shared.state.lock().unwrap().clone();
        let info = self.shared.model_info(&state.model);
        let intent = state.mode.intent();
        let (system, tools) = self.head(intent);
        // Before the history is copied: a prefix that moved takes the
        // reasoning bound to the old one with it.
        self.rebind(turn_id, binding::fingerprint(&system, &tools));
        // Whatever the history holds, the request must not carry a call with
        // no answer: the provider would refuse every request from here on
        // (`message::answer_dangling_calls`). In the copy, so the journal and
        // the history stay as they were written.
        let mut messages = self.shared.shown_to(&info, &self.history);
        crate::message::answer_dangling_calls(&mut messages);
        ProviderRequest {
            model: state.model,
            system,
            messages,
            tools,
            max_tokens: self.answer_room(),
            // Old models know neither `effort` nor adaptive reasoning;
            // sending it to them gets a 400 instead of an answer. There are two flags,
            // because the provider reports them separately: there is a model
            // that reasons but does not understand effort.
            effort: info.supports_effort.then_some(state.effort),
            thinking: info.supports_thinking,
        }
    }

    /// The ceiling on one answer: the model's own, or ours if lower. The
    /// window has to keep this much free for it (`compaction`).
    pub(super) fn answer_room(&self) -> u32 {
        let model = self.shared.state.lock().unwrap().model.clone();
        self.shared.model_info(&model).max_output.min(MAX_OUTPUT_TOKENS)
    }

    pub(super) async fn open_stream(
        &self,
        request: ProviderRequest,
        cancel: &CancellationToken,
    ) -> Result<ProviderStream, ProviderError> {
        self.shared.open(self.provider.as_ref(), request, cancel).await
    }

    /// The agent as the one making calls (`calls`).
    fn caller(&self) -> Caller {
        Caller {
            shared: self.shared.clone(),
            registry: self.registry.clone(),
            ws: self.ws.clone(),
            provider: self.provider.clone(),
            instructions: self.instructions.clone(),
            memory: self.memory.clone(),
            skills: self.skills.clone(),
            mcp_notes: self.mcp_notes.clone(),
            under: None,
            model: ModelPick::Default,
        }
    }

    /// Counts what the subagents of the last batch spent.
    ///
    /// Here and not where it was spent: the meter is the worker's alone, and
    /// the calls of a batch run side by side on a shared borrow of it. Aside
    /// from the ring, like the summarizer: a subagent's window is not the
    /// agent's, and the ring shows the agent's.
    pub(super) fn settle_subagents(&mut self) {
        let spent = self.shared.ledger.take();
        if spent.is_empty() {
            return;
        }
        for (counts, pricing) in spent {
            self.usage.record_aside(counts, pricing);
        }
        self.emit(AgentEvent::UsageUpdated {
            usage: self.usage_snapshot(),
        });
    }

    /// The turn used all its steps. Ended with the error block rather than
    /// quietly: a feed that just stops looks like the model finished.
    ///
    /// The history ends with the last batch's results, which is a state the
    /// next message handles like it does after a stop.
    fn stop_at_ceiling(&self, turn_id: TurnId, max_steps: u32) {
        self.emit(AgentEvent::Error {
            code: "step_limit".to_string(),
            message: format!(
                "Turn stopped: limit of {max_steps} steps reached. Say \"continue\" \
                 to give the model another turn, or change `maxSteps` in .yaca/settings.json."
            ),
            retryable: false,
        });
        self.emit(AgentEvent::TurnFinished {
            turn_id,
            stop_reason: StopReason::Failed,
            at: TsMillis::now(),
        });
    }

    /// The model ended its response without saying anything. Ended with the
    /// error block rather than quietly, for the same reason as the ceiling.
    fn stop_silent(&self, turn_id: TurnId, stop_reason: StopReason) {
        let why = match stop_reason {
            StopReason::MaxTokens => {
                "used its whole output budget on reasoning and wrote no answer"
            }
            StopReason::Refusal => "refused and wrote nothing",
            _ => "ended its turn after reasoning, with no answer and no tool call",
        };
        self.emit(AgentEvent::Error {
            code: "empty_answer".to_string(),
            message: format!("The model {why}. Say \"continue\" to ask again."),
            retryable: true,
        });
        self.emit(AgentEvent::TurnFinished {
            turn_id,
            stop_reason: StopReason::Failed,
            at: TsMillis::now(),
        });
    }

    fn fail_turn(&self, turn_id: TurnId, err: ProviderError) {
        self.emit(AgentEvent::Error {
            code: err.code(),
            message: err.to_string(),
            retryable: err.is_retryable(),
        });
        // The state ring goes out on `TurnFinished`, so it must
        // arrive on a failed turn too — otherwise the interface lies about the state.
        self.emit(AgentEvent::TurnFinished {
            turn_id,
            stop_reason: StopReason::Failed,
            at: TsMillis::now(),
        });
    }
}
