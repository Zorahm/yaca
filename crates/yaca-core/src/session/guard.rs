//! Protection against a turn that goes round in circles.
//!
//! Two independent brakes. The step ceiling (`Settings::max_steps`) is the
//! hard one: whatever the model is doing, a turn makes only so many requests.
//! The repeat check is the early one: a small model that gets the same answer
//! to the same call usually asks it again, and waiting for the ceiling to find
//! that out costs a hundred requests.
//!
//! A repeat is the same tool with the same arguments *and* the same result,
//! three times running. The result is part of it on purpose: a poll of
//! `BashOutput` that returns new lines each time, or a test run between two
//! edits, repeats the call and is making progress; the same answer three times
//! is not.

use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::id::CallId;
use crate::message::ContentBlock;
use crate::question::{Question, QuestionOption, QuestionReply};
use crate::tool::take_description;

use super::accumulator::PendingCall;
use super::questions::Questions;

/// Identical calls in a row after which the human is asked.
pub(super) const REPEAT_LIMIT: u32 = 3;

/// What the model reads when the human lets a repeating turn go on.
///
/// The question went to the human; the model does not know it was asked, and
/// the same answer to the same call is not something it noticed by itself.
const CHANGE_COURSE: &str = "[loop check] You have made the same call three times in a row \
     and got the same result each time. Do not make it again. Change what you are doing, \
     or say plainly what is blocking you.";

/// What the model reads when the human ended the turn over a repeat.
const STOPPED_ON_REPEAT: &str = "[loop check] The user stopped this turn because you made the \
     same call three times in a row with the same result. Wait for their next message.";

#[derive(PartialEq)]
struct Signature {
    tool: String,
    input: Value,
    result: String,
    is_error: bool,
}

/// The call that completed a run of identical ones.
pub(super) struct Repeat {
    /// Its row in the feed is where the question is drawn.
    pub(super) call_id: CallId,
    pub(super) tool: String,
}

/// What the run so far looks like, kept for one turn.
#[derive(Default)]
pub(super) struct RepeatGuard {
    last: Option<Signature>,
    run: u32,
}

impl RepeatGuard {
    /// Feeds the calls of one batch with their results, in the order the model
    /// wrote them. Returns the first call that made [`REPEAT_LIMIT`] in a row;
    /// the count starts over after it, so a human who said "go on" is asked
    /// again after three more rather than after every one.
    pub(super) fn observe(
        &mut self,
        calls: &[PendingCall],
        results: &[ContentBlock],
    ) -> Option<Repeat> {
        let mut found = None;
        for call in calls {
            let Some((result, is_error)) = results.iter().find_map(|block| match block {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } if *tool_use_id == call.id => Some((content.clone(), *is_error)),
                _ => None,
            }) else {
                continue;
            };

            // The feed's caption is not part of what the call does.
            let mut input = call.input.clone();
            let _ = take_description(&mut input);
            let signature = Signature {
                tool: call.name.clone(),
                input,
                result,
                is_error,
            };

            if self.last.as_ref() == Some(&signature) {
                self.run += 1;
            } else {
                self.run = 1;
                self.last = Some(signature);
            }

            if self.run >= REPEAT_LIMIT && found.is_none() {
                found = Some(Repeat {
                    call_id: CallId::from(call.id.clone()),
                    tool: call.name.clone(),
                });
                self.run = 0;
                self.last = None;
            }
        }
        found
    }
}

/// Whether the turn goes on after a repeat was put to the human.
pub(super) enum Flow {
    Go,
    Stop,
}

/// Asks the human what to do about a repeat and returns what the model must
/// be told alongside the batch's results, and whether the turn goes on.
///
/// There is no timeout, like for every question (§5.7): a turn that is
/// repeating itself is exactly the one nobody should be spending money on
/// while the human is away.
pub(super) async fn put_to_human(
    questions: Questions,
    repeat: &Repeat,
    cancel: &CancellationToken,
) -> (Flow, ContentBlock) {
    let question = Question {
        question: format!(
            "The model called {} three times in a row with the same arguments and received the same \
             result. It seems to be in a loop. What should be done?",
            repeat.tool
        ),
        options: vec![
            QuestionOption {
                label: "Continue".to_string(),
                description: Some("The model will receive a warning and try a different approach".to_string()),
            },
            QuestionOption {
                label: "Stop".to_string(),
                description: Some("End the turn; you will then decide what to do next".to_string()),
            },
        ],
        allow_other: true,
        multi_select: false,
    };

    let note = |text: String| ContentBlock::Text { text };
    match questions.ask(question, cancel).await {
        Ok(QuestionReply::Answered {
            selected,
            other: Some(words),
        }) if !selected.contains(&1) => (
            Flow::Go,
            note(format!("{CHANGE_COURSE} The user said: \"{words}\"")),
        ),
        Ok(QuestionReply::Answered { selected, .. }) if selected.contains(&0) => {
            (Flow::Go, note(CHANGE_COURSE.to_string()))
        }
        // "Stop", a dismissed question, or a turn cancelled while it
        // was on screen: the human did not say to go on.
        _ => (Flow::Stop, note(STOPPED_ON_REPEAT.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(id: &str, name: &str, input: Value) -> PendingCall {
        PendingCall {
            id: id.to_string(),
            name: name.to_string(),
            input,
            garbled: None,
        }
    }

    fn result(id: &str, text: &str) -> ContentBlock {
        ContentBlock::ToolResult {
            tool_use_id: id.to_string(),
            content: text.to_string(),
            is_error: false,
        }
    }

    fn batch(guard: &mut RepeatGuard, ids: &[&str], text: &str) -> Option<Repeat> {
        let calls: Vec<_> = ids
            .iter()
            .map(|id| call(id, "Read", json!({ "path": "a.rs" })))
            .collect();
        let results: Vec<_> = ids.iter().map(|id| result(id, text)).collect();
        guard.observe(&calls, &results)
    }

    #[test]
    fn the_third_identical_call_in_a_row_trips() {
        let mut guard = RepeatGuard::default();
        assert!(batch(&mut guard, &["1"], "same").is_none());
        assert!(batch(&mut guard, &["2"], "same").is_none());
        let repeat = batch(&mut guard, &["3"], "same").expect("third call must trip");
        assert_eq!(repeat.call_id, CallId::from("3"));
    }

    #[test]
    fn a_run_inside_one_batch_counts_the_same() {
        let mut guard = RepeatGuard::default();
        assert!(batch(&mut guard, &["1", "2", "3"], "same").is_some());
    }

    #[test]
    fn another_call_in_between_breaks_the_run() {
        let mut guard = RepeatGuard::default();
        batch(&mut guard, &["1", "2"], "same");
        let other = call("x", "Grep", json!({ "pattern": "fn" }));
        guard.observe(&[other], &[result("x", "hit")]);
        assert!(batch(&mut guard, &["3"], "same").is_none());
    }

    #[test]
    fn a_changing_result_is_progress_not_a_loop() {
        let mut guard = RepeatGuard::default();
        assert!(batch(&mut guard, &["1"], "line 1").is_none());
        assert!(batch(&mut guard, &["2"], "line 2").is_none());
        assert!(batch(&mut guard, &["3"], "line 3").is_none());
    }

    #[test]
    fn the_caption_is_not_part_of_the_call() {
        let mut guard = RepeatGuard::default();
        let calls = [
            call("1", "Read", json!({ "path": "a.rs", "description": "first" })),
            call("2", "Read", json!({ "path": "a.rs", "description": "again" })),
            call("3", "Read", json!({ "path": "a.rs" })),
        ];
        let results = [result("1", "r"), result("2", "r"), result("3", "r")];
        assert!(guard.observe(&calls, &results).is_some());
    }

    #[test]
    fn after_a_trip_the_count_starts_over() {
        let mut guard = RepeatGuard::default();
        assert!(batch(&mut guard, &["1", "2", "3"], "same").is_some());
        assert!(batch(&mut guard, &["4"], "same").is_none());
        assert!(batch(&mut guard, &["5"], "same").is_none());
        assert!(batch(&mut guard, &["6"], "same").is_some());
    }
}
