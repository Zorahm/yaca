//! The question before a call (§5.7): whether to ask, asking, and counting.
//!
//! Apart from `calls` because it is a decision of its own with rules of its
//! own — standing rules before the mode, a refusal before bypass — and running
//! a call only needs its answer.

use tokio_util::sync::CancellationToken;

use crate::event::AgentEvent;
use crate::id::{CallId, RequestId};
use crate::permission::broker::Standing;
use crate::permission::policy::Verdict;
use crate::permission::{PermissionAnswer, PermissionNeed, PermissionRequest};
use crate::tool::take_description;

use super::accumulator::PendingCall;
use super::calls::Caller;

/// Место вызова в пачке — для счётчика `2 из 3` (§5.7).
#[derive(Clone, Copy)]
pub(super) struct Queue<'a> {
    pub(super) asked: u32,
    pub(super) rest: &'a [PendingCall],
}

/// What happens to a call before it runs.
pub(super) enum Gate {
    Pass,
    Ask,
    /// A standing refusal covers it; the note goes to the agent.
    Refused(Option<String>),
}

pub(super) enum Answer {
    Allowed,
    /// `standing`: the human refused every call like this one, not only this
    /// one, and the model has to hear that.
    Denied {
        reason: Option<String>,
        standing: bool,
    },
    Cancelled,
}

impl Caller {
    /// Нужно ли спрашивать про этот вызов прямо сейчас.
    ///
    /// Standing rules come before the mode, so a refusal holds under bypass
    /// too: bypass means "stop asking me", not "do what I said no to".
    pub(super) fn gate(&self, need: &PermissionNeed) -> Gate {
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
    pub(super) async fn ask(
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
