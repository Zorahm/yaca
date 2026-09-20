//! Сборка тела Chat Completions.
//!
//! Три места, где перевод не механический:
//!
//! * `stream_options.include_usage` обязателен. Без него поток не присылает
//!   `usage` вовсе, и счётчик токенов §7.1 молча остался бы нулём.
//! * Блоки рассуждения обратно не отправляются. В этой форме нет поля для
//!   подписи, а рассуждение без подписи — текст, за который никто не отвечает.
//! * Потолок ответа едет в том поле, которое провайдер принимает, — см.
//!   [`MaxField`]. Отправить оба нельзя: на новых моделях OpenAI это 400.

use serde_json::{json, Value};

use crate::intent::Effort;
use crate::message::{ContentBlock, Message, Role};
use crate::provider::{ProviderRequest, ToolSchema};

/// Каким полем едет потолок ответа.
///
/// `max_tokens` понимают Ollama, vLLM, Groq, DeepSeek, OpenRouter и почти всё,
/// что называет себя OpenAI-совместимым. Новые модели самого OpenAI отвечают
/// на него 400 и в тексте ошибки прямо называют `max_completion_tokens` —
/// поэтому выбор не настройка, которую человек должен угадать, а то, что
/// провайдер сообщает о себе сам (см. [`super::OpenAiProvider::stream`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum MaxField {
    #[default]
    MaxTokens,
    MaxCompletionTokens,
}

impl MaxField {
    fn key(self) -> &'static str {
        match self {
            MaxField::MaxTokens => "max_tokens",
            MaxField::MaxCompletionTokens => "max_completion_tokens",
        }
    }
}

pub(super) fn build_body(req: &ProviderRequest, max_field: MaxField) -> Value {
    let mut body = json!({
        "model": req.model,
        "stream": true,
        "stream_options": { "include_usage": true },
        "messages": wire_messages(req),
    });
    body[max_field.key()] = json!(req.max_tokens);

    if !req.tools.is_empty() {
        body["tools"] = wire_tools(&req.tools);
    }
    if let Some(effort) = req.effort {
        body["reasoning_effort"] = json!(reasoning_effort(effort));
    }
    body
}

/// В этой форме уровней три. `Extra` и `Max` ложатся в верхний: выше здесь
/// ничего нет, и придумывать четвёртый некуда.
fn reasoning_effort(effort: Effort) -> &'static str {
    match effort {
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High | Effort::Extra | Effort::Max => "high",
    }
}

fn wire_tools(tools: &[ToolSchema]) -> Value {
    Value::Array(
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    },
                })
            })
            .collect(),
    )
}

fn wire_messages(req: &ProviderRequest) -> Value {
    let mut out = vec![json!({ "role": "system", "content": req.system })];
    for msg in &req.messages {
        match msg.role {
            Role::User => push_user(&mut out, msg),
            Role::Assistant => push_assistant(&mut out, msg),
        }
    }
    Value::Array(out)
}

/// Одно сообщение человека может стать несколькими.
///
/// Результаты тулов в этой форме — отдельные сообщения роли `tool`, и они
/// обязаны идти сразу за вызовом. Поэтому они выкладываются первыми, а текст
/// человека — после них, даже если в блоках он стоял раньше.
fn push_user(out: &mut Vec<Value>, msg: &Message) {
    let mut text = String::new();

    for block in &msg.content {
        match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                // Флага «неудача» здесь нет, а модель обязана отличить отказ
                // от результата: без пометки упавшая команда читается как
                // выполненная. Префикс английский — его читает модель.
                let content = if *is_error {
                    format!("Error: {content}")
                } else {
                    content.clone()
                };
                out.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": content,
                }));
            }
            ContentBlock::Text { text: t } => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
            // Рассуждение и вызов тула в сообщении человека не появляются.
            _ => {}
        }
    }

    if !text.is_empty() {
        out.push(json!({ "role": "user", "content": text }));
    }
}

fn push_assistant(out: &mut Vec<Value>, msg: &Message) {
    let mut text = String::new();
    let mut calls = Vec::new();

    for block in &msg.content {
        match block {
            ContentBlock::Text { text: t } => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
            ContentBlock::ToolUse { id, name, input } => calls.push(json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    // Аргументы здесь — строка, а не объект.
                    "arguments": input.to_string(),
                },
            })),
            // Рассуждению в этой форме места нет — см. шапку модуля.
            ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => {}
            ContentBlock::ToolResult { .. } => {}
        }
    }

    // Ход, в котором было одно рассуждение, отправлять нечем: сообщение
    // без текста и без вызовов отвергают и OpenAI, и половина шлюзов.
    if text.is_empty() && calls.is_empty() {
        return;
    }

    let mut message = json!({ "role": "assistant", "content": text });
    if !calls.is_empty() {
        message["tool_calls"] = Value::Array(calls);
    }
    out.push(message);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> ProviderRequest {
        ProviderRequest {
            model: "gpt-oss".into(),
            system: "ты Yaca".into(),
            messages: vec![Message::user_text("привет")],
            tools: vec![],
            max_tokens: 8_000,
            effort: Some(Effort::Extra),
            thinking: true,
        }
    }

    #[test]
    fn просит_счётчики_явно() {
        let body = build_body(&req(), MaxField::default());
        assert_eq!(body["stream_options"]["include_usage"], true);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn верхнее_усилие_складывается_в_high() {
        assert_eq!(build_body(&req(), MaxField::default())["reasoning_effort"], "high");
        let mut r = req();
        r.effort = Some(Effort::Low);
        assert_eq!(build_body(&r, MaxField::default())["reasoning_effort"], "low");
        r.effort = None;
        assert!(build_body(&r, MaxField::default()).get("reasoning_effort").is_none());
    }

    #[test]
    fn системный_промпт_становится_первым_сообщением() {
        let body = build_body(&req(), MaxField::default());
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "ты Yaca");
        assert_eq!(body["messages"][1]["role"], "user");
    }

    #[test]
    fn вызов_тула_становится_tool_calls_а_аргументы_строкой() {
        let mut r = req();
        r.messages = vec![Message::assistant(vec![ContentBlock::ToolUse {
            id: "call_1".into(),
            name: "Read".into(),
            input: serde_json::json!({ "path": "src/a.rs" }),
        }])];
        let body = build_body(&r, MaxField::default());
        let call = &body["messages"][1]["tool_calls"][0];
        assert_eq!(call["id"], "call_1");
        assert_eq!(call["function"]["name"], "Read");
        assert_eq!(call["function"]["arguments"], r#"{"path":"src/a.rs"}"#);
    }

    #[test]
    fn результат_тула_едет_отдельным_сообщением_роли_tool() {
        let mut r = req();
        r.messages = vec![Message {
            role: Role::User,
            content: vec![
                ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "готово".into(),
                    is_error: false,
                },
                ContentBlock::Text {
                    text: "спасибо".into(),
                },
            ],
        }];
        let body = build_body(&r, MaxField::default());
        assert_eq!(body["messages"][1]["role"], "tool");
        assert_eq!(body["messages"][1]["tool_call_id"], "call_1");
        // Текст человека — после результата, а не перед ним.
        assert_eq!(body["messages"][2]["role"], "user");
        assert_eq!(body["messages"][2]["content"], "спасибо");
    }

    #[test]
    fn неудача_тула_помечается_в_тексте() {
        let mut r = req();
        r.messages = vec![Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".into(),
                content: "exit 1".into(),
                is_error: true,
            }],
        }];
        let body = build_body(&r, MaxField::default());
        assert_eq!(body["messages"][1]["content"], "Error: exit 1");
    }

    #[test]
    fn рассуждение_не_отправляется_обратно() {
        let mut r = req();
        r.messages = vec![Message::assistant(vec![
            ContentBlock::Thinking {
                thinking: "думаю".into(),
                signature: Some("sig".into()),
            },
            ContentBlock::Text {
                text: "ответ".into(),
            },
        ])];
        let body = serde_json::to_string(&build_body(&r, MaxField::default())).unwrap();
        assert!(!body.contains("думаю"));
        assert!(!body.contains("sig"));
        assert!(body.contains("ответ"));
    }

    #[test]
    fn ход_из_одного_рассуждения_не_даёт_пустого_сообщения() {
        let mut r = req();
        r.messages = vec![Message::assistant(vec![ContentBlock::Thinking {
            thinking: "думаю".into(),
            signature: None,
        }])];
        // Только системный промпт: отправлять было нечего.
        assert_eq!(build_body(&r, MaxField::default())["messages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn потолок_ответа_едет_одним_полем_и_никогда_двумя() {
        let by_default = build_body(&req(), MaxField::MaxTokens);
        assert_eq!(by_default["max_tokens"], 8_000);
        assert!(by_default.get("max_completion_tokens").is_none());

        // Провайдер попросил другое имя — уезжает другое, и снова одно.
        let asked = build_body(&req(), MaxField::MaxCompletionTokens);
        assert_eq!(asked["max_completion_tokens"], 8_000);
        assert!(
            asked.get("max_tokens").is_none(),
            "на оба поля разом это 400"
        );
    }
}
