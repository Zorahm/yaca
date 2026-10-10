//! Core errors.
//!
//! Разделение важное и не косметическое: [`ToolError::Rejected`] — сработавший
//! инвариант (§5.9), интерфейс красит его `--vermilion-ink` и человеку делать нечего;
//! [`ToolError::Failed`] — настоящий сбой, `--rust`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("провайдер: {0}")]
    Provider(#[from] ProviderError),

    #[error("ввод-вывод: {0}")]
    Io(#[from] std::io::Error),

    #[error("сериализация: {0}")]
    Json(#[from] serde_json::Error),

    #[error("сессия остановлена")]
    Cancelled,

    #[error("{0}")]
    Config(String),
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("сеть: {0}")]
    Network(String),

    #[error("HTTP {status}: {message}")]
    Http {
        status: u16,
        message: String,
        /// Заголовок `retry-after` в секундах, если провайдер его прислал.
        retry_after_s: Option<u64>,
    },

    #[error("не разобрал поток: {0}")]
    Protocol(String),

    /// Провайдер описан так, что запрос не собрать: негодное имя заголовка,
    /// негодное значение. Повтор не поможет — чинить надо описание.
    #[error("описание провайдера: {0}")]
    Config(String),
}

impl ProviderError {
    /// Стоит ли повторять. Драйвит `RetryScheduled` (§5.10).
    pub fn is_retryable(&self) -> bool {
        match self {
            ProviderError::Network(_) => true,
            ProviderError::Http { status, .. } => {
                *status == 408 || *status == 429 || *status >= 500
            }
            ProviderError::Protocol(_) => false,
            ProviderError::Config(_) => false,
        }
    }

    /// The provider refused the request because it does not fit the window.
    ///
    /// Not retryable as it stands, but curable: compacting the history and
    /// sending it again (§5.11). No provider marks this with a field of its
    /// own, so it is told by the words they use — Anthropic's "prompt is too
    /// long" and "exceed context limit", OpenAI's `context_length_exceeded` and
    /// "maximum context length", which vLLM and most gateways repeat, and
    /// llama.cpp's "context size". A refusal worded otherwise is reported as
    /// the error it is, which is where things stood before.
    pub fn is_context_overflow(&self) -> bool {
        const WORDS: [&str; 5] = [
            "prompt is too long",
            "context limit",
            "context_length_exceeded",
            "maximum context length",
            "context size",
        ];
        match self {
            ProviderError::Http {
                status: 400 | 413,
                message,
                ..
            } => {
                let message = message.to_lowercase();
                WORDS.iter().any(|w| message.contains(w))
            }
            _ => false,
        }
    }

    /// The provider turned the request down for what was in it, rather than
    /// for who sent it or how much there was.
    ///
    /// The statuses a model without vision answers an image with. Nothing
    /// narrower is possible — each server words it its own way — so this only
    /// says the request is worth one more try without its images
    /// (`session::vision`).
    pub fn is_unacceptable(&self) -> bool {
        matches!(
            self,
            ProviderError::Http {
                status: 400 | 415 | 422,
                ..
            }
        ) && !self.is_context_overflow()
    }

    pub fn code(&self) -> String {
        match self {
            ProviderError::Network(_) => "network".into(),
            ProviderError::Http { status, .. } => format!("http_{status}"),
            ProviderError::Protocol(_) => "protocol".into(),
            ProviderError::Config(_) => "config".into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ToolError {
    /// Инвариант отказал. Не сбой: сработавшая защита (§5.9).
    #[error("{0}")]
    Rejected(String),

    /// Тул не смог сделать работу.
    ///
    /// `message` — форма неудачи для ленты (§5.5), одной строкой. `detail`
    /// появляется там, где модели нужно больше, чем человеку: вывод упавшей
    /// команды уже уехал в ленту по мере выполнения, дублировать его в
    /// подписи строки незачем, а починить без него модель не может.
    #[error("{message}")]
    Failed {
        message: String,
        detail: Option<String>,
    },

    #[error("отменено")]
    Cancelled,

    /// The human stopped this call themselves — the panel's "Stop", not a
    /// refused permission and not a failure.
    ///
    /// Kept apart from [`ToolError::Cancelled`], which means the whole turn was
    /// cancelled: this one arrives with the turn still running, and it is what
    /// ends it (§5.5).
    #[error("остановлено вами")]
    Interrupted,
}

impl ToolError {
    pub fn failed(msg: impl Into<String>) -> Self {
        ToolError::Failed {
            message: msg.into(),
            detail: None,
        }
    }

    /// Неудача, о которой модель должна знать подробнее, чем лента.
    pub fn failed_with(msg: impl Into<String>, detail: impl Into<String>) -> Self {
        ToolError::Failed {
            message: msg.into(),
            detail: Some(detail.into()),
        }
    }

    /// Что уедет модели как `tool_result`.
    pub fn for_model(&self) -> String {
        match self {
            ToolError::Failed {
                message,
                detail: Some(detail),
            } => format!("{message}\n{detail}"),
            other => other.to_string(),
        }
    }

    pub fn rejected(msg: impl Into<String>) -> Self {
        ToolError::Rejected(msg.into())
    }
}

impl From<std::io::Error> for ToolError {
    fn from(value: std::io::Error) -> Self {
        ToolError::failed(value.to_string())
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
    fn an_overflowing_prompt_is_recognised_in_the_words_providers_use() {
        for (status, message) in [
            (400, "prompt is too long: 213000 tokens > 200000 maximum"),
            (400, "input length and `max_tokens` exceed context limit: 180000 + 64000 > 200000"),
            (400, "This model's maximum context length is 128000 tokens. However, you requested 130000 tokens"),
            (400, r#"{"error":{"code":"context_length_exceeded"}}"#),
            (400, "the request exceeds the available context size, try increasing it"),
            (413, "Prompt Is Too Long"),
        ] {
            assert!(http(status, message).is_context_overflow(), "{status} {message}");
        }
    }

    #[test]
    fn other_refusals_are_not_taken_for_an_overflow() {
        assert!(!http(400, "messages: roles must alternate").is_context_overflow());
        assert!(!http(401, "invalid x-api-key").is_context_overflow());
        // The words alone are not enough: a server error mentioning the
        // context is not the context being full.
        assert!(!http(500, "context length handling crashed").is_context_overflow());
        assert!(!ProviderError::Network("prompt is too long".into()).is_context_overflow());
    }
}
