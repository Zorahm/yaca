//! Asking the decision model, with a deadline. See [`super`].
//!
//! A routing failure must never lose the user's message, so every way this can
//! go wrong (timeout, refusal, broken stream, unreadable reply) yields the same
//! answer: every signal unknown. The rules then decide what unknown means.

use std::time::Duration;

use futures_util::StreamExt;

use super::config::{Answers, Signal, SignalValue};
use super::question::{build_prompt, parse_answers};
use crate::error::ProviderError;
use crate::message::Message;
use crate::provider::{Provider, ProviderEvent, ProviderRequest};

/// The answer is a small JSON object: one number per signal, or one per option.
/// The cap keeps a rambling model from holding the deadline open for long.
const MAX_TOKENS: u32 = 256;

/// Ask every signal's question in one call and read the answers.
///
/// Returns one entry per signal, always. With no signals there is nothing to
/// ask, so the provider is not called at all.
pub async fn ask(
    provider: &dyn Provider,
    model: &str,
    signals: &[Signal],
    request_text: &str,
    timeout: Duration,
) -> Answers {
    if signals.is_empty() {
        return Answers::new();
    }
    let prompt = build_prompt(signals, request_text);
    let request = ProviderRequest {
        model: model.to_string(),
        system: prompt.system,
        messages: vec![Message::user_text(prompt.user)],
        // The classifier answers from the text alone: no tools to call, and
        // no reasoning to wait for within the deadline.
        tools: vec![],
        max_tokens: MAX_TOKENS,
        effort: None,
        thinking: false,
    };
    match tokio::time::timeout(timeout, collect_text(provider, request)).await {
        Ok(Ok(reply)) => parse_answers(signals, &reply),
        Ok(Err(err)) => {
            tracing::debug!(error = %err, "decision model failed; signals unknown");
            all_unknown(signals)
        }
        Err(_) => {
            tracing::debug!(?timeout, "decision model timed out; signals unknown");
            all_unknown(signals)
        }
    }
}

/// The text of the reply. Thinking and other blocks are not part of the answer.
async fn collect_text(
    provider: &dyn Provider,
    request: ProviderRequest,
) -> Result<String, ProviderError> {
    let mut stream = provider.stream(request).await?;
    let mut text = String::new();
    while let Some(event) = stream.next().await {
        if let ProviderEvent::TextDelta { text: delta, .. } = event? {
            text.push_str(&delta);
        }
    }
    Ok(text)
}

fn all_unknown(signals: &[Signal]) -> Answers {
    signals
        .iter()
        .map(|signal| (signal.name.clone(), SignalValue::Unknown))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::message::{ContentBlock, Role};
    use crate::provider::mock::{script_text, MockProvider, Turn};
    use crate::router::config::{Choice, Question};

    const TIMEOUT: Duration = Duration::from_secs(5);
    const REPLY: &str = r#"{"is_code": 0.5, "language": {"rust": 0.75, "python": 0.25}}"#;

    fn signals() -> Vec<Signal> {
        vec![
            Signal {
                name: "is_code".into(),
                question: Question::Noul {
                    instructions: "Is the request about code?".into(),
                },
            },
            Signal {
                name: "language".into(),
                question: Question::Choice {
                    instructions: "Which language?".into(),
                    choices: vec![
                        Choice {
                            key: "rust".into(),
                            description: "Rust code".into(),
                        },
                        Choice {
                            key: "python".into(),
                            description: "Python code".into(),
                        },
                    ],
                },
            },
        ]
    }

    fn expected() -> Answers {
        Answers::from([
            ("is_code".to_string(), SignalValue::Noul(0.5)),
            (
                "language".to_string(),
                SignalValue::Choice(BTreeMap::from([
                    ("rust".to_string(), 0.75),
                    ("python".to_string(), 0.25),
                ])),
            ),
        ])
    }

    #[tokio::test]
    async fn answers_come_back_from_the_model() {
        let mock = MockProvider::text(REPLY);
        let answers = ask(&mock, "small", &signals(), "write a parser", TIMEOUT).await;
        assert_eq!(answers, expected());
    }

    #[tokio::test]
    async fn fenced_reply_is_read() {
        let fenced = format!("```json\n{REPLY}\n```");
        let mock = MockProvider::new(vec![Turn::Events(script_text(&fenced))]);
        let answers = ask(&mock, "small", &signals(), "write a parser", TIMEOUT).await;
        assert_eq!(answers, expected());
    }

    #[tokio::test]
    async fn provider_refusal_gives_every_signal_unknown() {
        let mock = MockProvider::new(vec![Turn::Fail(ProviderError::Network(
            "connection reset".into(),
        ))]);
        let answers = ask(&mock, "small", &signals(), "write a parser", TIMEOUT).await;
        assert_eq!(answers, all_unknown(&signals()));
    }

    #[tokio::test]
    async fn slow_stream_past_the_deadline_gives_every_signal_unknown() {
        let mock = MockProvider::text(REPLY).with_gap(Duration::from_millis(300));
        let answers = ask(
            &mock,
            "small",
            &signals(),
            "write a parser",
            Duration::from_millis(30),
        )
        .await;
        assert_eq!(answers, all_unknown(&signals()));
    }

    #[tokio::test]
    async fn garbage_reply_gives_every_signal_unknown() {
        let mock = MockProvider::text("I would rather not say.");
        let answers = ask(&mock, "small", &signals(), "write a parser", TIMEOUT).await;
        assert_eq!(answers, all_unknown(&signals()));
    }

    #[tokio::test]
    async fn no_signals_means_no_request() {
        let mock = MockProvider::text(REPLY);
        let answers = ask(&mock, "small", &[], "write a parser", TIMEOUT).await;
        assert!(answers.is_empty());
        assert!(mock.seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn request_is_a_plain_single_message_with_the_text_inside() {
        let mock = MockProvider::text(REPLY);
        ask(&mock, "small-model", &signals(), "write a parser", TIMEOUT).await;

        let seen = mock.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        let req = &seen[0];
        assert_eq!(req.model, "small-model");
        assert_eq!(
            req.system,
            build_prompt(&signals(), "write a parser").system
        );
        assert!(req.tools.is_empty());
        assert!(!req.thinking);
        assert_eq!(req.effort, None);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, Role::User);
        assert!(matches!(
            &req.messages[0].content[..],
            [ContentBlock::Text { text }] if text.contains("write a parser")
        ));
    }
}
