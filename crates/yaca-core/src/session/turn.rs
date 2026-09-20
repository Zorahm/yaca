//! Ход агента: от сообщения человека до остановки.
//!
//! Работник владеет историей единолично — она никому не отдана под мьютекс, —
//! и потому же живёт отдельной задачей от диспетчера.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::ProviderError;
use crate::event::{AgentEvent, StopReason, TsMillis};
use crate::id::TurnId;
use crate::instructions::Instructions;
use crate::message::Message;
use crate::provider::{Provider, ProviderRequest, ProviderStream};
use crate::slash;
use crate::usage::UsageMeter;
use crate::workspace::Workspace;

use crate::checkpoint::CheckpointReason;

use super::accumulator::Accumulator;
use super::binding::{self, Fingerprint};
use super::inbox::Inbox;
use super::{checkpoints, compact, prompt, Job, Shared, ToolRegistry};

/// Потолок на один ответ. Стриминг снимает вопрос таймаутов, но окно
/// контекста конечно, и давать модели все 128k на реплику незачем.
const MAX_OUTPUT_TOKENS: u32 = 64_000;
const MAX_ATTEMPTS: u32 = 5;

pub(super) struct Worker {
    pub(super) ws: Arc<Workspace>,
    pub(super) provider: Arc<dyn Provider>,
    pub(super) registry: ToolRegistry,
    /// The project's `AGENTS.md`, read once when the session starts. The
    /// system prompt is the cached prefix of every request, and a file read per
    /// message would move that prefix on every edit.
    pub(super) instructions: Option<Instructions>,
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

    /// Какие тулы вообще существуют в текущем режиме (§7.1).
    pub(super) fn intent(&self) -> crate::intent::Intent {
        self.shared.state.lock().unwrap().mode.intent()
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

            let results = self.run_batch(&turn_id, &done.calls, &cancel).await;
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
        self.user_message(text, attachments)
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

    fn user_message(&self, text: String, attachments: Vec<String>) -> Message {
        // §7.3: what the human typed is a name, and what the model gets is the
        // prompt the project keeps under it — the same split the attachments
        // below make. The feed and the journal keep the typed line, so a
        // reopened session shows `/review` and not what it stood for.
        let text = slash::expand(self.ws.root(), &text).unwrap_or(text);

        if attachments.is_empty() {
            return Message::user_text(text);
        }

        let mut head = String::from("Человек приложил файлы:\n");
        for raw in &attachments {
            match self.ws.resolve_existing(raw) {
                Ok(path) => {
                    // §7.2: приложенный файл считается прочитанным.
                    self.ws.mark_read(&path);
                    head.push_str(&format!("- {}\n", self.ws.display(&path)));
                }
                Err(_) => head.push_str(&format!("- {raw} (не найден)\n")),
            }
        }
        head.push('\n');
        Message::user_text(head + &text)
    }

    fn build_request(&mut self, turn_id: &TurnId) -> ProviderRequest {
        let state = self.shared.state.lock().unwrap().clone();
        let info = self.shared.model_info(&state.model);
        let intent = state.mode.intent();
        let system = prompt::build(self.ws.root(), intent, self.instructions.as_ref());
        let tools = self.registry.schemas(intent);
        // Before the history is copied: a prefix that moved takes the
        // reasoning bound to the old one with it.
        self.rebind(turn_id, binding::fingerprint(&system, &tools));
        ProviderRequest {
            model: state.model,
            system,
            messages: self.history.clone(),
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
        let mut attempt = 1;
        loop {
            match self.provider.stream(request.clone()).await {
                Ok(stream) => return Ok(stream),
                Err(err) if err.is_retryable() && attempt < MAX_ATTEMPTS => {
                    let delay = backoff(attempt, retry_after(&err));
                    self.emit(AgentEvent::RetryScheduled {
                        attempt: attempt + 1,
                        of: MAX_ATTEMPTS,
                        delay_ms: delay.as_millis() as u32,
                        reason: err.to_string(),
                    });
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = cancel.cancelled() => return Err(err),
                    }
                    attempt += 1;
                }
                Err(err) => return Err(err),
            }
        }
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

fn retry_after(err: &ProviderError) -> Option<Duration> {
    match err {
        ProviderError::Http { retry_after_s, .. } => retry_after_s.map(Duration::from_secs),
        _ => None,
    }
}

/// Провайдер сказал, когда возвращаться, — слушаемся. Иначе удвоение
/// с потолком: пять попыток укладываются примерно в полминуты.
fn backoff(attempt: u32, hinted: Option<Duration>) -> Duration {
    if let Some(d) = hinted {
        return d.min(Duration::from_secs(60));
    }
    Duration::from_millis(500 * 2u64.pow(attempt.min(6) - 1)).min(Duration::from_secs(30))
}
