//! Provider driven by a script. It backs the golden tests: the input is a recorded
//! sequence of model responses, the output is the exact `AgentEvent` stream.
//!
//! This is the core's main test: the contract the whole interface depends on
//! is checked without network, without a key and without money.

use std::sync::Mutex;

use async_trait::async_trait;
use tokio_stream::wrappers::ReceiverStream;

use super::{Provider, ProviderEvent, ProviderRequest, ProviderStream};
use crate::error::ProviderError;

/// One model turn: either a stream of events or a refusal.
pub enum Turn {
    Events(Vec<ProviderEvent>),
    Fail(ProviderError),
}

pub struct MockProvider {
    turns: Mutex<std::collections::VecDeque<Turn>>,
    /// Pause before each event. Needed by the cancellation tests: an instant stream
    /// leaves no window in which `Esc` can be pressed.
    gap: std::time::Duration,
    /// Requests received. A test can check what was sent.
    pub seen: Mutex<Vec<ProviderRequest>>,
}

impl MockProvider {
    pub fn new(turns: Vec<Turn>) -> Self {
        Self {
            turns: Mutex::new(turns.into()),
            gap: std::time::Duration::ZERO,
            seen: Mutex::new(Vec::new()),
        }
    }

    pub fn with_gap(mut self, gap: std::time::Duration) -> Self {
        self.gap = gap;
        self
    }

    /// One turn with a plain text reply.
    pub fn text(reply: &str) -> Self {
        Self::new(vec![Turn::Events(script_text(reply))])
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    async fn stream(&self, req: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        self.seen.lock().unwrap().push(req);

        let turn =
            self.turns
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Turn::Fail(ProviderError::Protocol(
                    "the script ran out: the core made more requests than the test expected".into(),
                )));

        let events = match turn {
            Turn::Events(e) => e,
            Turn::Fail(e) => return Err(e),
        };

        let gap = self.gap;
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        tokio::spawn(async move {
            for ev in events {
                if !gap.is_zero() {
                    tokio::time::sleep(gap).await;
                }
                if tx.send(Ok(ev)).await.is_err() {
                    return;
                }
            }
        });
        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}

// --- ready-made scenarios ------------------------------------------------------

use super::BlockKind;
use crate::event::StopReason;
use crate::usage::TokenCounts;

fn usage(input: u32, output: u32) -> TokenCounts {
    TokenCounts {
        input,
        output,
        ..Default::default()
    }
}

/// The model replies with text and ends the turn.
pub fn script_text(text: &str) -> Vec<ProviderEvent> {
    vec![
        ProviderEvent::MessageStart {
            model: "mock".into(),
            usage: usage(100, 0),
        },
        ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::Text,
        },
        ProviderEvent::TextDelta {
            index: 0,
            text: text.to_string(),
        },
        ProviderEvent::BlockStop { index: 0 },
        ProviderEvent::MessageDelta {
            stop_reason: Some(StopReason::EndTurn),
            usage: usage(0, 20),
        },
        ProviderEvent::MessageStop,
    ]
}

/// The model calls a tool. `input` goes out in chunks, as in real life.
pub fn script_tool_call(id: &str, name: &str, input: serde_json::Value) -> Vec<ProviderEvent> {
    let json = input.to_string();
    // Split on a character boundary: the provider cuts by bytes, but the test must not
    // fail on Cyrillic in the arguments.
    let mid = (json.len() / 2..=json.len())
        .find(|i| json.is_char_boundary(*i))
        .unwrap_or(json.len());
    let (head, tail) = json.split_at(mid);
    vec![
        ProviderEvent::MessageStart {
            model: "mock".into(),
            usage: usage(100, 0),
        },
        ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::ToolUse {
                id: id.into(),
                name: name.into(),
            },
        },
        ProviderEvent::ToolInputDelta {
            index: 0,
            partial_json: head.to_string(),
        },
        ProviderEvent::ToolInputDelta {
            index: 0,
            partial_json: tail.to_string(),
        },
        ProviderEvent::BlockStop { index: 0 },
        ProviderEvent::MessageDelta {
            stop_reason: Some(StopReason::ToolUse),
            usage: usage(0, 30),
        },
        ProviderEvent::MessageStop,
    ]
}

/// The model reasons, then answers.
pub fn script_thinking(thought: &str, text: &str) -> Vec<ProviderEvent> {
    vec![
        ProviderEvent::MessageStart {
            model: "mock".into(),
            usage: usage(100, 0),
        },
        ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::Thinking,
        },
        ProviderEvent::ThinkingDelta {
            index: 0,
            text: thought.to_string(),
        },
        ProviderEvent::SignatureDelta {
            index: 0,
            signature: "sig".into(),
        },
        ProviderEvent::BlockStop { index: 0 },
        ProviderEvent::BlockStart {
            index: 1,
            kind: BlockKind::Text,
        },
        ProviderEvent::TextDelta {
            index: 1,
            text: text.to_string(),
        },
        ProviderEvent::BlockStop { index: 1 },
        ProviderEvent::MessageDelta {
            stop_reason: Some(StopReason::EndTurn),
            usage: usage(0, 20),
        },
        ProviderEvent::MessageStop,
    ]
}
