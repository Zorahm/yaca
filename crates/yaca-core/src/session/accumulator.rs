//! Assembles the provider stream into response blocks and feed events.
//!
//! Kept apart from the turn loop on purpose: it is a pure transformation, with
//! no network, no files and no clock, and it is checked line by line.
//!
//! The accumulator does not turn tool calls into `ToolCallStarted`: §5.4 requires
//! a present-tense caption at the moment of execution, not at the moment of parsing
//! response. The turn loop emits them when it actually starts the work.

use std::collections::BTreeMap;

use crate::event::{AgentEvent, StopReason};
use crate::id::{BlockId, TurnId};
use crate::message::ContentBlock;
use crate::provider::{BlockKind, ProviderEvent};
use crate::usage::TokenCounts;

#[derive(Debug, Clone, PartialEq)]
pub struct PendingCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    /// Why the arguments cannot be used, when they cannot. Such a call is not
    /// run, but it is in the history all the same and still has to be
    /// answered: the provider rejects a conversation with a call left
    /// without its result.
    pub garbled: Option<Garbled>,
}

/// Arguments that did not arrive as a JSON object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Garbled {
    /// The response hit its output cap while they were being written.
    Cut,
    /// Complete, but not a JSON object.
    Invalid,
}

/// The model a routing provider picked for one request, as it was reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDecision {
    /// The name of the matched decision; `None` when the default was used.
    pub route: Option<String>,
    pub provider: String,
    pub model: String,
    /// The names of the signals that were read to make the choice.
    pub matched: Vec<String>,
}

#[derive(Debug)]
pub struct Completion {
    /// Response blocks in the form they will be sent back to the history.
    pub blocks: Vec<ContentBlock>,
    pub calls: Vec<PendingCall>,
    pub stop_reason: StopReason,
    pub usage: TokenCounts,
    /// `(provider, model)` the request was routed to, from the last `Routed`
    /// the request carried; `None` when the provider did not route it. The
    /// request is priced by this model, not by the session's one: the session
    /// knows only the virtual model a router presents. The pair is what
    /// identifies a model in the list (`ModelInfo::provider_id`), and the
    /// provider's own `MessageStart.model` is not used — aliases and dated ids
    /// there do not match the configured ids.
    pub routed: Option<(String, String)>,
}

#[derive(Debug)]
enum Partial {
    Text {
        id: BlockId,
        text: String,
    },
    Thinking {
        id: BlockId,
        text: String,
        signature: Option<String>,
    },
    Redacted {
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        json: String,
    },
}

#[derive(Debug)]
pub struct Accumulator {
    turn_id: TurnId,
    blocks: BTreeMap<u32, Partial>,
    stop_reason: Option<StopReason>,
    usage: TokenCounts,
    /// The last `Routed` of this request. Not a feed event here: the turn
    /// decides whether the human sees it, once per turn (`turn::steps`).
    route: Option<RouteDecision>,
}

impl Accumulator {
    pub fn new(turn_id: TurnId) -> Self {
        Accumulator {
            turn_id,
            blocks: BTreeMap::new(),
            stop_reason: None,
            usage: TokenCounts::default(),
            route: None,
        }
    }

    /// The model this request was routed to, as far as it has been reported.
    pub fn route(&self) -> Option<&RouteDecision> {
        self.route.as_ref()
    }

    /// Feeds in a provider event, returns the feed events it produces.
    pub fn push(&mut self, event: ProviderEvent) -> Vec<AgentEvent> {
        match event {
            ProviderEvent::Routed {
                route,
                provider,
                model,
                matched,
            } => {
                self.route = Some(RouteDecision {
                    route,
                    provider,
                    model,
                    matched,
                });
                vec![]
            }

            ProviderEvent::MessageStart { usage, .. } => {
                self.usage.add(usage);
                vec![]
            }

            ProviderEvent::BlockStart { index, kind } => {
                let partial = match kind {
                    BlockKind::Text => Partial::Text {
                        id: BlockId::new(),
                        text: String::new(),
                    },
                    BlockKind::Thinking => Partial::Thinking {
                        id: BlockId::new(),
                        text: String::new(),
                        signature: None,
                    },
                    BlockKind::RedactedThinking { data } => Partial::Redacted { data },
                    BlockKind::ToolUse { id, name } => Partial::ToolUse {
                        id,
                        name,
                        json: String::new(),
                    },
                };
                self.blocks.insert(index, partial);
                vec![]
            }

            ProviderEvent::TextDelta { index, text } => {
                let Some(Partial::Text { id, text: buf }) = self.blocks.get_mut(&index) else {
                    // A delta without a block start: the provider got the indexes wrong.
                    // Text must not be lost, so a block is opened on the spot.
                    return self.orphan_text(index, text);
                };
                buf.push_str(&text);
                vec![AgentEvent::TextDelta {
                    turn_id: self.turn_id.clone(),
                    block_id: id.clone(),
                    text,
                }]
            }

            ProviderEvent::ThinkingDelta { index, text } => {
                let Some(Partial::Thinking { id, text: buf, .. }) = self.blocks.get_mut(&index)
                else {
                    return vec![];
                };
                buf.push_str(&text);
                vec![AgentEvent::ThinkingDelta {
                    turn_id: self.turn_id.clone(),
                    block_id: id.clone(),
                    text,
                }]
            }

            ProviderEvent::SignatureDelta { index, signature } => {
                if let Some(Partial::Thinking { signature: sig, .. }) = self.blocks.get_mut(&index)
                {
                    // The signature must go back unchanged, otherwise
                    // the next request will be rejected.
                    sig.get_or_insert_with(String::new).push_str(&signature);
                }
                vec![]
            }

            ProviderEvent::ToolInputDelta {
                index,
                partial_json,
            } => {
                if let Some(Partial::ToolUse { json, .. }) = self.blocks.get_mut(&index) {
                    json.push_str(&partial_json);
                }
                vec![]
            }

            ProviderEvent::BlockStop { .. } => vec![],

            ProviderEvent::MessageDelta { stop_reason, usage } => {
                self.usage.add(usage);
                if let Some(sr) = stop_reason {
                    self.stop_reason = Some(sr);
                }
                vec![]
            }

            ProviderEvent::MessageStop => vec![],
        }
    }

    fn orphan_text(&mut self, index: u32, text: String) -> Vec<AgentEvent> {
        let id = BlockId::new();
        self.blocks.insert(
            index,
            Partial::Text {
                id: id.clone(),
                text: text.clone(),
            },
        );
        vec![AgentEvent::TextDelta {
            turn_id: self.turn_id.clone(),
            block_id: id,
            text,
        }]
    }

    pub fn finish(self) -> Completion {
        let mut blocks = Vec::new();
        let mut calls = Vec::new();

        // Only the block being written when the cap hit can have been cut
        // short; an earlier call that does not parse was sent that way.
        let cut_at = (self.stop_reason == Some(StopReason::MaxTokens))
            .then(|| self.blocks.keys().next_back().copied())
            .flatten();

        for (index, partial) in self.blocks {
            match partial {
                Partial::Text { text, .. } => {
                    if !text.is_empty() {
                        blocks.push(ContentBlock::Text { text });
                    }
                }
                Partial::Thinking {
                    text, signature, ..
                } => blocks.push(ContentBlock::Thinking {
                    thinking: text,
                    signature,
                }),
                Partial::Redacted { data } => blocks.push(ContentBlock::RedactedThinking { data }),
                Partial::ToolUse { id, name, json } => {
                    // An empty string is a legal case: a tool with no arguments.
                    let parsed = if json.trim().is_empty() {
                        Ok(serde_json::json!({}))
                    } else {
                        serde_json::from_str::<serde_json::Value>(&json)
                    };
                    // The history still needs an object here: the provider
                    // refuses a `tool_use` whose input is anything else.
                    let (input, garbled) = match parsed {
                        Ok(value) if value.is_object() => (value, None),
                        _ if cut_at == Some(index) => (serde_json::json!({}), Some(Garbled::Cut)),
                        _ => (serde_json::json!({}), Some(Garbled::Invalid)),
                    };
                    blocks.push(ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });
                    calls.push(PendingCall {
                        id,
                        name,
                        input,
                        garbled,
                    });
                }
            }
        }

        let routed = self.route.map(|r| (r.provider, r.model));

        Completion {
            blocks,
            calls,
            // A stream cut off without a `message_delta` counts as the turn having ended.
            stop_reason: self.stop_reason.unwrap_or(StopReason::EndTurn),
            usage: self.usage,
            routed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::mock;

    fn feed(events: Vec<ProviderEvent>) -> (Vec<AgentEvent>, Completion) {
        let mut acc = Accumulator::new(TurnId::from("turn_1"));
        let mut out = Vec::new();
        for ev in events {
            out.extend(acc.push(ev));
        }
        (out, acc.finish())
    }

    #[test]
    fn text_concatenates_into_one_block_and_streams_as_deltas() {
        let (events, done) = feed(mock::script_text("hello"));

        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], AgentEvent::TextDelta { text, .. } if text == "hello"));
        assert_eq!(
            done.blocks,
            vec![ContentBlock::Text {
                text: "hello".into()
            }]
        );
        assert_eq!(done.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn deltas_of_the_same_block_share_one_block_id() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        acc.push(ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::Text,
        });
        let a = acc.push(ProviderEvent::TextDelta {
            index: 0,
            text: "one".into(),
        });
        let b = acc.push(ProviderEvent::TextDelta {
            index: 0,
            text: "two".into(),
        });

        let id = |e: &AgentEvent| match e {
            AgentEvent::TextDelta { block_id, .. } => block_id.clone(),
            _ => panic!("not a delta"),
        };
        assert_eq!(id(&a[0]), id(&b[0]));
        assert_eq!(
            acc.finish().blocks[0],
            ContentBlock::Text {
                text: "onetwo".into()
            }
        );
    }

    #[test]
    fn tool_arguments_are_assembled_from_chunks() {
        let (_, done) = feed(mock::script_tool_call(
            "toolu_1",
            "Read",
            serde_json::json!({ "path": "src/main.rs", "description": "Read entry point" }),
        ));

        assert_eq!(done.stop_reason, StopReason::ToolUse);
        assert_eq!(done.calls.len(), 1);
        assert_eq!(done.calls[0].name, "Read");
        assert_eq!(done.calls[0].input["path"], "src/main.rs");
    }

    #[test]
    fn corrupted_arguments_json_does_not_crash_turn() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        acc.push(ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::ToolUse {
                id: "t1".into(),
                name: "Grep".into(),
            },
        });
        acc.push(ProviderEvent::ToolInputDelta {
            index: 0,
            partial_json: "{\"pattern\": ".into(),
        });
        let done = acc.finish();
        assert_eq!(done.calls[0].input, serde_json::json!({}));
        assert_eq!(done.calls[0].garbled, Some(Garbled::Invalid));
    }

    fn tool_block(acc: &mut Accumulator, index: u32, id: &str, json: &str) {
        acc.push(ProviderEvent::BlockStart {
            index,
            kind: BlockKind::ToolUse {
                id: id.into(),
                name: "Write".into(),
            },
        });
        acc.push(ProviderEvent::ToolInputDelta {
            index,
            partial_json: json.into(),
        });
        acc.push(ProviderEvent::BlockStop { index });
    }

    #[test]
    fn only_the_last_call_can_be_cut_by_the_output_cap() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        tool_block(&mut acc, 0, "whole", r#"{"path": "a.txt", "content": "a"}"#);
        tool_block(&mut acc, 1, "broken", r#"{"path": "b.txt"}}"#);
        tool_block(&mut acc, 2, "cut", r#"{"path": "c.txt", "content": "cc"#);
        acc.push(ProviderEvent::MessageDelta {
            stop_reason: Some(StopReason::MaxTokens),
            usage: TokenCounts::default(),
        });

        let garbled: Vec<_> = acc.finish().calls.iter().map(|c| c.garbled).collect();
        // The broken one was finished before the cap came, so it was sent
        // broken: telling the model it ran out of room would be untrue.
        assert_eq!(garbled, vec![None, Some(Garbled::Invalid), Some(Garbled::Cut)]);
    }

    #[test]
    fn arguments_that_are_json_but_not_an_object_are_garbled() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        tool_block(&mut acc, 0, "t1", r#"["a.txt"]"#);
        let done = acc.finish();
        assert_eq!(done.calls[0].garbled, Some(Garbled::Invalid));
        // The history still gets an object: the provider accepts nothing else.
        assert_eq!(done.calls[0].input, serde_json::json!({}));
    }

    #[test]
    fn a_tool_without_arguments_is_not_garbled() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        tool_block(&mut acc, 0, "t1", "");
        assert_eq!(acc.finish().calls[0].garbled, None);
    }

    #[test]
    fn thinking_signature_is_preserved_for_echoing_back() {
        let (events, done) = feed(mock::script_thinking("thinking", "answer"));

        assert!(matches!(events[0], AgentEvent::ThinkingDelta { .. }));
        assert!(matches!(events[1], AgentEvent::TextDelta { .. }));
        assert_eq!(
            done.blocks[0],
            ContentBlock::Thinking {
                thinking: "thinking".into(),
                signature: Some("sig".into()),
            }
        );
    }

    #[test]
    fn token_counts_are_summed_from_message_start_and_end() {
        let (_, done) = feed(mock::script_text("x"));
        assert_eq!(done.usage.input, 100);
        assert_eq!(done.usage.output, 20);
    }

    #[test]
    fn a_routed_request_reports_its_model_without_a_feed_event() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        let out = acc.push(ProviderEvent::Routed {
            route: Some("code".into()),
            provider: "local".into(),
            model: "qwen-coder".into(),
            matched: vec!["keyword:refactor".into()],
        });
        assert!(out.is_empty(), "the turn decides what is shown: {out:?}");
        assert_eq!(acc.route().map(|r| r.model.as_str()), Some("qwen-coder"));

        for ev in mock::script_text("hi") {
            acc.push(ev);
        }
        let done = acc.finish();
        assert_eq!(
            done.routed,
            Some(("local".to_string(), "qwen-coder".to_string()))
        );
    }

    #[test]
    fn a_request_without_routing_has_no_routed_model() {
        let (_, done) = feed(mock::script_text("hi"));
        assert_eq!(done.routed, None);
    }

    #[test]
    fn delta_without_block_start_does_not_lose_text() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        let out = acc.push(ProviderEvent::TextDelta {
            index: 7,
            text: "orphaned".into(),
        });
        assert_eq!(out.len(), 1);
        assert_eq!(
            acc.finish().blocks[0],
            ContentBlock::Text {
                text: "orphaned".into()
            }
        );
    }
}
