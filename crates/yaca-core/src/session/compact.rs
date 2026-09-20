//! Compacting history (§5.11).
//!
//! Our own summarizing pass rather than the provider's. §5.11 requires the
//! summary to be readable and editable before the conversation continues, and a
//! summary the provider keeps to itself is neither. It also has to work with
//! any model (§8), including a local one that has no such feature at all.
//!
//! The history being dropped is rendered to plain text and summarized as text.
//! That side-steps every shape constraint at once: a `tool_use` block whose
//! `tool_result` was cut off, a thinking block whose signature the summarizing
//! model has no business seeing, a cheap model asked to accept a conversation
//! it cannot parse. What goes out is a transcript and a question about it.
//!
//! What comes back replaces the history, and the tail is kept verbatim: an
//! agent that has just lost the last thing it was told is worse company than an
//! agent with a slightly longer prompt. The tail is the last two exchanges with
//! the human, or — when those are most of the conversation, as one long turn of
//! calls is — the model's last two steps. Verbatim except for its reasoning: the
//! kept turns thought with the whole history before them, and a thinking block
//! replayed after a summary in its place is bound to a conversation that no
//! longer exists (`session::binding`).

use crate::message::{strip_thinking, ContentBlock, Message, Role};

/// How full the context has to be before the core compacts without being
/// asked. §6.1.1 warns at 80% and says it will happen on its own; this is where
/// "on its own" is — a share of what the window leaves for input once the
/// answer has its room (`session::compaction`).
pub const AUTO_RATIO: f32 = 0.9;

/// Exchanges kept word for word after the summary.
const KEEP_RECENT: usize = 2;

/// Below this there is nothing worth summarizing, and a summary of two
/// messages is longer than the two messages.
const MIN_EXCHANGES: usize = KEEP_RECENT + 2;

/// The model's steps — a response and the results of its calls — kept word for
/// word when the cut goes inside a turn. The last one is what the agent is
/// about to act on; the one before is why.
const KEEP_RECENT_STEPS: usize = 2;

/// Two steps summarized is the least worth a summarizing request. It also
/// means every compaction takes at least two steps away, so compacting again
/// and again cannot go on without end.
const MIN_STEPS: usize = KEEP_RECENT_STEPS + 2;

/// How much of one tool result survives into the transcript. The middle of a
/// build log is a list of files, and paying for it twice — once in the
/// conversation, once in the summary — buys nothing.
const RESULT_MAX_CHARS: usize = 800;

/// Marks the summary in the history, so the model reads it as a record of what
/// came before rather than as something the human just said.
pub const SUMMARY_HEADER: &str =
    "[Summary of the earlier part of this conversation, which has been compacted \
     to free context. Treat it as your own memory of what happened.]";

pub const SYSTEM: &str =
    "You are compacting a coding session so it can continue in a smaller context. \
     Write the record the agent will wake up with.\n\n\
     Cover, in this order and only where there is something to say:\n\
     - what the human asked for, with short requests kept in their own words — \
     the agent acts on them, and a paraphrase drifts — including constraints and \
     anything they refused;\n\
     - decisions taken and why, so they are not re-litigated;\n\
     - files created, changed or deleted, by path;\n\
     - what is verified working, and by what evidence;\n\
     - what is still open, and the next concrete step.\n\n\
     Be specific: paths, names, numbers. Drop the narration, the greetings and \
     the tool output. Do not invent anything that is not in the transcript, and \
     do not say the summary is a summary. Write in the language of the \
     conversation.";

/// Is this message the summary a compaction left behind?
///
/// Asked when a session is reopened: §5.11 lets the summary be edited, and to
/// edit it the worker has to know it is there.
pub fn is_summary(message: Option<&Message>) -> bool {
    let Some(message) = message else {
        return false;
    };
    message.role == Role::User
        && matches!(
            message.content.first(),
            Some(ContentBlock::Text { text }) if text.starts_with(SUMMARY_HEADER)
        )
}

/// A history split for compaction.
pub struct Plan {
    /// Rendered for the summarizer.
    pub transcript: String,
    /// Kept word for word after the summary.
    pub tail: Vec<Message>,
    /// Characters dropped, against the whole, for estimating what was freed.
    pub dropped_chars: usize,
    pub total_chars: usize,
}

/// Splits the history, or `None` when there is not enough of it to be worth
/// splitting.
pub fn plan(history: &[Message]) -> Option<Plan> {
    let at = cut(history)?;
    let (head, tail) = history.split_at(at);

    let dropped_chars = weight(head);
    let total_chars = dropped_chars + weight(tail);

    Some(Plan {
        transcript: transcript(head),
        tail: tail.to_vec(),
        dropped_chars,
        total_chars,
    })
}

/// Where to cut, in messages.
///
/// By whole turns where that frees the context: the last two exchanges with
/// the human stay as they were, which is the better record. But an agent
/// working through a task spends one turn on dozens of calls, and then the
/// last two turns are most of the conversation — keeping them frees nothing,
/// and a single turn has no second boundary to cut at at all. Then the cut goes
/// inside, between the model's steps.
fn cut(history: &[Message]) -> Option<usize> {
    let by_turns = cut_by_turns(history);
    if let Some(at) = by_turns {
        if weight(&history[at..]) * 2 <= weight(history) {
            return Some(at);
        }
    }
    cut_by_steps(history)
        .filter(|at| by_turns.is_none_or(|turns| *at > turns))
        .or(by_turns)
}

/// Only at a plain user message — one the human sent, not a carrier of tool
/// results. Anywhere else and a `tool_use` block loses the `tool_result` that
/// answers it, which the API rejects the whole conversation for.
fn cut_by_turns(history: &[Message]) -> Option<usize> {
    let boundaries: Vec<usize> = history
        .iter()
        .enumerate()
        .filter(|(_, m)| is_human_turn(m))
        .map(|(i, _)| i)
        .collect();

    if boundaries.len() < MIN_EXCHANGES {
        return None;
    }
    let at = boundaries[boundaries.len() - KEEP_RECENT];
    (at > 0).then_some(at)
}

/// Before one of the model's responses, keeping the last few with their
/// results.
///
/// A response is where a step begins: its calls, then the message carrying
/// their results. Cutting in front of it leaves every call in the head answered
/// in the head and every call in the tail answered in the tail.
fn cut_by_steps(history: &[Message]) -> Option<usize> {
    let steps: Vec<usize> = history
        .iter()
        .enumerate()
        .filter(|(i, m)| *i > 0 && m.role == Role::Assistant)
        .map(|(i, _)| i)
        .collect();

    if steps.len() < MIN_STEPS {
        return None;
    }
    Some(steps[steps.len() - KEEP_RECENT_STEPS])
}

/// A user message the human sent, as opposed to one carrying tool results.
fn is_human_turn(message: &Message) -> bool {
    message.role == Role::User
        && !message
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
}

/// What the summarizing model reads.
fn transcript(messages: &[Message]) -> String {
    let mut out = String::new();
    for message in messages {
        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    let who = if message.role == Role::User {
                        "Human"
                    } else {
                        "Agent"
                    };
                    out.push_str(&format!("{who}: {}\n\n", text.trim()));
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    out.push_str(&format!(
                        "Agent used {name}: {}\n",
                        clip(&input.to_string())
                    ));
                }
                ContentBlock::ToolResult {
                    content, is_error, ..
                } => {
                    let label = if *is_error { "failed" } else { "result" };
                    out.push_str(&format!("({label}) {}\n\n", clip(content)));
                }
                // Thinking is the model's scratch paper, and a signature means
                // nothing to a different model. Neither belongs in a summary.
                ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => {}
            }
        }
    }
    out
}

/// The history a compacted session continues with.
///
/// Also what an edited summary (§5.11) rebuilds with, and the reasoning goes
/// there for the same reason: every turn after the summary thought with the
/// old one in front of it.
pub fn rebuilt(summary: &str, tail: Vec<Message>) -> Vec<Message> {
    let mut out = Vec::with_capacity(tail.len() + 1);
    out.push(Message::user_text(format!(
        "{SUMMARY_HEADER}\n\n{}",
        summary.trim()
    )));
    out.extend(tail);
    strip_thinking(&mut out);
    out
}

/// How many tokens the compaction freed.
///
/// An estimate, and openly one. The exact number is a fact only the provider
/// holds, and it holds it one response later — by which time the divider §5.11
/// asks for has been on screen for a minute. Characters are the honest
/// proportion available now: the share of the conversation that went away,
/// applied to the context the provider last measured.
pub fn freed(context_used: u32, plan: &Plan, summary_chars: usize) -> u32 {
    if plan.total_chars == 0 {
        return 0;
    }
    let share = plan.dropped_chars.saturating_sub(summary_chars) as f64 / plan.total_chars as f64;
    (context_used as f64 * share.max(0.0)) as u32
}

/// Characters, not bytes: the summary is measured in characters too, and in a
/// Russian conversation the two differ by a factor of two.
pub(super) fn weight(messages: &[Message]) -> usize {
    messages
        .iter()
        .flat_map(|m| &m.content)
        .map(|b| match b {
            ContentBlock::Text { text } => text.chars().count(),
            ContentBlock::Thinking { thinking, .. } => thinking.chars().count(),
            ContentBlock::RedactedThinking { data } => data.chars().count(),
            ContentBlock::ToolUse { input, .. } => input.to_string().chars().count(),
            ContentBlock::ToolResult { content, .. } => content.chars().count(),
        })
        .sum()
}

fn clip(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= RESULT_MAX_CHARS {
        return trimmed.to_string();
    }
    trimmed
        .chars()
        .take(RESULT_MAX_CHARS)
        .chain("… (обрезано)".chars())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn human(text: &str) -> Message {
        Message::user_text(text)
    }

    fn agent(text: &str) -> Message {
        Message::assistant(vec![ContentBlock::Text { text: text.into() }])
    }

    fn call(id: &str) -> Message {
        Message::assistant(vec![ContentBlock::ToolUse {
            id: id.into(),
            name: "Read".into(),
            input: serde_json::json!({ "path": "a.rs" }),
        }])
    }

    fn result(id: &str) -> Message {
        Message::tool_results(vec![ContentBlock::ToolResult {
            tool_use_id: id.into(),
            content: "fn main() {}".into(),
            is_error: false,
        }])
    }

    fn conversation() -> Vec<Message> {
        vec![
            human("раз"),
            call("c1"),
            result("c1"),
            agent("ответ раз"),
            human("два"),
            agent("ответ два"),
            human("три"),
            agent("ответ три"),
            human("четыре"),
            agent("ответ четыре"),
        ]
    }

    #[test]
    fn the_summary_is_recognised_when_a_session_is_reopened() {
        let history = rebuilt("сделали то-то", vec![human("три")]);
        assert!(is_summary(history.first()));
        assert!(!is_summary(Some(&human("обычное сообщение"))));
        assert!(!is_summary(None));
    }

    #[test]
    fn a_short_conversation_is_not_worth_compacting() {
        assert!(plan(&[human("раз"), agent("ответ")]).is_none());
    }

    #[test]
    fn the_cut_lands_on_something_the_human_said() {
        let history = conversation();
        let at = cut(&history).unwrap();
        // Anywhere else and a tool call loses the result that answers it,
        // which the API rejects the whole conversation for.
        assert!(is_human_turn(&history[at]), "срез на {at}");
        assert_eq!(at, 6, "две последние пары остаются как есть");
    }

    #[test]
    fn the_tail_is_kept_word_for_word() {
        let plan = plan(&conversation()).unwrap();
        assert_eq!(plan.tail.len(), 4);
        assert_eq!(plan.tail[0], human("три"));
    }

    #[test]
    fn the_transcript_carries_both_sides_and_the_calls() {
        let plan = plan(&conversation()).unwrap();
        assert!(plan.transcript.contains("Human: раз"));
        assert!(plan.transcript.contains("Agent: ответ раз"));
        assert!(plan.transcript.contains("Agent used Read"));
        assert!(plan.transcript.contains("(result) fn main() {}"));
    }

    #[test]
    fn a_huge_tool_result_does_not_ride_along_whole() {
        let mut history = conversation();
        history[2] = Message::tool_results(vec![ContentBlock::ToolResult {
            tool_use_id: "c1".into(),
            content: "строка\n".repeat(10_000),
            is_error: false,
        }]);
        let plan = plan(&history).unwrap();
        assert!(plan.transcript.contains("обрезано"));
        assert!(plan.transcript.chars().count() < 2_000);
    }

    #[test]
    fn the_rebuilt_history_opens_with_the_summary_and_then_the_tail() {
        let plan = plan(&conversation()).unwrap();
        let history = rebuilt("сделали то-то", plan.tail);

        assert_eq!(history.len(), 5);
        assert_eq!(history[0].role, Role::User);
        let ContentBlock::Text { text } = &history[0].content[0] else {
            panic!("не текст");
        };
        // The model has to read it as its own memory, not as a new request.
        assert!(text.starts_with(SUMMARY_HEADER));
        assert!(text.contains("сделали то-то"));
        assert_eq!(history[1], human("три"));
    }

    /// One human message and then a long run of calls — what an agent working
    /// through a task looks like, and what the cut by turns cannot touch.
    fn long_turn(steps: usize, result: &str) -> Vec<Message> {
        let mut history = vec![human("почини все тесты")];
        for i in 0..steps {
            let id = format!("c{i}");
            history.push(call(&id));
            history.push(Message::tool_results(vec![ContentBlock::ToolResult {
                tool_use_id: id,
                content: result.into(),
                is_error: false,
            }]));
        }
        history
    }

    /// Every call in `messages` has its result there too.
    fn whole(messages: &[Message]) -> bool {
        let ids = |want_use: bool| -> Vec<String> {
            messages
                .iter()
                .flat_map(|m| &m.content)
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { id, .. } if want_use => Some(id.clone()),
                    ContentBlock::ToolResult { tool_use_id, .. } if !want_use => {
                        Some(tool_use_id.clone())
                    }
                    _ => None,
                })
                .collect()
        };
        ids(true) == ids(false)
    }

    #[test]
    fn a_long_turn_is_cut_between_its_steps() {
        let history = long_turn(6, "fn main() {}");
        let plan = plan(&history).expect("a long turn must be compactable");

        // The last two steps, each a call with its result, word for word.
        assert_eq!(plan.tail.len(), 4);
        assert_eq!(plan.tail[0].role, Role::Assistant);
        assert!(whole(&plan.tail));
        // The request that started it all goes to the summarizer.
        assert!(plan.transcript.contains("Human: почини все тесты"));
    }

    #[test]
    fn a_turn_too_short_to_cut_inside_is_left_alone() {
        // Two steps kept and two summarized is the least worth doing.
        assert!(plan(&long_turn(3, "x")).is_none());
        assert!(plan(&long_turn(4, "x")).is_some());
    }

    #[test]
    fn heavy_recent_turns_are_cut_inside_rather_than_kept_whole() {
        let mut history = conversation();
        history.extend(long_turn(6, &"вывод сборки ".repeat(500))[1..].iter().cloned());
        let at_turns = cut_by_turns(&history).unwrap();

        let plan = plan(&history).unwrap();
        // Keeping the last two turns whole would keep nearly everything.
        assert!(history.len() - plan.tail.len() > at_turns);
        assert_eq!(plan.tail.len(), 4);
        assert!(whole(&plan.tail));
    }

    #[test]
    fn light_recent_turns_are_still_kept_whole() {
        let mut history = conversation();
        history.push(human("пять"));
        history.push(agent("ответ пять"));
        let plan = plan(&history).unwrap();
        // A cut by turns frees most of it, so the turns stay whole.
        assert!(matches!(&plan.tail[0].content[0], ContentBlock::Text { text } if text == "четыре"));
    }

    #[test]
    fn the_summarizer_keeps_short_requests_in_the_humans_words() {
        // Cut inside a turn, the request the agent is still working on goes
        // into the summary, and a paraphrase of it is where drift starts.
        assert!(SYSTEM.contains("in their own words"));
    }

    #[test]
    fn what_was_freed_is_proportional_to_what_went_away() {
        let plan = Plan {
            transcript: String::new(),
            tail: Vec::new(),
            dropped_chars: 900,
            total_chars: 1000,
        };
        // Nine tenths of the conversation left, minus what the summary costs.
        assert_eq!(freed(100_000, &plan, 100), 80_000);
    }

    #[test]
    fn a_summary_longer_than_what_it_replaced_frees_nothing() {
        let plan = Plan {
            transcript: String::new(),
            tail: Vec::new(),
            dropped_chars: 100,
            total_chars: 1000,
        };
        assert_eq!(freed(100_000, &plan, 5_000), 0);
    }
}
