//! Провайдер поверх Messages API.
//!
//! Не только `api.anthropic.com`: адрес, заголовки и список моделей приходят
//! описанием (`registry::ProviderSpec`), поэтому этой же реализацией
//! разговаривают корпоративный прокси, OpenRouter, LiteLLM и всё остальное,
//! что отдаёт `/v1/messages` в форме Anthropic.
//!
//! Сборка тела — в [`body`], разбор потока — в [`events`], общий с
//! OpenAI-формой HTTP — в [`super::http`].

mod body;
mod events;

use async_trait::async_trait;

use super::http;
use super::registry::ProviderSpec;
use super::{Provider, ProviderRequest, ProviderStream};
use crate::error::ProviderError;

pub struct AnthropicProvider {
    http: reqwest::Client,
    spec: ProviderSpec,
    key: Option<String>,
}

impl AnthropicProvider {
    /// Ключ необязателен: описание может не просить его вовсе — см.
    /// `ProviderSpec::needs_key`.
    pub fn new(spec: ProviderSpec, key: Option<String>) -> Self {
        AnthropicProvider {
            http: reqwest::Client::new(),
            spec,
            key,
        }
    }

    /// Встроенный Anthropic с ключом. Короткий путь для CLI и тестов.
    pub fn with_key(key: impl Into<String>) -> Self {
        Self::new(ProviderSpec::anthropic(), Some(key.into()))
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        &self.spec.id
    }

    async fn stream(&self, req: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        let headers = http::header_map(&self.spec, self.key.as_deref())?;

        let resp = self
            .http
            .post(self.spec.endpoint("/v1/messages"))
            .headers(headers)
            .json(&body::build_body(&req))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let resp = http::check_status(resp, &self.spec, self.key.as_deref()).await?;
        Ok(http::pump(resp, events::Decoder))
    }
}
