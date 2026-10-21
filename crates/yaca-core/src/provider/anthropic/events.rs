//! Parsing of the Messages API stream into normalized `ProviderEvent`s.
//!
//! An unknown block or event type is passed over rather than breaking the stream: a response,
//! in which a server-side tool arrived still contains the text the human is
//! waiting for, and failing here would cost them a whole turn.

use serde::{Deserialize, Serialize};

use crate::error::ProviderError;
use crate::event::StopReason;
use crate::provider::http::StreamDecoder;
use crate::provider::{BlockKind, ProviderEvent};
use crate::usage::TokenCounts;

/// The Messages API closes the message itself: `message_stop` arrives in the stream,
/// and there is nothing to append to the tail once the bytes have been cut off.
/// A stream that ends without it was cut, and is reported as such.
#[derive(Default)]
pub(super) struct Decoder {
    stopped: bool,
}

impl StreamDecoder for Decoder {
    fn payload(&mut self, payload: &str) -> Result<Vec<ProviderEvent>, ProviderError> {
        let events = decode_payload(payload)?;
        self.stopped |= events.contains(&ProviderEvent::MessageStop);
        Ok(events)
    }

    fn cut_off(&self) -> bool {
        !self.stopped
    }
}

fn decode_payload(payload: &str) -> Result<Vec<ProviderEvent>, ProviderError> {
    let raw: StreamEvent = serde_json::from_str(payload)
        .map_err(|e| ProviderError::Protocol(format!("{e}: {payload}")))?;

    Ok(match raw {
        StreamEvent::MessageStart { message } => vec![ProviderEvent::MessageStart {
            model: message.model,
            usage: message.usage.into(),
        }],
        StreamEvent::ContentBlockStart {
            index,
            content_block,
        } => match content_block {
            WireBlockStart::Text { text } => {
                let mut evs = vec![ProviderEvent::BlockStart {
                    index,
                    kind: BlockKind::Text,
                }];
                if !text.is_empty() {
                    evs.push(ProviderEvent::TextDelta { index, text });
                }
                evs
            }
            WireBlockStart::Thinking { thinking } => {
                let mut evs = vec![ProviderEvent::BlockStart {
                    index,
                    kind: BlockKind::Thinking,
                }];
                if !thinking.is_empty() {
                    evs.push(ProviderEvent::ThinkingDelta {
                        index,
                        text: thinking,
                    });
                }
                evs
            }
            WireBlockStart::RedactedThinking { data } => vec![ProviderEvent::BlockStart {
                index,
                kind: BlockKind::RedactedThinking { data },
            }],
            WireBlockStart::ToolUse { id, name } => vec![ProviderEvent::BlockStart {
                index,
                kind: BlockKind::ToolUse { id, name },
            }],
            // Server tools and block types we do not know yet: silently passed over.
            // Failing on an unknown block type is the worst possible answer.
            WireBlockStart::Unknown => vec![],
        },
        StreamEvent::ContentBlockDelta { index, delta } => match delta {
            WireDelta::TextDelta { text } => vec![ProviderEvent::TextDelta { index, text }],
            WireDelta::ThinkingDelta { thinking } => vec![ProviderEvent::ThinkingDelta {
                index,
                text: thinking,
            }],
            WireDelta::SignatureDelta { signature } => {
                vec![ProviderEvent::SignatureDelta { index, signature }]
            }
            WireDelta::InputJsonDelta { partial_json } => {
                vec![ProviderEvent::ToolInputDelta {
                    index,
                    partial_json,
                }]
            }
            WireDelta::Unknown => vec![],
        },
        StreamEvent::ContentBlockStop { index } => vec![ProviderEvent::BlockStop { index }],
        StreamEvent::MessageDelta { delta, usage } => vec![ProviderEvent::MessageDelta {
            stop_reason: delta.stop_reason.as_deref().and_then(parse_stop_reason),
            usage: usage.into(),
        }],
        StreamEvent::MessageStop => vec![ProviderEvent::MessageStop],
        StreamEvent::Ping => vec![],
        StreamEvent::Error { error } => {
            return Err(ProviderError::Protocol(format!(
                "{}: {}",
                error.r#type, error.message
            )))
        }
        StreamEvent::Unknown => vec![],
    })
}

fn parse_stop_reason(s: &str) -> Option<StopReason> {
    Some(match s {
        "end_turn" => StopReason::EndTurn,
        "max_tokens" => StopReason::MaxTokens,
        "stop_sequence" => StopReason::StopSequence,
        "tool_use" => StopReason::ToolUse,
        "refusal" => StopReason::Refusal,
        "pause_turn" => StopReason::PauseTurn,
        _ => return None,
    })
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamEvent {
    MessageStart {
        message: WireMessageStart,
    },
    ContentBlockStart {
        index: u32,
        content_block: WireBlockStart,
    },
    ContentBlockDelta {
        index: u32,
        delta: WireDelta,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: WireMessageDelta,
        #[serde(default)]
        usage: WireUsage,
    },
    MessageStop,
    Ping,
    Error {
        error: WireError,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct WireMessageStart {
    model: String,
    #[serde(default)]
    usage: WireUsage,
}

#[derive(Debug, Deserialize)]
struct WireMessageDelta {
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WireError {
    #[serde(default, rename = "type")]
    r#type: String,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct WireUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
    #[serde(default)]
    cache_creation_input_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: u32,
}

impl From<WireUsage> for TokenCounts {
    fn from(u: WireUsage) -> Self {
        TokenCounts {
            input: u.input_tokens,
            output: u.output_tokens,
            cache_write: u.cache_creation_input_tokens,
            cache_read: u.cache_read_input_tokens,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireBlockStart {
    Text {
        #[serde(default)]
        text: String,
    },
    Thinking {
        #[serde(default)]
        thinking: String,
    },
    RedactedThinking {
        #[serde(default)]
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireDelta {
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        thinking: String,
    },
    SignatureDelta {
        signature: String,
    },
    InputJsonDelta {
        partial_json: String,
    },
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_block_type_does_not_break_the_stream() {
        let payload = r#"{"type":"content_block_start","index":0,
            "content_block":{"type":"server_tool_use","id":"x","name":"web_search"}}"#;
        assert_eq!(decode_payload(payload).unwrap(), vec![]);
    }

    #[test]
    fn an_unknown_event_does_not_break_the_stream() {
        assert_eq!(decode_payload(r#"{"type":"whatever"}"#).unwrap(), vec![]);
    }

    #[test]
    fn parses_reasoning_and_text_deltas() {
        let think = decode_payload(
            r#"{"type":"content_block_delta","index":0,
                "delta":{"type":"thinking_delta","thinking":"aha"}}"#,
        )
        .unwrap();
        assert_eq!(
            think,
            vec![ProviderEvent::ThinkingDelta {
                index: 0,
                text: "aha".into()
            }]
        );

        let text = decode_payload(
            r#"{"type":"content_block_delta","index":1,
                "delta":{"type":"text_delta","text":"answer"}}"#,
        )
        .unwrap();
        assert_eq!(
            text,
            vec![ProviderEvent::TextDelta {
                index: 1,
                text: "answer".into()
            }]
        );
    }

    #[test]
    fn a_stream_is_cut_off_until_message_stop_arrives() {
        let mut d = Decoder::default();
        assert!(d.cut_off(), "nothing has arrived yet");
        d.payload(r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hm"}}"#)
            .unwrap();
        assert!(d.cut_off(), "the connection dropped mid-thought");
        d.payload(r#"{"type":"message_stop"}"#).unwrap();
        assert!(!d.cut_off());
    }

    #[test]
    fn reads_cache_counters() {
        let evs = decode_payload(
            r#"{"type":"message_start","message":{"model":"claude-opus-5",
                "usage":{"input_tokens":10,"output_tokens":0,
                "cache_creation_input_tokens":2,"cache_read_input_tokens":8000}}}"#,
        )
        .unwrap();
        match &evs[0] {
            ProviderEvent::MessageStart { usage, .. } => {
                assert_eq!(usage.cache_read, 8000);
                assert_eq!(usage.cache_write, 2);
            }
            other => panic!("not the expected event: {other:?}"),
        }
    }
}
