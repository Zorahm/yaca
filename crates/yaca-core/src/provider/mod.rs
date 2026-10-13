//! Provider boundary.
//!
//! The core knows nothing about Anthropic or SSE. It sends [`ProviderRequest`]
//! and reads a stream of normalized [`ProviderEvent`]. §8 promises work with any
//! model, including weak local ones, so a second implementation will surely
//! appear, and the boundary is drawn now rather than when it is needed.

pub mod anthropic;
mod http;
pub mod mock;
pub mod models;
pub mod openai;
pub mod registry;
pub mod sse;

use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::Stream;

use crate::error::ProviderError;
use crate::event::StopReason;
use crate::intent::Effort;
use crate::message::Message;
use crate::usage::TokenCounts;

pub type ProviderStream = Pin<Box<dyn Stream<Item = Result<ProviderEvent, ProviderError>> + Send>>;

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;

    async fn stream(&self, req: ProviderRequest) -> Result<ProviderStream, ProviderError>;
}

/// Provider built from its description.
///
/// The key is taken from the keychain under `spec.id` — and only if the description asks for it:
/// a gateway without authorization must not require a key it does not have
/// (see `ProviderSpec::needs_key`).
pub fn build(spec: registry::ProviderSpec) -> Arc<dyn Provider> {
    let key = spec
        .needs_key()
        .then(|| crate::secrets::api_key(&spec.id).0)
        .flatten();
    build_with_key(spec, key)
}

/// The same with an explicit key: used by the CLI with a key from the environment, and by tests.
pub fn build_with_key(spec: registry::ProviderSpec, key: Option<String>) -> Arc<dyn Provider> {
    match spec.protocol {
        registry::Protocol::Anthropic => Arc::new(anthropic::AnthropicProvider::new(spec, key)),
        registry::Protocol::OpenAi => Arc::new(openai::OpenAiProvider::new(spec, key)),
    }
}

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSchema>,
    pub max_tokens: u32,
    /// `output_config.effort`. `None` — the model does not support it, the field is not sent.
    pub effort: Option<Effort>,
    /// Whether to ask for reasoning. The display is always `summarized`: §5.3 draws the text
    /// of the reasoning, and by default the API returns an empty one.
    pub thinking: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Normalized stream event. The index is the position of the block in the response;
/// the provider must emit it monotonically, the core builds its assembly on that.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderEvent {
    MessageStart {
        model: String,
        usage: TokenCounts,
    },
    BlockStart {
        index: u32,
        kind: BlockKind,
    },
    TextDelta {
        index: u32,
        text: String,
    },
    ThinkingDelta {
        index: u32,
        text: String,
    },
    SignatureDelta {
        index: u32,
        signature: String,
    },
    /// Tool arguments arrive as JSON chunks and are concatenated up to `BlockStop`.
    ToolInputDelta {
        index: u32,
        partial_json: String,
    },
    BlockStop {
        index: u32,
    },
    MessageDelta {
        stop_reason: Option<StopReason>,
        usage: TokenCounts,
    },
    MessageStop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockKind {
    Text,
    Thinking,
    RedactedThinking { data: String },
    ToolUse { id: String, name: String },
}
