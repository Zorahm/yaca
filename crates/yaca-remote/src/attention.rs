//! Which events want the human, and the watcher that sends them to the phone.
//!
//! The phone notifies about these whether or not it is attached to the chat: a
//! permission asked in a chat nobody is looking at is exactly the thing that must
//! reach the human.

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;
use yaca_core::event::AgentEvent;

use crate::host::LiveChat;
use crate::protocol::{Attention, ServerMessage};
use crate::writer::Outbox;

/// The reason an event asks the human to come back, if it does.
pub(crate) fn of(event: &AgentEvent) -> Option<Attention> {
    match event {
        AgentEvent::PermissionRequested { request } => Some(Attention::Permission {
            request: request.id.clone(),
            target: request.target.clone(),
        }),
        AgentEvent::QuestionAsked { request } => Some(Attention::Question {
            text: request.question.clone(),
        }),
        AgentEvent::ElicitationRequested { .. } => Some(Attention::Elicitation),
        AgentEvent::TurnFinished { stop_reason, .. } => Some(Attention::TurnFinished {
            stop_reason: *stop_reason,
        }),
        _ => None,
    }
}

/// Sends the attention of one chat until it is stopped or the chat ends.
pub(crate) async fn watch(
    chat: LiveChat,
    mut events: Receiver<AgentEvent>,
    outbox: Outbox,
    stop: CancellationToken,
) {
    loop {
        tokio::select! {
            () = stop.cancelled() => break,
            received = events.recv() => match received {
                Ok(event) => {
                    let Some(reason) = of(&event) else { continue };
                    let message = ServerMessage::Attention {
                        session: chat.handle.id().clone(),
                        project: chat.project.clone(),
                        title: chat.handle.meta().map(|meta| meta.title),
                        reason,
                    };
                    if !outbox.send(message).await {
                        break;
                    }
                }
                // Losing an event to lag is better than waiting on a backlog that can
                // never catch up; the next attention event still gets through.
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use yaca_core::event::{StopReason, TsMillis};
    use yaca_core::id::{CallId, RequestId, TurnId};
    use yaca_core::mcp::ElicitationRequest;
    use yaca_core::permission::{PermissionKind, PermissionRequest};
    use yaca_core::question::QuestionRequest;

    use super::*;

    fn permission(target: &str) -> PermissionRequest {
        PermissionRequest {
            id: RequestId::new(),
            call_id: CallId::new(),
            kind: PermissionKind::RunCommand,
            target: target.into(),
            detail: format!("{target} --force"),
            note: None,
            size_bytes: None,
            files: None,
            invariant: None,
            unsaved: None,
            server: None,
            scopes: Vec::new(),
            position: 1,
            total: 1,
        }
    }

    #[test]
    fn a_permission_request_asks_with_its_target() {
        let request = permission("git");
        let id = request.id.clone();
        let event = AgentEvent::PermissionRequested { request };
        assert_eq!(
            of(&event),
            Some(Attention::Permission {
                request: id,
                target: "git".into()
            })
        );
    }

    #[test]
    fn a_question_asks_with_its_text() {
        let event = AgentEvent::QuestionAsked {
            request: QuestionRequest {
                id: RequestId::new(),
                call_id: CallId::new(),
                question: "Which branch?".into(),
                options: Vec::new(),
                allow_other: true,
                multi_select: false,
            },
        };
        assert_eq!(
            of(&event),
            Some(Attention::Question {
                text: "Which branch?".into()
            })
        );
    }

    #[test]
    fn an_elicitation_asks_without_detail() {
        let event = AgentEvent::ElicitationRequested {
            request: ElicitationRequest {
                id: RequestId::new(),
                server: "forms".into(),
                message: "Fill in the form".into(),
                fields: Vec::new(),
            },
        };
        assert_eq!(of(&event), Some(Attention::Elicitation));
    }

    #[test]
    fn a_finished_turn_asks_with_its_stop_reason() {
        let event = AgentEvent::TurnFinished {
            turn_id: TurnId::new(),
            stop_reason: StopReason::MaxTokens,
            at: TsMillis(0),
        };
        assert_eq!(
            of(&event),
            Some(Attention::TurnFinished {
                stop_reason: StopReason::MaxTokens
            })
        );
    }

    #[test]
    fn everything_else_is_not_attention() {
        let event = AgentEvent::TurnStarted {
            turn_id: TurnId::new(),
            at: TsMillis(0),
        };
        assert_eq!(of(&event), None);
        let event = AgentEvent::PermissionResolved {
            request_id: RequestId::new(),
        };
        assert_eq!(of(&event), None);
    }
}
