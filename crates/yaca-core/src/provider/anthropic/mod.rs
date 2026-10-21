//! Provider on top of the Messages API.
//!
//! Not only `api.anthropic.com`: the address, headers and model list come in a
//! description (`registry::ProviderSpec`), so the same implementation talks to a
//! corporate proxy, OpenRouter, LiteLLM and anything else that serves
//! `/v1/messages` in the Anthropic shape.
//!
//! Building the body is in [`body`], parsing the stream is in [`events`], and the
//! HTTP code shared with the OpenAI shape is in [`super::http`].

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
    /// The key is optional: the description may not ask for it at all — see
    /// `ProviderSpec::needs_key`.
    pub fn new(spec: ProviderSpec, key: Option<String>) -> Self {
        AnthropicProvider {
            http: reqwest::Client::new(),
            spec,
            key,
        }
    }

    /// Built-in Anthropic with a key. A short path for the CLI and tests.
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
            .post(self.spec.api("/messages"))
            .headers(headers)
            .json(&body::build_body(&req))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let resp = http::check_status(resp, &self.spec, self.key.as_deref()).await?;
        Ok(http::pump(resp, events::Decoder::default()))
    }
}
