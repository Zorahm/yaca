//! Провайдер поверх Chat Completions.
//!
//! Форма, которую отдают Ollama, LM Studio, vLLM, сам OpenAI, Groq, Together,
//! DeepSeek и OpenRouter. Отличается от Messages API не только телом: здесь
//! нет ни индексов блоков, ни явного конца сообщения, поэтому разбор потока
//! ведёт состояние — см. [`events`].
//!
//! Чего эта форма не умеет и что поэтому теряется: подпись рассуждения, запись
//! в кеш промпта и уровни усилия выше `high`. Каждое место помечено там, где
//! перевод их выбрасывает.
//!
//! Одну вещь провайдер о себе рассказывает сам — каким полем принимать потолок
//! ответа. См. [`OpenAiProvider::stream`].

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
    /// Просил ли провайдер `max_completion_tokens` вместо `max_tokens`.
    ///
    /// Живёт на провайдере, а не в описании: это свойство модели, а не
    /// адреса, — один и тот же шлюз отдаёт и те, и другие. Не переживает
    /// сессию, и правильно: следующий запуск спросит заново и узнает, если
    /// модель сменилась.
    completion_tokens: AtomicBool,
}

impl OpenAiProvider {
    /// Ключ необязателен: локальный шлюз его обычно не спрашивает, и
    /// требовать его значило бы не пускать Yaca к Ollama на своей же машине.
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
            .post(self.spec.endpoint("/v1/chat/completions"))
            .headers(headers)
            .json(&body::build_body(req, max_field))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let resp = http::check_status(resp, &self.spec, self.key.as_deref()).await?;
        Ok(http::pump(resp, events::Decoder::default()))
    }
}

/// Отказ, в котором провайдер сам назвал поле, которое хочет вместо нашего.
///
/// Текст у OpenAI однозначный: `Unsupported parameter: 'max_tokens' is not
/// supported with this model. Use 'max_completion_tokens' instead.` Проверяется
/// именно упоминание имени, а не код и не модель: угадывать по имени модели —
/// это список, который устаревает к следующему релизу.
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

    /// Запрос, а при нужде — второй.
    ///
    /// Потолок ответа зовётся `max_tokens` почти везде и `max_completion_tokens`
    /// у новых моделей самого OpenAI. Отправить оба нельзя, угадать по адресу
    /// или имени модели — значит завести список, который устареет. Поэтому
    /// спрашиваем как принято, а если провайдер отказал и в отказе назвал
    /// другое поле — переспрашиваем им и запоминаем на сессию. Ход стоит одну
    /// лишнюю попытку один раз, а не настройку, которую человек должен угадать.
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
        // Другая причина того же кода — переспрашивать нечем.
        assert!(!asks_for_completion_tokens(&http(
            400,
            "model `gpt-nope` does not exist"
        )));
        // Тот же текст, но не 400: 429 лечится ожиданием, а не полем.
        assert!(!asks_for_completion_tokens(&http(
            429,
            "max_completion_tokens"
        )));
        assert!(!asks_for_completion_tokens(&ProviderError::Network(
            "max_completion_tokens".into()
        )));
    }
}
