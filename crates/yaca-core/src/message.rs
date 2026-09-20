//! Модель разговора, независимая от провайдера.
//!
//! В форму конкретного API переводит сам провайдер. Anthropic ложится почти
//! один в один, OpenAI-совместимые эндпоинты — нет, и именно поэтому перевод
//! живёт в провайдере, а не здесь.

use serde::{Deserialize, Serialize};

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
