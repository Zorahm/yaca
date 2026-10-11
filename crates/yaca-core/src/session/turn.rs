//! Ход агента: от сообщения человека до остановки.
//!
//! Работник владеет историей единолично — она никому не отдана под мьютекс, —
//! и потому же живёт отдельной задачей от диспетчера.

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
use crate::usage::UsageMeter;
use crate::workspace::Workspace;

use crate::checkpoint::CheckpointReason;

use super::accumulator::Accumulator;
use super::calls::Caller;
use super::binding::{self, Fingerprint};
use super::inbox::Inbox;
use super::{checkpoints, compact, prompt, Job, Shared, ToolRegistry};

/// Потолок на один ответ. Стриминг снимает вопрос таймаутов, но окно
/// контекста конечно, и давать модели все 128k на реплику незачем.
const MAX_OUTPUT_TOKENS: u32 = 64_000;

pub(super) struct Worker {
    pub(super) ws: Arc<Workspace>,
    pub(super) provider: Arc<dyn Provider>,
    pub(super) registry: Arc<ToolRegistry>,
    /// The project's `AGENTS.md`, read once when the session starts. The
    /// system prompt is the cached prefix of every request, and a file read per
    /// message would move that prefix on every edit.
    pub(super) instructions: Option<Arc<Instructions>>,
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
        let mut inbox = Inbox::new(rx);
        while let Some(job) = inbox.next().await {
            let token = CancellationToken::new();
            *self.shared.cancel.lock().unwrap() = token.clone();

            self.set_busy(true);
            match job {
                Job::Message { text, attachments } => {
                    self.run_turn(text, attachments, token, &mut inbox).await;
                }
                Job::Compact => {
                    self.compact(&token, None).await;
                }
                Job::EditSummary { text } => self.edit_summary(text),
            }

            self.set_busy(false);
        }
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
        let opening = self.hear(&turn_id, text, attachments).await;
        self.emit(AgentEvent::TurnStarted {
            turn_id: turn_id.clone(),
            at: TsMillis::now(),
        });
        self.remember(&turn_id, opening);

        // Set once a request refused as too long has been compacted and sent
        // again, until one gets through.
        let mut rescued = false;
        loop {
            if cancel.is_cancelled() {
                self.emit(AgentEvent::TurnCancelled { turn_id });
                return;
            }

            // Before every request, not only before the human's message: a
            // turn of many calls fills the window between its steps (§6.1.1).
            self.compact_if_full(&turn_id, &cancel).await;

            let request = self.build_request(&turn_id);
            let mut stream = match self.open_stream(request, &cancel).await {
                Ok(s) => s,
                // The provider measured what the estimate did not. Compacted
                // and sent again, once: refused again, what is left does not
                // fit either, and the provider's words are the answer.
                Err(err) if err.is_context_overflow() && !rescued => {
                    rescued = true;
                    if compact::plan(&self.history).is_some()
                        && self.compact(&cancel, Some(&turn_id)).await
                    {
                        continue;
                    }
                    return self.fail_turn(turn_id, err);
                }
                Err(err) => return self.fail_turn(turn_id, err),
            };
            rescued = false;

            let mut acc = Accumulator::new(turn_id.clone());
            loop {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        self.emit(AgentEvent::TurnCancelled { turn_id });
                        return;
                    }
                    item = stream.next() => match item {
                        None => break,
                        Some(Ok(event)) => {
                            for out in acc.push(event) {
                                self.emit(out);
                            }
                        }
                        Some(Err(err)) => return self.fail_turn(turn_id, err),
                    },
                }
            }

            let done = acc.finish();

            let info = self
                .shared
                .model_info(&self.shared.state.lock().unwrap().model);
            self.usage.record(done.usage, info.pricing);
            self.emit(AgentEvent::UsageUpdated {
                usage: self.usage.snapshot(info.context_max),
            });

            if !done.blocks.is_empty() {
                self.remember(&turn_id, Message::assistant(done.blocks));
            }
            // The figure just recorded covers everything sent and answered.
            self.measured = self.history.len();

            // Calls run whatever the stop reason says. A `tool_use` left
            // without its result makes the provider reject this conversation
            // on every request from here on, and the journal carries the hole
            // into a reopened session. Local OpenAI-compatible servers end a
            // message of calls with "stop", and a response cut at the output
            // cap ends with its last call half-written — that one is answered
            // with an error rather than run (`calls::garbled`).
            if done.calls.is_empty() {
                self.emit(AgentEvent::TurnFinished {
                    turn_id,
                    stop_reason: done.stop_reason,
                });
                return;
            }

            let results = self
                .caller()
                .run_batch(&turn_id, &done.calls, &cancel)
                .await;
            self.settle_subagents();
            // Все результаты одним сообщением: разложить их по нескольким —
            // тихо отучить модель звать тулы параллельно.
            let mut message = Message::tool_results(results);

            // §7.4: what the human sent meanwhile joins here, after the batch
            // and before the model's next step, in the same message — a
            // second user message in a row is one the provider may refuse.
            // Not after a cancel: the model is asked nothing more in this
            // turn, so the message would sit in the history unanswered; it
            // stays queued and gets a turn of its own.
            if !cancel.is_cancelled() {
                for queued in inbox.messages() {
                    let said = self.hear(&turn_id, queued.text, queued.attachments).await;
                    message.content.extend(said.content);
                }
            }
            self.remember(&turn_id, message);
        }
    }

    /// Something the human said: the point to come back to, the card in the
    /// feed, the line in the journal, and the message the model reads.
    ///
    /// The same whether it opens a turn or joins a running one (§7.4): each
    /// message gets its own point, and `Esc Esc` means "back to before the
    /// last thing I said" either way.
    async fn hear(&self, turn_id: &TurnId, text: String, attachments: Vec<String>) -> Message {
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
    fn remember(&mut self, turn_id: &TurnId, message: Message) {
        if let Some(recorder) = &self.shared.recorder {
            recorder.message(turn_id, &message);
        }
        self.history.push(message);
    }

    async fn user_message(&self, text: String, attachments: Vec<String>) -> Message {
        // §7.3: what the human typed is a name, and what the model gets is the
        // prompt the project keeps under it — the same split the attachments
        // below make. The feed and the journal keep the typed line, so a
        // reopened session shows `/review` and not what it stood for.
        let text = slash::expand(self.ws.root(), &text).unwrap_or(text);

        if attachments.is_empty() {
            return Message::user_text(text);
        }

        let mut head = String::from("Человек приложил файлы:\n");
        let mut pictures = Vec::new();
        for raw in &attachments {
            let Some(path) = self.attached(raw) else {
                head.push_str(&format!("- {raw} (не найден)\n"));
                continue;
            };
            let name = self.ws.display(&path);
            if !picture::is_picture(&path) {
                // §7.2: приложенный файл считается прочитанным.
                self.ws.mark_read(&path);
                head.push_str(&format!("- {name}\n"));
                continue;
            }
            // An attached image is the image itself, not a path to go and
            // open: the human put it in front of the model by attaching it.
            match tokio::task::spawn_blocking(move || picture::load(&path)).await {
                Ok(Ok(picture)) => {
                    head.push_str(&format!("- {name} (изображение, показано ниже)\n"));
                    pictures.push((name, picture));
                }
                Ok(Err(err)) => head.push_str(&format!("- {name} ({err})\n")),
                Err(_) => head.push_str(&format!("- {name} (изображение не прочитано)\n")),
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

    fn build_request(&mut self, turn_id: &TurnId) -> ProviderRequest {
        let state = self.shared.state.lock().unwrap().clone();
        let info = self.shared.model_info(&state.model);
        let intent = state.mode.intent();
        let system = prompt::build(
            self.ws.root(),
            intent,
            self.instructions.as_deref(),
            self.shared.shell,
        );
        let tools = self.registry.schemas(intent);
        // Before the history is copied: a prefix that moved takes the
        // reasoning bound to the old one with it.
        self.rebind(turn_id, binding::fingerprint(&system, &tools));
        ProviderRequest {
            model: state.model,
            system,
            messages: self.shared.shown_to(&info, &self.history),
            tools,
            max_tokens: self.answer_room(),
            // Старые модели не знают ни `effort`, ни адаптивного рассуждения;
            // отправить им это — получить 400 вместо ответа. Флага два,
            // потому что провайдер сообщает их отдельно: бывает модель,
            // которая рассуждает, но усилия не понимает.
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
            under: None,
        }
    }

    /// Counts what the subagents of the last batch spent.
    ///
    /// Here and not where it was spent: the meter is the worker's alone, and
    /// the calls of a batch run side by side on a shared borrow of it. Aside
    /// from the ring, like the summarizer: a subagent's window is not the
    /// agent's, and the ring shows the agent's.
    fn settle_subagents(&mut self) {
        let spent = self.shared.ledger.take();
        if spent.is_empty() {
            return;
        }
        for (counts, pricing) in spent {
            self.usage.record_aside(counts, pricing);
        }
        self.emit(AgentEvent::UsageUpdated {
            usage: self.usage.snapshot(self.context_max()),
        });
    }

    fn fail_turn(&self, turn_id: TurnId, err: ProviderError) {
        self.emit(AgentEvent::Error {
            code: err.code(),
            message: err.to_string(),
            retryable: err.is_retryable(),
        });
        // Кольцо состояния гаснет по `TurnFinished`, поэтому оно обязано
        // прийти и на неудачном ходу — иначе интерфейс врёт про состояние.
        self.emit(AgentEvent::TurnFinished {
            turn_id,
            stop_reason: StopReason::Failed,
        });
    }
}
