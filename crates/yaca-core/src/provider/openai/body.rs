//! Building the Chat Completions body.
//!
//! Three places where the translation is not mechanical:
//!
//! * `stream_options.include_usage` is required. Without it the stream does not send
//!   `usage` at all, and the token counter of §7.1 would silently stay at zero.
//! * Reasoning is replayed only for the DeepSeek API's tool requests, where
//!   unsigned `reasoning_content` is part of the conversation protocol.
//! * The response ceiling goes in the field the provider accepts — see
//!   [`MaxField`]. Sending both is not possible: on newer OpenAI models that is a 400.

use serde_json::{json, Value};

use crate::intent::Effort;
use crate::message::{ContentBlock, Message, Role};
use crate::provider::{ProviderRequest, ToolSchema};

/// Which field the response ceiling is sent in.
///
/// `max_tokens` is understood by Ollama, vLLM, Groq, DeepSeek, OpenRouter and nearly
/// everything that calls itself OpenAI-compatible. Newer models from OpenAI itself
/// answer it with a 400 and name `max_completion_tokens` right in the error text —
/// so the choice is not a setting the human has to guess, but what the
/// provider reports about itself (see [`super::OpenAiProvider::stream`]).
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

pub(super) fn build_body(req: &ProviderRequest, max_field: MaxField, deepseek: bool) -> Value {
    let thinking_disabled = deepseek && req.effort == Some(Effort::None);
    let replay_reasoning = deepseek && !thinking_disabled && !req.tools.is_empty();
    let mut body = json!({
        "model": req.model,
        "stream": true,
        "stream_options": { "include_usage": true },
        "messages": wire_messages(req, replay_reasoning),
    });
    body[max_field.key()] = json!(req.max_tokens);

    if !req.tools.is_empty() {
        body["tools"] = wire_tools(&req.tools);
    }
    if thinking_disabled {
        body["thinking"] = json!({ "type": "disabled" });
    } else if let Some(effort) = req.effort.filter(|effort| *effort != Effort::Auto) {
        body["reasoning_effort"] = json!(effort.as_api());
    }
    body
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

fn wire_messages(req: &ProviderRequest, replay_reasoning: bool) -> Value {
    let mut out = vec![json!({ "role": "system", "content": req.system })];
    for msg in &req.messages {
        match msg.role {
            Role::User => push_user(&mut out, msg),
            Role::Assistant => push_assistant(&mut out, msg, replay_reasoning),
        }
    }
    Value::Array(out)
}

/// One user message can become several.
///
/// Tool results in this shape are separate messages with the `tool` role, and they
/// must follow the call directly. So they go first, and the human's text goes after
/// them, even if it stood earlier in the blocks.
fn push_user(out: &mut Vec<Value>, msg: &Message) {
    // Text and images in the order they came: the line naming an image
    // stands right before it.
    let mut parts: Vec<Value> = Vec::new();
    let mut pictured = false;

    for block in &msg.content {
        match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                // There is no failure flag here, and the model must tell a refusal
                // from a result: without a marker, a failed command reads as
                // a successful one. The prefix is in English — the model reads it.
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
            ContentBlock::Text { text } => parts.push(json!({ "type": "text", "text": text })),
            // A `tool` message carries text only, which is why an image a
            // tool read arrives as a block of its own and lands here, in the
            // user message after the results.
            ContentBlock::Image {
                media_type, data, ..
            } => {
                pictured = true;
                parts.push(json!({
                    "type": "image_url",
                    "image_url": { "url": format!("data:{media_type};base64,{data}") },
                }));
            }
            // A reasoning block and a tool call do not appear in the human's message.
            _ => {}
        }
    }

    if parts.is_empty() {
        return;
    }
    // Plain text stays a string: that is the form every server in this shape
    // takes, and the list of parts is one only those with vision do.
    let content = if pictured {
        Value::Array(parts)
    } else {
        let lines: Vec<&str> = parts.iter().filter_map(|p| p["text"].as_str()).collect();
        Value::String(lines.join("\n"))
    };
    out.push(json!({ "role": "user", "content": content }));
}

fn push_assistant(out: &mut Vec<Value>, msg: &Message, replay_reasoning: bool) {
    let mut text = String::new();
    let mut calls = Vec::new();
    let mut reasoning = String::new();

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
                    // Arguments here are a string, not an object.
                    "arguments": input.to_string(),
                },
            })),
            ContentBlock::Thinking {
                thinking,
                signature: None,
            } if replay_reasoning => reasoning.push_str(thinking),
            ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => {}
            ContentBlock::ToolResult { .. } | ContentBlock::Image { .. } => {}
        }
    }

    if text.is_empty() && calls.is_empty() && reasoning.is_empty() {
        return;
    }

    let mut message = json!({ "role": "assistant", "content": text });
    if replay_reasoning {
        // Older or compacted histories can have no reasoning for a message.
        // DeepSeek still expects the field, so send an empty string in that case.
        message["reasoning_content"] = json!(reasoning);
    }
    if !calls.is_empty() {
        message["tool_calls"] = Value::Array(calls);
    }
    out.push(message);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_body(req: &ProviderRequest, max_field: MaxField) -> Value {
        super::build_body(req, max_field, false)
    }

    fn req() -> ProviderRequest {
        ProviderRequest {
            model: "gpt-oss".into(),
            system: "You are Yaca".into(),
            messages: vec![Message::user_text("hello")],
            tools: vec![],
            max_tokens: 8_000,
            effort: Some(Effort::Extra),
            thinking: true,
        }
    }

    #[test]
    fn asks_for_counters_explicitly() {
        let body = build_body(&req(), MaxField::default());
        assert_eq!(body["stream_options"]["include_usage"], true);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn effort_is_sent_without_collapsing_distinct_levels() {
        assert_eq!(
            build_body(&req(), MaxField::default())["reasoning_effort"],
            "xhigh"
        );
        let mut r = req();
        r.effort = Some(Effort::Low);
        assert_eq!(
            build_body(&r, MaxField::default())["reasoning_effort"],
            "low"
        );
        r.effort = None;
        assert!(build_body(&r, MaxField::default())
            .get("reasoning_effort")
            .is_none());
    }

    #[test]
    fn the_system_prompt_becomes_the_first_message() {
        let body = build_body(&req(), MaxField::default());
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "You are Yaca");
        assert_eq!(body["messages"][1]["role"], "user");
    }

    #[test]
    fn a_tool_call_becomes_tool_calls_with_arguments_as_a_string() {
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
    fn a_tool_result_goes_as_a_separate_tool_role_message() {
        let mut r = req();
        r.messages = vec![Message {
            role: Role::User,
            content: vec![
                ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "done".into(),
                    is_error: false,
                },
                ContentBlock::Text {
                    text: "thanks".into(),
                },
            ],
        }];
        let body = build_body(&r, MaxField::default());
        assert_eq!(body["messages"][1]["role"], "tool");
        assert_eq!(body["messages"][1]["tool_call_id"], "call_1");
        // The human's text goes after the result, not before it.
        assert_eq!(body["messages"][2]["role"], "user");
        assert_eq!(body["messages"][2]["content"], "thanks");
    }

    #[test]
    fn an_image_a_tool_read_follows_its_result_in_a_user_message() {
        let mut r = req();
        r.messages = vec![Message {
            role: Role::User,
            content: vec![
                ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "shot.png, 2×2".into(),
                    is_error: false,
                },
                ContentBlock::Text {
                    text: "Image: shot.png".into(),
                },
                ContentBlock::Image {
                    media_type: "image/png".into(),
                    data: "AAAA".into(),
                    width: 2,
                    height: 2,
                },
            ],
        }];
        let body = build_body(&r, MaxField::default());

        assert_eq!(body["messages"][1]["role"], "tool");
        assert_eq!(body["messages"][1]["content"], "shot.png, 2×2");
        let parts = &body["messages"][2]["content"];
        assert_eq!(body["messages"][2]["role"], "user");
        assert_eq!(parts[0]["text"], "Image: shot.png");
        assert_eq!(parts[1]["type"], "image_url");
        assert_eq!(parts[1]["image_url"]["url"], "data:image/png;base64,AAAA");
    }

    #[test]
    fn a_failed_tool_is_marked_in_the_text() {
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
    fn reasoning_is_not_sent_back() {
        let mut r = req();
        r.messages = vec![Message::assistant(vec![
            ContentBlock::Thinking {
                thinking: "I think".into(),
                signature: Some("sig".into()),
            },
            ContentBlock::Text {
                text: "answer".into(),
            },
        ])];
        let body = serde_json::to_string(&build_body(&r, MaxField::default())).unwrap();
        assert!(!body.contains("I think"));
        assert!(!body.contains("sig"));
        assert!(body.contains("answer"));
    }

    #[test]
    fn deepseek_replays_reasoning_with_tools_and_accepts_older_history() {
        let mut r = req();
        r.tools.push(ToolSchema {
            name: "Read".into(),
            description: "Read a file".into(),
            input_schema: json!({ "type": "object" }),
        });
        r.messages.extend([
            Message::assistant(vec![ContentBlock::Text {
                text: "Ready.".into(),
            }]),
            Message::user_text("Read a.rs"),
            Message::assistant(vec![
                ContentBlock::Thinking {
                    thinking: "Read ".into(),
                    signature: None,
                },
                ContentBlock::Thinking {
                    thinking: "a.rs".into(),
                    signature: None,
                },
                ContentBlock::ToolUse {
                    id: "call_1".into(),
                    name: "Read".into(),
                    input: json!({ "path": "a.rs" }),
                },
            ]),
            Message::tool_results(vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".into(),
                content: "file contents".into(),
                is_error: false,
            }]),
        ]);

        let body = super::build_body(&r, MaxField::default(), true);
        let messages = &body["messages"];
        assert_eq!(messages[2]["reasoning_content"], "");
        assert_eq!(messages[4]["reasoning_content"], "Read a.rs");
        assert_eq!(messages[4]["tool_calls"][0]["id"], "call_1");
        assert_eq!(messages[5]["tool_call_id"], "call_1");
        assert!(build_body(&r, MaxField::default())["messages"][4]
            .get("reasoning_content")
            .is_none());

        r.effort = Some(Effort::None);
        let body = super::build_body(&r, MaxField::default(), true);
        assert_eq!(body["thinking"]["type"], "disabled");
        assert!(body.get("reasoning_effort").is_none());
        assert!(body["messages"][4].get("reasoning_content").is_none());

        r.effort = None;
        r.tools.clear();
        let body = super::build_body(&r, MaxField::default(), true);
        assert!(body["messages"][4].get("reasoning_content").is_none());
    }

    #[test]
    fn a_turn_of_only_reasoning_gives_no_empty_message() {
        let mut r = req();
        r.messages = vec![Message::assistant(vec![ContentBlock::Thinking {
            thinking: "I think".into(),
            signature: None,
        }])];
        // Only the system prompt: there was nothing to send.
        assert_eq!(
            build_body(&r, MaxField::default())["messages"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn the_output_limit_goes_as_one_field_and_never_as_two() {
        let by_default = build_body(&req(), MaxField::MaxTokens);
        assert_eq!(by_default["max_tokens"], 8_000);
        assert!(by_default.get("max_completion_tokens").is_none());

        // The provider asked for another name — that one goes out, and again only one.
        let asked = build_body(&req(), MaxField::MaxCompletionTokens);
        assert_eq!(asked["max_completion_tokens"], 8_000);
        assert!(
            asked.get("max_tokens").is_none(),
            "sending both fields at once is a 400"
        );
    }
}
