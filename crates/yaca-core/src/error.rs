//! Core errors.
//!
//! The split matters and is not cosmetic: [`ToolError::Rejected`] is a tripped
//! invariant (§5.9); the interface paints it `--vermilion-ink` and there is
//! nothing for the human to do. [`ToolError::Failed`] is a real failure, `--rust`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("provider: {0}")]
    Provider(#[from] ProviderError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization: {0}")]
    Json(#[from] serde_json::Error),

    #[error("session cancelled")]
    Cancelled,

    #[error("{0}")]
    Config(String),
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("network: {0}")]
    Network(String),

    #[error("HTTP {status}: {message}")]
    Http {
        status: u16,
        message: String,
    /// The `retry-after` header in seconds, if the provider sent one.
        retry_after_s: Option<u64>,
    },

    #[error("protocol parsing failed: {0}")]
    Protocol(String),

    /// The provider is described in a way that makes the request impossible to
    /// build: a bad header name, a bad value. Retrying will not help — the
    /// description has to be fixed.
    #[error("provider config: {0}")]
    Config(String),
}

impl ProviderError {
    /// Whether to retry. Drives `RetryScheduled` (§5.10).
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
    /// An invariant refused. Not a failure: a safeguard that tripped (§5.9).
    #[error("{0}")]
    Rejected(String),

    /// The tool could not do the work.
    ///
    /// `message` is the shape of the failure for the feed (§5.5), one line. `detail`
    /// appears where the model needs more than the human does: the output of a failed
    /// command has already gone to the feed as it ran, so repeating it in the line's
    /// caption is pointless, yet the model cannot fix anything without it.
    #[error("{message}")]
    Failed {
        message: String,
        detail: Option<String>,
    },

    #[error("cancelled")]
    Cancelled,

    /// The human stopped this call themselves — the panel's "Stop", not a
    /// refused permission and not a failure.
    ///
    /// Kept apart from [`ToolError::Cancelled`], which means the whole turn was
    /// cancelled: this one arrives with the turn still running, and it is what
    /// ends it (§5.5).
    #[error("interrupted by user")]
    Interrupted,
}

impl ToolError {
    pub fn failed(msg: impl Into<String>) -> Self {
        ToolError::Failed {
            message: msg.into(),
            detail: None,
        }
    }

    /// A failure the model must know more about than the feed shows.
    pub fn failed_with(msg: impl Into<String>, detail: impl Into<String>) -> Self {
        ToolError::Failed {
            message: msg.into(),
            detail: Some(detail.into()),
        }
    }

    /// What goes to the model as `tool_result`.
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
