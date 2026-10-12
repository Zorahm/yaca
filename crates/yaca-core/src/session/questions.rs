//! The road a question takes from a tool to the screen and back.
//!
//! A permission is asked by the caller before its tool runs, because the
//! answer decides whether the tool runs at all. A question is asked by the tool
//! itself, in the middle of its run, because its answer *is* the result the
//! tool returns. So the tool gets this handle through its context, and the
//! handle does what the caller does for a permission: emit through the shared
//! sink (so the question reaches the journal), wait on the broker, and close
//! the question on every road out.
//!
//! Concurrency is not decided here. The tool is not `concurrent`, so it holds
//! the session's lane while it waits, exactly as a permission question does:
//! the human answers one thing at a time, and nothing else that writes may run
//! beside it.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::error::ToolError;
use crate::event::AgentEvent;
use crate::id::{CallId, RequestId};
use crate::question::{Question, QuestionReply};

use super::Shared;

/// A question the tool may ask, bound to the call that asks it.
#[derive(Clone)]
pub struct Questions {
    shared: Arc<Shared>,
    call_id: CallId,
}

impl Questions {
    pub(super) fn new(shared: Arc<Shared>, call_id: CallId) -> Self {
        Questions { shared, call_id }
    }

    /// Puts the question on screen and waits for the human.
    ///
    /// There is no timeout (§5.7): the question stays up until it is answered,
    /// dismissed, or the turn is cancelled. A cancelled turn gives
    /// [`ToolError::Cancelled`], which the caller turns into a stopped call, the
    /// same as a stopped permission question.
    pub async fn ask(
        &self,
        question: Question,
        cancel: &CancellationToken,
    ) -> Result<QuestionReply, ToolError> {
        let id = RequestId::new();
        let request = question.into_request(id.clone(), self.call_id.clone());
        let rx = self.shared.questions.open(id.clone(), request.clone());
        self.shared.emit(AgentEvent::QuestionAsked { request });

        let reply = tokio::select! {
            _ = cancel.cancelled() => {
                self.shared.questions.forget(&id);
                None
            }
            // A closed channel is what `abandon_all` leaves behind: the turn
            // was cancelled from the dispatcher, which is the same outcome.
            received = rx => received.ok(),
        };

        self.shared.emit(AgentEvent::QuestionResolved {
            request_id: id,
            reply: reply.clone(),
        });
        reply.ok_or(ToolError::Cancelled)
    }
}
