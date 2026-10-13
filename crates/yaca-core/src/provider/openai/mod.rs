//! Provider on top of Chat Completions.
//!
//! The shape served by Ollama, LM Studio, vLLM, OpenAI itself, Groq, Together,
//! DeepSeek and OpenRouter. It differs from the Messages API not only in the body: here
//! there are no block indexes and no explicit end of the message, so parsing the stream
//! keeps state — see [`events`].
//!
//! What this shape cannot do, and so what is lost: the reasoning signature, writing
//! to the prompt cache, and effort levels above `high`. Each place is marked where
//! the translation drops them.
//!
//! One thing the provider tells us about itself — which field it takes the response
//! ceiling in. See [`OpenAiProvider::stream`].

mod body;
mod events;

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;

use super::http;
use super::registry::ProviderSpec;
use super::{Provider, ProviderRequest, ProviderStream};
use crate::error::ProviderError;
use body::MaxField;

pub struct OpenAiProvider {
    http: reqwest::Client,
    spec: ProviderSpec,
    key: Option<String>,
    /// Whether the provider asked for `max_completion_tokens` instead of `max_tokens`.
    ///
    /// It lives on the provider, not in the description: this is a property of the model, not of
    /// the address — one and the same gateway serves both kinds. It does not outlive
    /// the session, and that is right: the next launch asks again and finds out if
    /// the model has changed.
    completion_tokens: AtomicBool,
}

impl OpenAiProvider {
    /// The key is optional: a local gateway usually does not ask for it, and
    /// requiring it would keep Yaca from reaching Ollama on the same machine.
    pub fn new(spec: ProviderSpec, key: Option<String>) -> Self {
        OpenAiProvider {
            http: reqwest::Client::new(),
            spec,
            key,
            completion_tokens: AtomicBool::new(false),
        }
    }

    fn max_field(&self) -> MaxField {
        if self.completion_tokens.load(Ordering::Relaxed) {
            MaxField::MaxCompletionTokens
        } else {
            MaxField::MaxTokens
        }
    }

    async fn post(
        &self,
        req: &ProviderRequest,
        max_field: MaxField,
    ) -> Result<ProviderStream, ProviderError> {
        let headers = http::header_map(&self.spec, self.key.as_deref())?;

        let resp = self
            .http
            .post(self.spec.api("/chat/completions"))
            .headers(headers)
            .json(&body::build_body(req, max_field))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let resp = http::check_status(resp, &self.spec, self.key.as_deref()).await?;
        Ok(http::pump(resp, events::Decoder::default()))
    }
}

/// Refusal in which the provider itself named the field it wants instead of ours.
///
/// OpenAI's text is unambiguous: `Unsupported parameter: 'max_tokens' is not
/// supported with this model. Use 'max_completion_tokens' instead.` Only the
/// mention of the name is checked, not the code and not the model: guessing from
/// the model name is a list that goes stale by the next release.
fn asks_for_completion_tokens(err: &ProviderError) -> bool {
    matches!(
        err,
        ProviderError::Http { status: 400, message, .. } if message.contains("max_completion_tokens")
    )
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn name(&self) -> &str {
        &self.spec.id
    }

    /// The request, and if needed a second one.
    ///
    /// The response ceiling is called `max_tokens` almost everywhere and
    /// `max_completion_tokens` on newer OpenAI models. Sending both is not possible,
    /// and guessing from the address or the model name would mean keeping a list
    /// that goes stale. So we ask the usual way, and if the provider refused and
    /// named another field in the refusal, we ask again with that field and remember
    /// it for the session. A turn costs one extra attempt, once — not a setting the
    /// human has to guess.
    async fn stream(&self, req: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        let field = self.max_field();
        match self.post(&req, field).await {
            Err(e) if field == MaxField::MaxTokens && asks_for_completion_tokens(&e) => {
                self.completion_tokens.store(true, Ordering::Relaxed);
                self.post(&req, MaxField::MaxCompletionTokens).await
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http(status: u16, message: &str) -> ProviderError {
        ProviderError::Http {
            status,
            message: message.into(),
            retry_after_s: None,
        }
    }

    #[test]
    fn отказ_с_именем_поля_узнаётся() {
        assert!(asks_for_completion_tokens(&http(
            400,
            "Unsupported parameter: 'max_tokens' is not supported with this \
             model. Use 'max_completion_tokens' instead."
        )));
    }

    #[test]
    fn прочие_отказы_не_считаются_просьбой_переспросить() {
        // A different reason for the same code — there is nothing to ask again with.
        assert!(!asks_for_completion_tokens(&http(
            400,
            "model `gpt-nope` does not exist"
        )));
        // The same text, but not a 400: a 429 is cured by waiting, not by a field.
        assert!(!asks_for_completion_tokens(&http(
            429,
            "max_completion_tokens"
        )));
        assert!(!asks_for_completion_tokens(&ProviderError::Network(
            "max_completion_tokens".into()
        )));
    }
}
