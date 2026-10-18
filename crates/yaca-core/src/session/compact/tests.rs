//! Tests of where a conversation is cut and what the summarizer reads.

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
        human("one"),
        call("c1"),
        result("c1"),
        agent("answer one"),
        human("two"),
        agent("answer two"),
        human("three"),
        agent("answer three"),
        human("four"),
        agent("answer four"),
    ]
}

#[test]
fn an_image_weighs_its_pixels_and_not_its_base64() {
    let image = Message {
        role: Role::User,
        content: vec![ContentBlock::Image {
            media_type: "image/png".into(),
            // A megabyte on the wire, and about 1.8k tokens in the window.
            data: "A".repeat(1_000_000).into(),
            width: 1_568,
            height: 882,
        }],
    };
    // Counted as text it would be a third of a million tokens, and one
    // screenshot would compact the conversation it was just added to.
    assert_eq!(weight(std::slice::from_ref(&image)), 1_843 * 3);
    assert!(transcript(&[image]).contains("an image, 1568×882"));
}

#[test]
fn the_summary_is_recognised_when_a_session_is_reopened() {
    let history = rebuilt("did this and that", &[], vec![human("three")]);
    assert!(is_summary(history.first()));
    assert!(!is_summary(Some(&human("regular message"))));
    assert!(!is_summary(None));
}

#[test]
fn a_short_conversation_is_not_worth_compacting() {
    assert!(plan(&[human("one"), agent("answer")]).is_none());
}

#[test]
fn the_cut_lands_on_something_the_human_said() {
    let history = conversation();
    let at = cut(&history).unwrap();
    // Anywhere else and a tool call loses the result that answers it,
    // which the API rejects the whole conversation for.
    assert!(is_human_turn(&history[at]), "cut at {at}");
    assert_eq!(at, 6, "last two pairs remain as-is");
}

#[test]
fn the_tail_is_kept_word_for_word() {
    let plan = plan(&conversation()).unwrap();
    assert_eq!(plan.tail.len(), 4);
    assert_eq!(plan.tail[0], human("three"));
}

#[test]
fn the_transcript_carries_both_sides_and_the_calls() {
    let plan = plan(&conversation()).unwrap();
    assert!(plan.transcript.contains("Human: one"));
    assert!(plan.transcript.contains("Agent: answer one"));
    assert!(plan.transcript.contains("Agent used Read"));
    assert!(plan.transcript.contains("(result) fn main() {}"));
}

#[test]
fn a_huge_tool_result_does_not_ride_along_whole() {
    let mut history = conversation();
    history[2] = Message::tool_results(vec![ContentBlock::ToolResult {
        tool_use_id: "c1".into(),
        content: "line\n".repeat(10_000),
        is_error: false,
    }]);
    let plan = plan(&history).unwrap();
    assert!(plan.transcript.contains("truncated"));
    assert!(plan.transcript.chars().count() < 2_000);
}

#[test]
fn the_rebuilt_history_opens_with_the_summary_and_then_the_tail() {
    let plan = plan(&conversation()).unwrap();
    let history = rebuilt("did this and that", &[], plan.tail);

    assert_eq!(history.len(), 5);
    assert_eq!(history[0].role, Role::User);
    let ContentBlock::Text { text } = &history[0].content[0] else {
        panic!("not text");
    };
    // The model has to read it as its own memory, not as a new request.
    assert!(text.starts_with(SUMMARY_HEADER));
    assert!(text.contains("did this and that"));
    assert_eq!(history[1], human("three"));
}

/// One human message and then a long run of calls — what an agent working
/// through a task looks like, and what the cut by turns cannot touch.
fn long_turn(steps: usize, result: &str) -> Vec<Message> {
    let mut history = vec![human("fix all tests")];
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
    assert!(plan.transcript.contains("Human: fix all tests"));
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
    history.extend(long_turn(6, &"build output ".repeat(500))[1..].iter().cloned());
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
    history.push(human("five"));
    history.push(agent("answer five"));
    let plan = plan(&history).unwrap();
    // A cut by turns frees most of it, so the turns stay whole.
    assert!(matches!(&plan.tail[0].content[0], ContentBlock::Text { text } if text == "four"));
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
        skills: Vec::new(),
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
        skills: Vec::new(),
    };
    assert_eq!(freed(100_000, &plan, 5_000), 0);
}

#[test]
fn a_skill_loaded_in_the_dropped_part_is_kept_and_one_in_the_tail_is_not() {
    let skill = |name: &str| format!("<skill_content name=\"{name}\">x</skill_content>");
    let mut history = vec![human("one"), agent(&skill("early")), human("two"), agent("b")];
    history.extend([human(&skill("late")), agent("c"), human("four"), agent("d")]);
    let plan = plan(&history).unwrap();
    assert_eq!(plan.skills, ["early"]);
}

#[test]
fn an_edited_summary_keeps_the_skills_kept_with_it() {
    let kept = vec![KEPT_SKILLS_HEADER.to_string(), "<skill_content name=\"a\">x</skill_content>".to_string()];
    let history = rebuilt("previous summary", &kept, vec![human("three")]);
    assert_eq!(kept_skills(&history), kept);
    assert!(kept_skills(&[human("not summary")]).is_empty());
}
