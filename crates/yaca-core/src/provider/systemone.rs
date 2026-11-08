//! Client of the typed-decision API: `POST {base_url}/v1/systemone`.
//!
//! Decision models such as Jev-Style are not chat models. They are scored, not
//! sampled: one pass gives a probability for a yes/no question, and a chat
//! request to them returns noise. Ollama, LM Studio, llama.cpp builds with the
//! route and `jev-style serve` all take the same shape:
//!
//! ```json
//! {"model": "m", "state": "…", "questions": {"id": {"type": "noul", "instructions": "…"}}}
//! ```
//!
//! and answer `{"answers": {"id": {"type": "noul", "noul": 0.97}}}`, where
//! `noul` is the probability that the statement is true.
//!
//! Auto's reviewer reads a yes/no probability; the Decision tool also asks
//! multiple-choice questions through the same authenticated transport.

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value};

use super::http;
use super::registry::ProviderSpec;
use crate::error::ProviderError;
use crate::usage::TokenCounts;

/// The id the one question goes under. The answer is read back by it.
const QUESTION: &str = "q";

pub struct SystemOne {
    http: reqwest::Client,
    spec: ProviderSpec,
    key: Option<String>,
    /// The server answered "no such route". Remembered for the session: a chat
    /// server does not grow the route between two commands, and asking each time
    /// would double every request with one that cannot succeed.
    absent: AtomicBool,
}

/// What came of one question.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The probability that the statement is true, from 0 to 1.
    Probability(f64),
    /// The server has no `/v1/systemone`. The caller may ask another way.
    Absent,
    /// The server has the route and the answer did not come: a refusal, a
    /// broken connection, a reply that is not an answer. Not a reason to try
    /// another way — the server was asked in the way it speaks.
    Failed(String),
}

impl SystemOne {
    /// The key is optional, as for the chat providers: a local server needs none.
    pub fn new(spec: ProviderSpec, key: Option<String>) -> Self {
        SystemOne {
            http: reqwest::Client::new(),
            spec,
            key,
            absent: AtomicBool::new(false),
        }
    }

    /// The tokens a reply says it spent. A server that reports none counts as
    /// zero: usage is a price note, not part of the answer.
    pub fn usage(reply: &Value) -> TokenCounts {
        let count = |key: &str| {
            reply["usage"][key]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0)
        };
        TokenCounts {
            input: count("input_tokens"),
            output: count("output_tokens"),
            ..TokenCounts::default()
        }
    }

    /// Whether the server has said it does not speak this.
    pub fn is_absent(&self) -> bool {
        self.absent.load(Ordering::Relaxed)
    }

    /// How likely is it that `statement` holds of `state`?
    pub async fn noul(&self, model: &str, state: &str, statement: &str) -> Outcome {
        let questions = json!({ QUESTION: { "type": "noul", "instructions": statement } });
        match self.ask(model, state, questions).await {
            Ok(Some(value)) => read_probability(&value),
            Ok(None) => Outcome::Absent,
            Err(e) => Outcome::Failed(e.to_string()),
        }
    }

    /// Ask typed questions. `None` means the server has no decision route;
    /// only that case permits the caller to fall back to chat.
    pub async fn ask(
        &self,
        model: &str,
        state: &str,
        questions: Value,
    ) -> Result<Option<Value>, ProviderError> {
        if self.is_absent() {
            return Ok(None);
        }
        let mut headers = http::header_map(&self.spec, self.key.as_deref())?;
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        let body = json!({
            "model": model,
            "state": state,
            "questions": questions,
        });
        let resp = self
            .http
            .post(self.spec.api("/systemone"))
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(http::network_error)?;
        let resp = match http::check_status(resp, &self.spec, self.key.as_deref()).await {
            Ok(resp) => resp,
            // A server without the route says so with one of these. A 400 is
            // not among them: Ollama answers 400 for a model that is not a
            // decision model, and that is the server speaking the protocol.
            Err(ProviderError::Http {
                status: 404 | 405 | 501,
                ..
            }) => {
                self.absent.store(true, Ordering::Relaxed);
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        resp.json::<Value>()
            .await
            .map(Some)
            .map_err(|e| ProviderError::Protocol(e.without_url().to_string()))
    }
}

/// `answers.q.noul`, a number in `0..=1`. Anything else is not an answer.
fn read_probability(value: &Value) -> Outcome {
    match value["answers"][QUESTION]["noul"].as_f64() {
        Some(p) if (0.0..=1.0).contains(&p) => Outcome::Probability(p),
        _ => Outcome::Failed("the reply has no probability for the question".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::registry::Protocol;

    fn spec(base_url: &str) -> ProviderSpec {
        ProviderSpec {
            id: "decision".into(),
            display_name: "Decision".into(),
            protocol: Protocol::OpenAi,
            base_url: base_url.into(),
            headers: Default::default(),
            models: Vec::new(),
        }
    }

    #[test]
    fn the_probability_is_read_by_the_question_id() {
        let reply = json!({"model": "m", "answers": {"q": {"type": "noul", "noul": 0.97}}});
        assert_eq!(read_probability(&reply), Outcome::Probability(0.97));
    }

    #[test]
    fn a_reply_without_a_usable_number_is_a_failure() {
        for reply in [
            json!({}),
            json!({"answers": {}}),
            json!({"answers": {"q": {"noul": "high"}}}),
            json!({"answers": {"q": {"noul": 1.5}}}),
            json!({"answers": {"other": {"noul": 0.9}}}),
        ] {
            assert!(
                matches!(read_probability(&reply), Outcome::Failed(_)),
                "{reply}"
            );
        }
    }

    #[test]
    fn the_route_lives_under_v1_of_the_providers_address() {
        assert_eq!(
            spec("http://localhost:11434").api("/systemone"),
            "http://localhost:11434/v1/systemone"
        );
        // A base URL pasted with its /v1 on is forgiven, as for chat.
        assert_eq!(
            spec("http://localhost:1234/v1").api("/systemone"),
            "http://localhost:1234/v1/systemone"
        );
    }
}
