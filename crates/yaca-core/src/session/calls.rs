//! Выполнение тулов: от вопроса о разрешении до строки в ленте.
//!
//! Разрешение спрашивается до запуска, а не вокруг него: §5.7 существует ровно
//! затем, чтобы человек решил раньше, чем что-либо произошло.
//!
//! Вызовов в одном ответе модели бывает несколько, и просит она их пачкой ровно
//! затем, чтобы они шли разом. Кто может идти разом, решает `Tool::concurrent`;
//! остальное — по очереди. См. [`Worker::run_batch`].

use tokio_util::sync::CancellationToken;

use crate::error::ToolError;
use crate::event::{AgentEvent, ToolMeta};
use crate::id::{CallId, RequestId, TurnId};
use crate::message::ContentBlock;
use crate::permission::broker::Standing;
use crate::permission::policy::Verdict;
use crate::permission::{PermissionAnswer, PermissionNeed, PermissionRequest};
use crate::tool::{take_description, Progress, ToolCtx};

use super::accumulator::{Garbled, PendingCall};
use super::checkpoints;
use super::turn::Worker;

/// What the model is told when the human stops a call.
///
/// English, like the system prompt, and blunt on purpose. A model that reads
/// only "cancelled" treats it as a transient failure and tries the same thing
/// again; the point of stopping was that the person wants a word first.
const STOPPED_BY_HUMAN: &str = "The user doesn't want to proceed with this tool use. \
     The tool use was rejected: nothing it would have done was done. STOP what you are \
     doing and wait for the user to tell you how to proceed. Their next message may \
     explain what went wrong or how they would rather you worked — read it closely.";

/// Место вызова в пачке — для счётчика `2 из 3` (§5.7).
#[derive(Clone, Copy)]
pub(super) struct Queue<'a> {
    pub(super) asked: u32,
    pub(super) rest: &'a [PendingCall],
}

/// What happens to a call before it runs.
enum Gate {
    Pass,
    Ask,
    /// A standing refusal covers it; the note goes to the agent.
    Refused(Option<String>),
}

enum Answer {
    Allowed,
    /// `standing`: the human refused every call like this one, not only this
    /// one, and the model has to hear that.
    Denied {
        reason: Option<String>,
        standing: bool,
    },
    Cancelled,
}

impl Worker {
    /// Пачка вызовов из одного ответа модели (§5.4).
    ///
    /// Подряд идущие вызовы, помеченные [`Tool::concurrent`], уходят работать
    /// вместе; остальные — по очереди, потому что порядок им нужен: чекпоинт
    /// снимается перед командой, вопрос §5.7 показывается по одному, правка
    /// ложится поверх предыдущей.
    ///
    /// Одновременно — только подряд идущие. Пустить читающие вперёд их соседей
    /// значило бы переставить ход, который модель написала в своём порядке.
    ///
    /// Результаты собираются обратно в исходном порядке. Связывает их
    /// `tool_use_id`, но порядок — то, в чём модель читает собственный ход.
    ///
    /// [`Tool::concurrent`]: crate::tool::Tool::concurrent
    pub(super) async fn run_batch(
        &self,
        turn_id: &TurnId,
        calls: &[PendingCall],
        cancel: &CancellationToken,
    ) -> Vec<ContentBlock> {
        let mut done: Vec<(usize, ContentBlock)> = Vec::with_capacity(calls.len());
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
                    let (result, _) = self
                        .run_call(turn_id, calls[index].clone(), cancel, queue)
                        .await;
                    (index, result)
                });
                // Ни один из них не спрашивает разрешения — иначе они бы сюда
                // не попали, — поэтому `asked` не двигается.
                done.extend(futures_util::future::join_all(together).await);
                at += run;
            } else {
                let queue = Queue {
                    asked,
                    rest: &calls[at + 1..],
                };
                let (result, did_ask) = self
                    .run_call(turn_id, calls[at].clone(), cancel, queue)
                    .await;
                asked += u32::from(did_ask);
                done.push((at, result));
                at += 1;
            }
        }

        done.sort_by_key(|(index, _)| *index);
        done.into_iter().map(|(_, result)| result).collect()
    }

    /// Сколько вызовов с начала списка можно пустить одновременно.
    ///
    /// Неизвестный тул — не одновременный: про него ничего не известно, а
    /// ответит на него [`Worker::run_call`] строкой об этом.
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

    pub(super) async fn run_call(
        &self,
        turn_id: &TurnId,
        call: PendingCall,
        cancel: &CancellationToken,
        queue: Queue<'_>,
    ) -> (ContentBlock, bool) {
        let call_id = CallId::from(call.id.clone());
        let mut input = call.input;
        // Подпись — забота ленты, модели она обратно не нужна.
        let description = take_description(&mut input);

        // Arguments that never arrived whole: there is nothing to run, but
        // the call is in the history and is answered like any other.
        if let Some(kind) = call.garbled {
            let err = garbled(kind);
            let result = self.failed_unrun(turn_id, call_id, &call.name, description, input, err);
            return (result, false);
        }

        let Some(tool) = self.registry.get(&call.name, self.intent()) else {
            let err = ToolError::failed(format!("неизвестный инструмент: {}", call.name));
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
        });

        let mut asked_here = false;
        if let Some(need) = tool.permission(self.ws.as_ref(), &input) {
            match self.gate(&need) {
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
            events: self.shared.events.clone(),
            progress: Progress::new(call_id.clone(), self.shared.events.clone()),
            cancel: cancel.clone(),
        };

        let started = std::time::Instant::now();
        let outcome = tool.run(&ctx, input).await;
        let duration_ms = started.elapsed().as_millis() as u32;

        match outcome {
            Ok(out) => {
                // Лента получает текст ровно один раз. Стримящий тул уже отдал
                // его по дороге — команда пишет в ленту, пока работает (§0), —
                // остальным его отдаёт отсюда сессия.
                if !ctx.progress.used() && !out.text.is_empty() {
                    self.emit(AgentEvent::ToolCallProgress {
                        call_id: call_id.clone(),
                        chunk: out.text.clone(),
                    });
                }
                // Файловое действие: строка получает счётчики и дифф.
                if let Some(change) = out.changed {
                    self.emit(AgentEvent::FileChanged {
                        call_id: call_id.clone(),
                        fact: change.fact,
                        diff: change.diff,
                    });
                }
                self.emit(AgentEvent::ToolCallFinished {
                    call_id,
                    meta: ToolMeta {
                        duration_ms,
                        ..out.meta
                    },
                });
                (tool_result(&call.id, out.text, false), asked_here)
            }
            Err(ToolError::Rejected(reason)) => {
                // Сработавшая защита — не сбой (§5.9).
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
                // Лента получает строку, модель — строку и подробность:
                // вывод упавшей команды в подписи действия был бы простынёй,
                // а без него модели нечего чинить.
                self.emit(AgentEvent::ToolCallFailed {
                    call_id,
                    message: err.to_string(),
                });
                (tool_result(&call.id, err.for_model(), true), asked_here)
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
        });
        self.emit(AgentEvent::ToolCallFailed {
            call_id,
            message: err.to_string(),
        });
        result
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

    /// Нужно ли спрашивать про этот вызов прямо сейчас.
    ///
    /// Standing rules come before the mode, so a refusal holds under bypass
    /// too: bypass means "stop asking me", not "do what I said no to".
    fn gate(&self, need: &PermissionNeed) -> Gate {
        match self.shared.broker.standing(need) {
            Some(Standing::Refused(refusal)) => Gate::Refused(refusal.reason),
            Some(Standing::Granted) => Gate::Pass,
            None => match self.shared.policy().verdict(need.kind) {
                Verdict::Allow => Gate::Pass,
                Verdict::Ask => Gate::Ask,
            },
        }
    }

    /// Показывает блок §5.7 и ждёт. Таймаута нет — только отмена турна.
    async fn ask(
        &self,
        call_id: &CallId,
        need: &PermissionNeed,
        cancel: &CancellationToken,
        queue: Queue<'_>,
    ) -> Answer {
        let id = RequestId::new();
        let rx = self.shared.broker.open(id.clone());

        let position = queue.asked + 1;
        self.emit(AgentEvent::PermissionRequested {
            request: PermissionRequest {
                id: id.clone(),
                call_id: call_id.clone(),
                kind: need.kind,
                target: need.target.clone(),
                detail: need.detail.clone(),
                note: need.note.clone(),
                size_bytes: need.size_bytes,
                files: need.files,
                scopes: need.scopes(self.ws.root()),
                position,
                total: position + self.still_to_ask(&queue),
            },
        });

        let answer = tokio::select! {
            _ = cancel.cancelled() => {
                self.shared.broker.forget(&id);
                self.emit(AgentEvent::PermissionResolved { request_id: id });
                return Answer::Cancelled;
            }
            received = rx => received,
        };

        self.emit(AgentEvent::PermissionResolved { request_id: id });

        match answer {
            // Правило уже выдал диспетчер — работнику остаётся пропустить.
            Ok(PermissionAnswer::Allow) | Ok(PermissionAnswer::AllowAlways { .. }) => {
                Answer::Allowed
            }
            Ok(PermissionAnswer::Deny { reason }) => Answer::Denied {
                reason,
                standing: false,
            },
            // The rule itself was set by the dispatcher, like a grant.
            Ok(PermissionAnswer::DenyAlways { reason, .. }) => Answer::Denied {
                reason,
                standing: true,
            },
            // Канал закрыт: вопрос отменили вместе с турном.
            Err(_) => Answer::Cancelled,
        }
    }

    /// Сколько ещё вызовов из этой пачки спросят разрешения. Считается
    /// заново перед каждым вопросом: «разрешать всегда» по дороге может
    /// снять часть очереди, и счётчик `2 из 3` обязан это учесть.
    fn still_to_ask(&self, queue: &Queue<'_>) -> u32 {
        queue
            .rest
            .iter()
            .filter(|call| {
                let Some(tool) = self.registry.get(&call.name, self.intent()) else {
                    return false;
                };
                let mut input = call.input.clone();
                let _ = take_description(&mut input);
                let Some(need) = tool.permission(self.ws.as_ref(), &input) else {
                    return false;
                };
                matches!(self.gate(&need), Gate::Ask)
            })
            .count() as u32
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
            "ответ оборвался на лимите, вызов не выполнен",
            "Your response reached its output limit while the arguments of this call \
             were still being written, so it was not run and nothing changed. Make the \
             call again with less in it: write a large file in parts — create it with \
             the first part, then add the rest with Edit — and change an existing file \
             with Edit rather than rewriting it.",
        ),
        Garbled::Invalid => ToolError::failed_with(
            "аргументы вызова — не JSON, вызов не выполнен",
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
