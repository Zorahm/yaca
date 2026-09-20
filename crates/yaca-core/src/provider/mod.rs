//! Граница провайдера.
//!
//! Ядро не знает ни про Anthropic, ни про SSE. Оно шлёт [`ProviderRequest`]
//! и читает поток нормализованных [`ProviderEvent`]. §8 обещает работу с любыми
//! моделями, включая слабые локальные, поэтому вторая реализация появится
//! наверняка, и граница проведена сейчас, а не когда припрёт.

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

/// Провайдер по его описанию.
///
/// Ключ берётся из кейчейна под `spec.id` — и только если описание его просит:
/// шлюз без авторизации не должен требовать ключа, которого у него нет
/// (см. `ProviderSpec::needs_key`).
pub fn build(spec: registry::ProviderSpec) -> Arc<dyn Provider> {
    let key = spec
        .needs_key()
        .then(|| crate::secrets::api_key(&spec.id).0)
        .flatten();
    build_with_key(spec, key)
}

/// То же с явным ключом: этим пользуются CLI с ключом из окружения и тесты.
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
    /// `output_config.effort`. `None` — модель не умеет, поле не отправляется.
    pub effort: Option<Effort>,
    /// Просить ли рассуждение. Дисплей всегда `summarized`: §5.3 рисует текст
    /// рассуждения, а по умолчанию API отдаёт пустой.
    pub thinking: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Нормализованное событие потока. Индекс — позиция блока в ответе;
/// провайдер обязан выдавать его монотонно, ядро на этом строит склейку.
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
    /// Аргументы тула приходят кусками JSON и склеиваются до `BlockStop`.
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
