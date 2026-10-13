//! The conversation model, independent of the provider.
//!
//! The provider itself translates it into the form of a concrete API. Anthropic maps onto it almost
//! one to one, OpenAI-compatible endpoints do not, and that is why the translation
//! lives in the provider rather than here.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::picture::Picture;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user_text(text: impl Into<String>) -> Self {
        Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Message {
            role: Role::Assistant,
            content,
        }
    }

    /// Tool results go out in one user message.
    ///
    /// Spreading them over several messages quietly teaches the model to stop
    /// making parallel calls, and the logs make that almost impossible to notice.
    pub fn tool_results(results: Vec<ContentBlock>) -> Self {
        Message {
            role: Role::User,
            content: results,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    /// A reasoning block. `signature` is returned by the model and must go back
    /// unchanged, otherwise the next request is rejected.
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
    /// Encrypted reasoning. The content is opaque, but it has to be sent back.
    RedactedThinking {
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    /// An image, in a message from the human's side only.
    ///
    /// One a tool read travels here too, after the results of its batch rather
    /// than inside its own result: one of the two wire shapes has no place for
    /// an image in a tool's answer, and both have one in a user message.
    Image {
        media_type: String,
        /// Base64, shared: the history is copied into every request.
        data: Arc<str>,
        width: u32,
        height: u32,
    },
}

impl ContentBlock {
    /// An image as the model is given it: a line saying which one this is,
    /// then the pixels. Several may arrive in one message, and without the
    /// line the model has no way to tell which path each belongs to.
    pub fn picture(name: &str, picture: Picture) -> [ContentBlock; 2] {
        [
            ContentBlock::Text {
                text: format!("Image: {name}"),
            },
            ContentBlock::Image {
                media_type: picture.media_type.to_string(),
                data: picture.data,
                width: picture.width,
                height: picture.height,
            },
        ]
    }
}

/// What the model reads for a call whose result was never written down.
const CALL_NEVER_FINISHED: &str = "[This call did not finish: the session was interrupted      before its result was recorded, so what it did is unknown. Check the state of the      project before relying on it.]";

/// Gives every `tool_use` that has no `tool_result` right after it an answer.
///
/// A provider refuses a conversation with such a call on every request from
/// then on, and the way one gets there is not a bug in a turn but a stop in the
/// middle of it: the process was killed or the window closed while a batch ran,
/// and the journal holds the assistant's message without what followed. The
/// session reopens into a conversation it can never continue.
///
/// The missing results go at the head of the message that follows — results
/// first, then whatever else it says — or into a message of their own when the
/// calls are the last thing there is. Calls that were answered are untouched.
pub fn answer_dangling_calls(messages: &mut Vec<Message>) {
    let mut at = 0;
    while at < messages.len() {
        let wanted: Vec<String> = match messages[at].role {
            Role::Assistant => messages[at]
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { id, .. } => Some(id.clone()),
                    _ => None,
                })
                .collect(),
            Role::User => Vec::new(),
        };
        if wanted.is_empty() {
            at += 1;
            continue;
        }

        let next_is_user = messages.get(at + 1).is_some_and(|m| m.role == Role::User);
        let answered: Vec<&str> = match messages.get(at + 1) {
            Some(next) if next_is_user => next
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let missing: Vec<ContentBlock> = wanted
            .iter()
            .filter(|id| !answered.contains(&id.as_str()))
            .map(|id| ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: CALL_NEVER_FINISHED.to_string(),
                is_error: true,
            })
            .collect();

        if !missing.is_empty() {
            if next_is_user {
                messages[at + 1].content.splice(0..0, missing);
            } else {
                messages.insert(at + 1, Message::tool_results(missing));
            }
        }
        at += 1;
    }
}

/// What stands where an image was, for a model that cannot be shown one.
const IMAGE_OMITTED: &str = "[An image was here. The model in use does not accept images, \
     so it is not shown: say so if the answer depends on it.]";

pub fn has_images(messages: &[Message]) -> bool {
    messages
        .iter()
        .flat_map(|m| &m.content)
        .any(|b| matches!(b, ContentBlock::Image { .. }))
}

/// The same conversation with every image replaced by a note that it was there.
///
/// For a model without vision. The images stay in the history itself: the
/// human may switch to a model that sees, and then they are wanted again.
pub fn without_images(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .map(|message| Message {
            role: message.role,
            content: message
                .content
                .iter()
                .map(|block| match block {
                    ContentBlock::Image { .. } => ContentBlock::Text {
                        text: IMAGE_OMITTED.to_string(),
                    },
                    other => other.clone(),
                })
                .collect(),
        })
        .collect()
}

/// Removes every thinking block, and any message left with nothing in it.
///
/// Used where the prefix the reasoning was bound to has gone — see
/// `session::binding`. The emptied message goes too: a turn cut off by
/// `max_tokens` while still thinking is an assistant message of reasoning
/// alone, and one with empty content is rejected by the API outright.
pub fn strip_thinking(messages: &mut Vec<Message>) {
    for message in messages.iter_mut() {
        message.content.retain(|b| {
            !matches!(
                b,
                ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. }
            )
        });
    }
    messages.retain(|m| !m.content.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thought() -> ContentBlock {
        ContentBlock::Thinking {
            thinking: "hm".into(),
            signature: Some("sig".into()),
        }
    }

    fn text(t: &str) -> ContentBlock {
        ContentBlock::Text { text: t.into() }
    }

    fn image() -> ContentBlock {
        ContentBlock::Image {
            media_type: "image/png".into(),
            data: "AAAA".into(),
            width: 2,
            height: 2,
        }
    }

    #[test]
    fn a_blind_model_gets_a_note_where_the_image_was() {
        let history = vec![Message {
            role: Role::User,
            content: vec![text("look"), image()],
        }];
        assert!(has_images(&history));

        let shown = without_images(&history);
        assert!(!has_images(&shown));
        // Still two blocks: the model is told something was there.
        assert_eq!(shown[0].content.len(), 2);
        assert!(matches!(&shown[0].content[1], ContentBlock::Text { text } if text.contains("image")));
        // And the history keeps it for a model that can see.
        assert!(has_images(&history));
    }

    #[test]
    fn an_image_block_survives_the_journal() {
        let line = serde_json::to_string(&image()).unwrap();
        assert!(line.contains(r#""kind":"image""#), "{line}");
        assert_eq!(serde_json::from_str::<ContentBlock>(&line).unwrap(), image());
    }

    #[test]
    fn stripping_keeps_everything_but_the_reasoning() {
        let mut history = vec![
            Message::user_text("q"),
            Message::assistant(vec![
                thought(),
                ContentBlock::RedactedThinking { data: "x".into() },
                text("a"),
            ]),
        ];
        strip_thinking(&mut history);
        assert_eq!(
            history,
            vec![Message::user_text("q"), Message::assistant(vec![text("a")])]
        );
    }

    #[test]
    fn a_turn_that_was_only_reasoning_goes_whole() {
        let mut history = vec![
            Message::user_text("q"),
            Message::assistant(vec![thought()]),
            Message::user_text("again"),
        ];
        strip_thinking(&mut history);
        assert_eq!(
            history,
            vec![Message::user_text("q"), Message::user_text("again")]
        );
    }

    fn call(id: &str) -> ContentBlock {
        ContentBlock::ToolUse {
            id: id.into(),
            name: "Bash".into(),
            input: serde_json::json!({}),
        }
    }

    fn result(id: &str) -> ContentBlock {
        ContentBlock::ToolResult {
            tool_use_id: id.into(),
            content: "ok".into(),
            is_error: false,
        }
    }

    fn answered(message: &Message) -> Vec<String> {
        message
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn calls_left_without_results_are_answered_before_the_next_message() {
        // What a session killed mid-batch leaves in its journal, and what the
        // human's next message then follows.
        let mut history = vec![
            Message::user_text("q"),
            Message::assistant(vec![call("a"), call("b")]),
            Message::user_text("are you there?"),
        ];
        answer_dangling_calls(&mut history);

        assert_eq!(history.len(), 3, "no message added, none reordered");
        assert_eq!(answered(&history[2]), ["a", "b"]);
        // The results come first in the message, the human's words after.
        assert!(matches!(&history[2].content[2], ContentBlock::Text { text } if text == "are you there?"));
        assert!(matches!(
            &history[2].content[0],
            ContentBlock::ToolResult { is_error: true, .. }
        ));
    }

    #[test]
    fn only_the_missing_results_are_made_up() {
        let mut history = vec![
            Message::assistant(vec![call("a"), call("b")]),
            Message::tool_results(vec![result("b")]),
        ];
        answer_dangling_calls(&mut history);
        assert_eq!(answered(&history[1]), ["a", "b"]);
        // The real one is untouched.
        assert!(matches!(
            &history[1].content[1],
            ContentBlock::ToolResult { content, is_error: false, .. } if content == "ok"
        ));
    }

    #[test]
    fn calls_at_the_very_end_get_a_message_of_their_own() {
        let mut history = vec![Message::user_text("q"), Message::assistant(vec![call("a")])];
        answer_dangling_calls(&mut history);
        assert_eq!(history.len(), 3);
        assert_eq!(history[2].role, Role::User);
        assert_eq!(answered(&history[2]), ["a"]);
    }

    #[test]
    fn a_whole_conversation_is_left_as_it_was() {
        let mut history = vec![
            Message::user_text("q"),
            Message::assistant(vec![call("a")]),
            Message::tool_results(vec![result("a")]),
            Message::assistant(vec![text("done")]),
        ];
        let before = history.clone();
        answer_dangling_calls(&mut history);
        assert_eq!(history, before);
    }
}
