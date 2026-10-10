//! Сборка тела запроса Messages API.
//!
//! Три вещи здесь сделаны намеренно и легко ломаются при «упрощении»:
//!
//! * `thinking.display` выставлен в `summarized`. По умолчанию на Opus 5 он
//!   `omitted`, и §5.3 показывал бы пустой блок рассуждения.
//! * Глубина рассуждения задаётся через `output_config.effort`, а не
//!   `thinking.budget_tokens`: последний на Opus 5 / Sonnet 5 / Opus 4.7+
//!   удалён и отвечает 400.
//! * `temperature` не отправляется вовсе — там же удалён.

use serde_json::json;

use crate::message::{ContentBlock, Message, Role};
use crate::provider::{ProviderRequest, ToolSchema};

pub(super) fn build_body(req: &ProviderRequest) -> serde_json::Value {
    let mut body = json!({
        "model": req.model,
        "max_tokens": req.max_tokens,
        "stream": true,
        // Системный промпт стабилен весь разговор — первая точка кеша.
        "system": [{
            "type": "text",
            "text": req.system,
            "cache_control": { "type": "ephemeral" },
        }],
        "messages": wire_messages(&req.messages),
    });

    if !req.tools.is_empty() {
        body["tools"] = wire_tools(&req.tools);
    }
    if req.thinking {
        // `display` явно: по умолчанию текст рассуждения приходит пустым.
        body["thinking"] = json!({ "type": "adaptive", "display": "summarized" });
    }
    if let Some(effort) = req.effort {
        body["output_config"] = json!({ "effort": effort.as_api() });
    }
    body
}

fn wire_tools(tools: &[ToolSchema]) -> serde_json::Value {
    serde_json::Value::Array(
        tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect(),
    )
}

fn wire_messages(messages: &[Message]) -> serde_json::Value {
    let last = messages.len().saturating_sub(1);
    let mut out = Vec::with_capacity(messages.len());

    for (i, msg) in messages.iter().enumerate() {
        let last_block = msg.content.len().saturating_sub(1);
        let content: Vec<serde_json::Value> = msg
            .content
            .iter()
            .enumerate()
            .map(|(j, block)| {
                let mut v = wire_block(block);
                // Вторая точка кеша — хвост истории. Она едет вперёд каждый ход,
                // и весь предыдущий разговор читается из кеша.
                if i == last && j == last_block {
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("cache_control".into(), json!({ "type": "ephemeral" }));
                    }
                }
                v
            })
            .collect();

        out.push(json!({
            "role": match msg.role { Role::User => "user", Role::Assistant => "assistant" },
            "content": content,
        }));
    }
    serde_json::Value::Array(out)
}

fn wire_block(block: &ContentBlock) -> serde_json::Value {
    match block {
        ContentBlock::Text { text } => json!({ "type": "text", "text": text }),
        ContentBlock::Thinking {
            thinking,
            signature,
        } => {
            let mut v = json!({ "type": "thinking", "thinking": thinking });
            if let Some(sig) = signature {
                v["signature"] = json!(sig);
            }
            v
        }
        ContentBlock::RedactedThinking { data } => {
            json!({ "type": "redacted_thinking", "data": data })
        }
        ContentBlock::ToolUse { id, name, input } => {
            json!({ "type": "tool_use", "id": id, "name": name, "input": input })
        }
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        }),
        ContentBlock::Image {
            media_type, data, ..
        } => json!({
            "type": "image",
            "source": { "type": "base64", "media_type": media_type, "data": &**data },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::Effort;

    fn req() -> ProviderRequest {
        ProviderRequest {
            model: "claude-opus-5".into(),
            system: "ты Yaca".into(),
            messages: vec![Message::user_text("привет")],
            tools: vec![],
            max_tokens: 64_000,
            effort: Some(Effort::Extra),
            thinking: true,
        }
    }

    #[test]
    fn рассуждение_просит_видимый_текст() {
        let body = build_body(&req());
        assert_eq!(body["thinking"]["display"], "summarized");
        assert_eq!(body["thinking"]["type"], "adaptive");
    }

    #[test]
    fn усилие_идёт_через_output_config_а_не_бюджет() {
        let body = build_body(&req());
        assert_eq!(body["output_config"]["effort"], "xhigh");
        assert!(body["thinking"].get("budget_tokens").is_none());
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn две_точки_кеша_система_и_хвост_истории() {
        let mut r = req();
        r.messages = vec![
            Message::user_text("первое"),
            Message::assistant(vec![ContentBlock::Text {
                text: "ответ".into(),
            }]),
            Message::user_text("второе"),
        ];
        let body = build_body(&r);
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        assert!(body["messages"][0]["content"][0]
            .get("cache_control")
            .is_none());
        assert_eq!(
            body["messages"][2]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
    }

    #[test]
    fn an_image_goes_as_a_base64_source() {
        let block = ContentBlock::Image {
            media_type: "image/png".into(),
            data: "AAAA".into(),
            width: 2,
            height: 2,
        };
        let wire = wire_block(&block);
        assert_eq!(wire["type"], "image");
        assert_eq!(wire["source"]["type"], "base64");
        assert_eq!(wire["source"]["media_type"], "image/png");
        assert_eq!(wire["source"]["data"], "AAAA");
        // The size is ours, for weighing the window; the API has no such field.
        assert!(wire.get("width").is_none());
    }

    #[test]
    fn подпись_рассуждения_едет_обратно() {
        let block = ContentBlock::Thinking {
            thinking: "…".into(),
            signature: Some("sig123".into()),
        };
        assert_eq!(wire_block(&block)["signature"], "sig123");
    }
}
