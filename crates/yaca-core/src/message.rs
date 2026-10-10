//! Модель разговора, независимая от провайдера.
//!
//! В форму конкретного API переводит сам провайдер. Anthropic ложится почти
//! один в один, OpenAI-совместимые эндпоинты — нет, и именно поэтому перевод
//! живёт в провайдере, а не здесь.

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

    /// Результаты тулов уходят одним сообщением пользователя.
    ///
    /// Раскидать их по нескольким сообщениям — тихо отучить модель от
    /// параллельных вызовов, и заметить это по логам почти невозможно.
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
    /// Блок рассуждения. `signature` возвращается моделью и обязан уехать
    /// обратно без изменений, иначе следующий запрос отвергнут.
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
    /// Зашифрованное рассуждение. Содержимое непрозрачно, но пересылать надо.
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
}
