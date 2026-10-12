//! Redrawing the feed of a session that was closed (§9).
//!
//! The journal holds every event the feed was built from, so reopening a
//! session is folding it again. Not all of it, though: some events say what
//! happened, and some say what *was* — and what was is over.
//!
//! [`AgentEvent::ProcessesUpdated`] is the process table of a run whose
//! commands died with it, and its `Остановить` would stop nothing.
//! [`AgentEvent::StateChanged`] is the mode and model the run ended on; the
//! reopened session has its own, and the shell already carries it.
//! [`AgentEvent::CheckpointCreated`] is worse than stale: `.yaca/shadow` keeps
//! what the snapshots hold, but the list of points lives in the session and
//! does not outlive it, so a replayed divider would offer `Откатить сюда` to a
//! point the core no longer knows. §0 has a name for that.
//!
//! Hence the rule: **a replay carries what happened — not what was, and not
//! what can still be done.**
//!
//! Then the tail gets closed. A run that died mid-turn — window killed, machine
//! down — leaves a journal whose last turn never finished, and folding that as
//! it stands gives a feed with a live `●` on a call that stopped hours ago, and
//! a question with buttons nobody is listening to. Both are ended here with the
//! events the core would have sent had anyone been there to stop it.

use crate::event::AgentEvent;
use crate::id::{RequestId, TurnId};

/// The journal of a session, as the feed of a new run may draw it.
pub fn feed(events: Vec<AgentEvent>) -> Vec<AgentEvent> {
    let mut out = Vec::with_capacity(events.len());
    let mut open_turn: Option<TurnId> = None;
    let mut open_request: Option<RequestId> = None;
    let mut open_question: Option<RequestId> = None;

    for event in events {
        match &event {
            // What was, and no longer is.
            AgentEvent::StateChanged { .. }
            | AgentEvent::ProcessesUpdated { .. }
            | AgentEvent::McpUpdated { .. }
            | AgentEvent::LspUpdated { .. }
            | AgentEvent::ProcessOutputReset { .. }
            | AgentEvent::ProcessOutput { .. }
            | AgentEvent::CheckpointCreated { .. }
            | AgentEvent::CheckpointsUpdated { .. }
            | AgentEvent::CheckpointPreviewed { .. } => continue,

            AgentEvent::TurnStarted { turn_id, .. } => open_turn = Some(turn_id.clone()),
            AgentEvent::TurnFinished { turn_id, .. } | AgentEvent::TurnCancelled { turn_id }
                if open_turn.as_ref() == Some(turn_id) =>
            {
                open_turn = None;
            }

            AgentEvent::PermissionRequested { request } => open_request = Some(request.id.clone()),
            AgentEvent::PermissionResolved { request_id }
                if open_request.as_ref() == Some(request_id) =>
            {
                open_request = None;
            }

            AgentEvent::QuestionAsked { request } => open_question = Some(request.id.clone()),
            AgentEvent::QuestionResolved { request_id, .. }
                if open_question.as_ref() == Some(request_id) =>
            {
                open_question = None;
            }

            _ => {}
        }
        out.push(event);
    }

    // The question first: the answer that never came is why the turn stopped,
    // and the block has to be gone before the row under it is marked stopped.
    // A question is closed with no reply, which is the same thing the live run
    // does when the turn is cancelled, so the feed shows it as unanswered.
    if let Some(request_id) = open_question {
        out.push(AgentEvent::QuestionResolved {
            request_id,
            reply: None,
        });
    }
    if let Some(request_id) = open_request {
        out.push(AgentEvent::PermissionResolved { request_id });
    }
    if let Some(turn_id) = open_turn {
        out.push(AgentEvent::TurnCancelled { turn_id });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{CheckpointReason, CheckpointSummary};
    use crate::event::{StopReason, TsMillis};
    use crate::id::{BlockId, CallId, CheckpointId};
    use crate::permission::{PermissionKind, PermissionRequest};

    fn started(turn: &str) -> AgentEvent {
        AgentEvent::TurnStarted {
            turn_id: TurnId::from(turn),
            at: TsMillis(1_700_000_000_000),
        }
    }

    fn finished(turn: &str) -> AgentEvent {
        AgentEvent::TurnFinished {
            turn_id: TurnId::from(turn),
            stop_reason: StopReason::EndTurn,
        }
    }

    fn said(text: &str) -> AgentEvent {
        AgentEvent::TextDelta {
            turn_id: TurnId::from("t1"),
            block_id: BlockId::from("blk_1"),
            text: text.into(),
        }
    }

    fn asked(id: &str) -> AgentEvent {
        AgentEvent::PermissionRequested {
            request: PermissionRequest {
                server: None,
                id: RequestId::from(id),
                call_id: CallId::from("call_1"),
                kind: PermissionKind::RunCommand,
                target: "cargo".into(),
                detail: "cargo test".into(),
                note: None,
                size_bytes: None,
                files: None,
                invariant: None,
                unsaved: None,
                scopes: Vec::new(),
                position: 1,
                total: 1,
            },
        }
    }

    fn snapshot() -> AgentEvent {
        AgentEvent::CheckpointCreated {
            checkpoint: CheckpointSummary {
                id: CheckpointId::from("ckpt_1"),
                at: TsMillis(1_700_000_000_000),
                reason: CheckpointReason::Message,
                tracked: 12,
            },
        }
    }

    #[test]
    fn a_session_that_ended_cleanly_replays_as_it_was_written() {
        let journal = vec![
            AgentEvent::UserMessage {
                turn_id: TurnId::from("t1"),
                text: "привет".into(),
                attachments: Vec::new(),
            },
            started("t1"),
            said("и тебе"),
            finished("t1"),
        ];

        assert_eq!(feed(journal.clone()), journal);
    }

    #[test]
    fn the_process_table_of_a_dead_run_is_left_behind() {
        let journal = vec![
            started("t1"),
            AgentEvent::ProcessesUpdated {
                processes: Vec::new(),
            },
            AgentEvent::ProcessOutput {
                id: crate::process::ProcessId(1),
                chunk: "listening on :5173\n".into(),
            },
            finished("t1"),
        ];

        // The commands died with the run; a panel listing them would offer a
        // `Остановить` that stops nothing.
        assert_eq!(feed(journal), vec![started("t1"), finished("t1")]);
    }

    #[test]
    fn checkpoint_dividers_are_left_behind_because_the_way_back_is_gone() {
        let journal = vec![snapshot(), started("t1"), said("готово"), finished("t1")];

        // The snapshots are still in `.yaca/shadow`, but the list of points is
        // not: the divider would offer `Откатить сюда` to a point the new run
        // has never heard of.
        assert_eq!(
            feed(journal),
            vec![started("t1"), said("готово"), finished("t1")]
        );
    }

    #[test]
    fn a_rollback_that_happened_stays_in_the_feed() {
        let restored = AgentEvent::CheckpointRestored {
            id: CheckpointId::from("ckpt_1"),
            at: TsMillis(1_700_000_000_000),
            files: 3,
        };
        let journal = vec![started("t1"), restored.clone(), finished("t1")];

        // `Откачено к 14:32 · 3 файла` is a thing that happened, not a thing
        // that can still be done.
        assert_eq!(feed(journal), vec![started("t1"), restored, finished("t1")]);
    }

    #[test]
    fn a_turn_that_never_finished_is_closed() {
        let journal = vec![
            started("t1"),
            AgentEvent::ToolCallProgress {
                call_id: CallId::from("call_1"),
                chunk: "…".into(),
            },
        ];

        let replayed = feed(journal);
        assert_eq!(
            replayed.last(),
            Some(&AgentEvent::TurnCancelled {
                turn_id: TurnId::from("t1")
            }),
            "a call left running would keep its live dot forever"
        );
    }

    #[test]
    fn a_turn_that_did_finish_is_not_closed_twice() {
        let replayed = feed(vec![started("t1"), finished("t1")]);
        assert_eq!(replayed, vec![started("t1"), finished("t1")]);
    }

    #[test]
    fn a_question_nobody_answered_is_closed() {
        let replayed = feed(vec![started("t1"), asked("req_1")]);

        assert_eq!(
            replayed[2],
            AgentEvent::PermissionResolved {
                request_id: RequestId::from("req_1")
            },
            "the block would otherwise wait for an answer that cannot arrive"
        );
        assert_eq!(
            replayed[3],
            AgentEvent::TurnCancelled {
                turn_id: TurnId::from("t1")
            }
        );
    }

    fn asked_question(id: &str) -> AgentEvent {
        AgentEvent::QuestionAsked {
            request: crate::question::QuestionRequest {
                id: RequestId::from(id),
                call_id: CallId::from("call_1"),
                question: "Which way?".into(),
                options: Vec::new(),
                allow_other: true,
                multi_select: false,
            },
        }
    }

    #[test]
    fn a_question_nobody_answered_is_closed_without_a_reply() {
        let replayed = feed(vec![started("t1"), asked_question("q_1")]);

        assert_eq!(
            replayed[2],
            AgentEvent::QuestionResolved {
                request_id: RequestId::from("q_1"),
                reply: None,
            },
            "a question with buttons nobody will press would wait forever"
        );
        assert_eq!(
            replayed[3],
            AgentEvent::TurnCancelled {
                turn_id: TurnId::from("t1")
            }
        );
    }

    #[test]
    fn an_answered_ask_user_question_is_not_closed_again() {
        let journal = vec![
            started("t1"),
            asked_question("q_1"),
            AgentEvent::QuestionResolved {
                request_id: RequestId::from("q_1"),
                reply: Some(crate::question::QuestionReply::Dismissed),
            },
            finished("t1"),
        ];

        assert_eq!(feed(journal.clone()), journal);
    }

    #[test]
    fn an_answered_ask_user_question_gains_no_second_resolution() {
        let answered = AgentEvent::QuestionResolved {
            request_id: RequestId::from("q_1"),
            reply: Some(crate::question::QuestionReply::Dismissed),
        };
        let journal = vec![started("t1"), asked_question("q_1"), answered.clone()];

        let replayed = feed(journal);
        assert_eq!(
            replayed.iter().filter(|e| **e == answered).count(),
            1,
            "an answered question must not gain a second resolution"
        );
    }

    #[test]
    fn a_question_that_was_answered_is_not_closed_again() {
        let journal = vec![
            started("t1"),
            asked("req_1"),
            AgentEvent::PermissionResolved {
                request_id: RequestId::from("req_1"),
            },
            finished("t1"),
        ];

        assert_eq!(feed(journal.clone()), journal);
    }
}
