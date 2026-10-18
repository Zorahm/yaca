//! Redrawing the feed of a session that was closed (§9).
//!
//! The journal holds every event the feed was built from, so reopening a
//! session is folding it again. Not all of it, though: some events say what
//! happened, and some say what *was* — and what was is over.
//!
//! [`AgentEvent::ProcessesUpdated`] is the process table of a run whose
//! commands died with it, and its "Stop" would stop nothing.
//! [`AgentEvent::StateChanged`] is the mode and model the run ended on; the
//! reopened session has its own, and the shell already carries it.
//! [`AgentEvent::CheckpointCreated`] stays only for the points the session still
//! holds: the list of them is saved next to the journal (`checkpoint::ledger`),
//! so a divider for a point on that list offers a rollback that works, and one
//! for a point that is not — a snapshot someone deleted by hand — would offer
//! one the core no longer knows. §0 has a name for that. The list itself
//! arrives as one [`AgentEvent::CheckpointsUpdated`] at the end, not as the
//! lists the run went through.
//!
//! Hence the rule: **a replay carries what happened — not what was, and what
//! can still be done only where the core says it can.**
//!
//! Then the tail gets closed. A run that died mid-turn — window killed, machine
//! down — leaves a journal whose last turn never finished, and folding that as
//! it stands gives a feed with a live `●` on a call that stopped hours ago, and
//! a question with buttons nobody is listening to. Both are ended here with the
//! events the core would have sent had anyone been there to stop it.

use crate::checkpoint::CheckpointSummary;
use crate::event::AgentEvent;
use crate::id::{RequestId, TurnId};

/// The journal of a session, as the feed of a new run may draw it, for a run
/// that has no checkpoints to offer.
pub fn feed(events: Vec<AgentEvent>) -> Vec<AgentEvent> {
    with_checkpoints(events, &[])
}

/// [`feed`] for a session that holds `checkpoints`: the dividers of those
/// points stay, and the panel's list is `checkpoints` as they are now.
pub fn with_checkpoints(
    events: Vec<AgentEvent>,
    checkpoints: &[CheckpointSummary],
) -> Vec<AgentEvent> {
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
            | AgentEvent::CheckpointsUpdated { .. }
            | AgentEvent::CheckpointPreviewed { .. } => continue,

            // A divider is kept for a point the session can still go back to.
            AgentEvent::CheckpointCreated { checkpoint }
                if !checkpoints.iter().any(|held| held.id == checkpoint.id) =>
            {
                continue
            }

            AgentEvent::TurnStarted { turn_id, .. } => open_turn = Some(turn_id.clone()),
            AgentEvent::TurnFinished { turn_id, .. } | AgentEvent::TurnCancelled { turn_id, .. }
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

    if !checkpoints.is_empty() {
        out.push(AgentEvent::CheckpointsUpdated {
            checkpoints: checkpoints.to_vec(),
        });
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
        out.push(AgentEvent::TurnCancelled {
            turn_id,
            at: Default::default(),
        });
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
            at: Default::default(),
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
                text: "hello".into(),
                attachments: Vec::new(),
            },
            started("t1"),
            said("to you too"),
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
        // "Stop" that stops nothing.
        assert_eq!(feed(journal), vec![started("t1"), finished("t1")]);
    }

    fn point() -> CheckpointSummary {
        CheckpointSummary {
            id: CheckpointId::from("ckpt_1"),
            at: TsMillis(1_700_000_000_000),
            reason: CheckpointReason::Message,
            tracked: 12,
        }
    }

    #[test]
    fn dividers_are_left_behind_when_the_session_holds_no_points() {
        let journal = vec![snapshot(), started("t1"), said("done"), finished("t1")];

        // Nothing to roll back to: the divider would offer a rollback to a
        // point this run has never heard of.
        assert_eq!(
            feed(journal),
            vec![started("t1"), said("done"), finished("t1")]
        );
    }

    #[test]
    fn a_divider_of_a_point_the_session_holds_stays_and_the_panel_is_filled() {
        let journal = vec![snapshot(), started("t1"), said("done"), finished("t1")];

        let replayed = with_checkpoints(journal, &[point()]);

        assert_eq!(
            replayed,
            vec![
                snapshot(),
                started("t1"),
                said("done"),
                finished("t1"),
                AgentEvent::CheckpointsUpdated {
                    checkpoints: vec![point()]
                },
            ]
        );
    }

    #[test]
    fn a_divider_of_a_point_that_is_gone_does_not_stay() {
        let mut gone = point();
        gone.id = CheckpointId::from("ckpt_gone");
        let journal = vec![
            AgentEvent::CheckpointCreated {
                checkpoint: gone.clone(),
            },
            snapshot(),
            started("t1"),
            finished("t1"),
        ];

        let replayed = with_checkpoints(journal, &[point()]);

        assert!(replayed
            .iter()
            .all(|e| !matches!(e, AgentEvent::CheckpointCreated { checkpoint } if checkpoint.id == gone.id)));
        assert!(replayed.contains(&snapshot()));
    }

    #[test]
    fn the_lists_the_old_run_went_through_are_replaced_by_the_current_one() {
        let old = AgentEvent::CheckpointsUpdated {
            checkpoints: vec![point(), point()],
        };
        let replayed = with_checkpoints(vec![snapshot(), old, started("t1")], &[point()]);

        let lists: Vec<_> = replayed
            .iter()
            .filter(|e| matches!(e, AgentEvent::CheckpointsUpdated { .. }))
            .collect();
        assert_eq!(lists.len(), 1);
    }

    #[test]
    fn a_rollback_that_happened_stays_in_the_feed() {
        let restored = AgentEvent::CheckpointRestored {
            id: CheckpointId::from("ckpt_1"),
            at: TsMillis(1_700_000_000_000),
            files: 3,
        };
        let journal = vec![started("t1"), restored.clone(), finished("t1")];

        // "Rolled back to 14:32 · 3 files" is a thing that happened, not a thing
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
                turn_id: TurnId::from("t1"),
                at: Default::default(),
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
                turn_id: TurnId::from("t1"),
                at: Default::default(),
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
                turn_id: TurnId::from("t1"),
                at: Default::default(),
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
    fn the_model_that_answered_a_request_is_kept_in_the_replay() {
        let decided = AgentEvent::RouteDecided {
            route: Some("code".into()),
            provider: "local".into(),
            model: "qwen".into(),
            matched: vec!["keyword:refactor".into()],
        };
        let journal = vec![started("t1"), decided, said("done"), finished("t1")];

        // What happened, not live status: the feed shows which model answered
        // above the text it wrote, as the live run did.
        assert_eq!(feed(journal.clone()), journal);
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
