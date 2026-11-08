//! Asking the decision model, with a deadline. See [`super`].
//!
//! A decision model is not a chat model: it scores a statement instead of
//! writing an answer. A server that has the typed route is asked there, in its
//! own shape; only a server without it is asked in chat.
//!
//! A routing failure must never lose the user's message, so every way this can
//! go wrong (timeout, refusal, broken stream, unreadable reply) yields the same
//! answer: every signal unknown. The rules then decide what unknown means.

use std::time::Duration;

use futures_util::StreamExt;

use super::config::{Answers, Signal, SignalValue};
use super::question::{self, build_prompt, parse_answers};
use crate::error::ProviderError;
use crate::message::Message;
use crate::provider::systemone::SystemOne;
use crate::provider::{Provider, ProviderEvent, ProviderRequest};

/// The answer is a small JSON object: one number per signal, or one per option.
/// The cap keeps a rambling model from holding the deadline open for long.
const MAX_TOKENS: u32 = 256;

/// Ask every signal's question in one call and read the answers.
///
/// Returns one entry per signal, always. With no signals there is nothing to
/// ask, so neither client is called at all.
#[cfg(test)]
pub async fn ask(
    systemone: Option<&SystemOne>,
    provider: &dyn Provider,
    model: &str,
    signals: &[Signal],
    request_text: &str,
    timeout: Duration,
) -> Answers {
    ask_with_usage(systemone, provider, model, signals, request_text, timeout)
        .await
        .0
}

pub async fn ask_with_usage(
    systemone: Option<&SystemOne>,
    provider: &dyn Provider,
    model: &str,
    signals: &[Signal],
    request_text: &str,
    timeout: Duration,
) -> (Answers, Option<(crate::usage::TokenCounts, Option<f64>)>) {
    if signals.is_empty() {
        return (Answers::new(), None);
    }

    // A decision server is spoken to in the way it speaks. Asked in chat it
    // answers 405 where the route is missing and noise where it is not, and
    // either way every signal comes back unknown.
    if let Some(client) = systemone.filter(|client| !client.is_absent()) {
        let asked = tokio::time::timeout(
            timeout,
            client.ask(
                model,
                &question::cut_request(request_text),
                question::typed_questions(signals),
            ),
        )
        .await;
        match asked {
            Ok(Ok(Some(reply))) => {
                let usage = SystemOne::usage(&reply);
                return (question::read_typed(signals, &reply), Some((usage, None)));
            }
            // No such route: the server is a chat server. Ask it in chat.
            Ok(Ok(None)) => {}
            Ok(Err(err)) => {
                tracing::debug!(error = %err, "decision model failed; signals unknown");
                return (all_unknown(signals), None);
            }
            Err(_) => {
                tracing::debug!(?timeout, "decision model timed out; signals unknown");
                return (all_unknown(signals), None);
            }
        }
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
        Ok(Ok((reply, usage, cost))) => (parse_answers(signals, &reply), Some((usage, cost))),
        Ok(Err(err)) => {
            tracing::debug!(error = %err, "decision model failed; signals unknown");
            (all_unknown(signals), None)
        }
        Err(_) => {
            tracing::debug!(?timeout, "decision model timed out; signals unknown");
            (all_unknown(signals), None)
        }
    }
}

/// The text of the reply. Thinking and other blocks are not part of the answer.
async fn collect_text(
    provider: &dyn Provider,
    request: ProviderRequest,
) -> Result<(String, crate::usage::TokenCounts, Option<f64>), ProviderError> {
    let mut stream = provider.stream(request).await?;
    let mut text = String::new();
    let mut usage = crate::usage::TokenCounts::default();
    let mut cost = None;
    while let Some(event) = stream.next().await {
        match event? {
            ProviderEvent::TextDelta { text: delta, .. } => text.push_str(&delta),
            ProviderEvent::MessageStart { usage: counts, .. }
            | ProviderEvent::MessageDelta { usage: counts, .. } => usage.add(counts),
            ProviderEvent::Cost { usd } => cost = Some(usd),
            _ => {}
        }
    }
    Ok((text, usage, cost))
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
        let answers = ask(None, &mock, "small", &signals(), "write a parser", TIMEOUT).await;
        assert_eq!(answers, expected());
    }

    #[tokio::test]
    async fn fenced_reply_is_read() {
        let fenced = format!("```json\n{REPLY}\n```");
        let mock = MockProvider::new(vec![Turn::Events(script_text(&fenced))]);
        let answers = ask(None, &mock, "small", &signals(), "write a parser", TIMEOUT).await;
        assert_eq!(answers, expected());
    }

    #[tokio::test]
    async fn provider_refusal_gives_every_signal_unknown() {
        let mock = MockProvider::new(vec![Turn::Fail(ProviderError::Network(
            "connection reset".into(),
        ))]);
        let answers = ask(None, &mock, "small", &signals(), "write a parser", TIMEOUT).await;
        assert_eq!(answers, all_unknown(&signals()));
    }

    #[tokio::test]
    async fn slow_stream_past_the_deadline_gives_every_signal_unknown() {
        let mock = MockProvider::text(REPLY).with_gap(Duration::from_millis(300));
        let answers = ask(
            None,
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
        let answers = ask(None, &mock, "small", &signals(), "write a parser", TIMEOUT).await;
        assert_eq!(answers, all_unknown(&signals()));
    }

    #[tokio::test]
    async fn no_signals_means_no_request() {
        let mock = MockProvider::text(REPLY);
        let answers = ask(None, &mock, "small", &[], "write a parser", TIMEOUT).await;
        assert!(answers.is_empty());
        assert!(mock.seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn request_is_a_plain_single_message_with_the_text_inside() {
        let mock = MockProvider::text(REPLY);
        ask(None, &mock, "small-model", &signals(), "write a parser", TIMEOUT).await;

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

    // --- typed-decision route ---------------------------------------------------

    use crate::provider::registry::{Protocol, ProviderSpec};
    use crate::testutil::json_server;

    fn client(base_url: &str) -> SystemOne {
        SystemOne::new(
            ProviderSpec {
                id: "decision".into(),
                display_name: "Decision".into(),
                protocol: Protocol::OpenAi,
                base_url: base_url.into(),
                headers: Default::default(),
                models: Vec::new(),
            },
            None,
        )
    }

    const TYPED_REPLY: &str = r#"{"model":"m",
        "answers":{
          "is_code":{"type":"noul","noul":0.93},
          "language":{"type":"choice","choice":"rust",
            "probabilities":{"rust":0.88,"python":0.12}}},
        "usage":{"input_tokens":120,"output_tokens":0}}"#;

    #[tokio::test]
    async fn a_server_with_the_route_answers_in_its_own_shape() {
        let (url, seen) = json_server(200, TYPED_REPLY).await;
        let chat = MockProvider::text(REPLY);
        let client = client(&url);

        let (answers, usage) =
            ask_with_usage(Some(&client), &chat, "m", &signals(), "write a parser", TIMEOUT)
                .await;

        assert_eq!(
            answers,
            Answers::from([
                ("is_code".into(), SignalValue::Noul(0.93)),
                (
                    "language".into(),
                    SignalValue::Choice(BTreeMap::from([
                        ("rust".into(), 0.88),
                        ("python".into(), 0.12),
                    ])),
                ),
            ])
        );
        assert_eq!(usage.map(|(counts, cost)| (counts.input, cost)), Some((120, None)));
        // The model that scores was spoken to; the chat shape was not used.
        assert!(chat.seen.lock().unwrap().is_empty());

        let seen = seen.lock().unwrap();
        assert!(seen[0].starts_with("POST /v1/systemone"), "{}", seen[0]);
        // Every signal travels as a typed question, the choice with its options.
        let body: serde_json::Value =
            serde_json::from_str(seen[0].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["model"], "m");
        assert_eq!(body["state"], "write a parser");
        assert_eq!(body["questions"]["is_code"]["type"], "noul");
        assert_eq!(body["questions"]["language"]["type"], "choice");
        assert_eq!(body["questions"]["language"]["criteria"]["rust"], "Rust code");
    }

    #[tokio::test]
    async fn a_server_without_the_route_is_asked_in_chat() {
        let (url, seen) = json_server(405, r#"{"error":"method_not_allowed"}"#).await;
        let chat = MockProvider::text(REPLY);
        let client = client(&url);

        let answers = ask(Some(&client), &chat, "m", &signals(), "write a parser", TIMEOUT).await;

        assert_eq!(answers, expected());
        assert_eq!(chat.seen.lock().unwrap().len(), 1);
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_failing_route_gives_unknown_without_a_chat_retry() {
        let (url, _) = json_server(500, r#"{"error":"scoring failed"}"#).await;
        let chat = MockProvider::text(REPLY);
        let client = client(&url);

        let answers = ask(Some(&client), &chat, "m", &signals(), "write a parser", TIMEOUT).await;

        assert_eq!(answers, all_unknown(&signals()));
        assert!(chat.seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_typed_reply_missing_a_signal_leaves_it_unknown() {
        let (url, _) = json_server(200, r#"{"answers":{"is_code":{"type":"noul","noul":0.93}}}"#).await;
        let chat = MockProvider::text(REPLY);
        let client = client(&url);

        let answers = ask(Some(&client), &chat, "m", &signals(), "write a parser", TIMEOUT).await;

        assert_eq!(answers["is_code"], SignalValue::Noul(0.93));
        assert_eq!(answers["language"], SignalValue::Unknown);
        assert!(chat.seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_route_that_never_answers_is_cut_by_the_deadline() {
        // A socket that accepts and never replies: the request cannot finish,
        // so only the deadline can end it.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });
        let chat = MockProvider::text(REPLY);
        let client = client(&format!("http://{addr}"));

        let answers = ask(
            Some(&client),
            &chat,
            "m",
            &signals(),
            "write a parser",
            Duration::from_millis(50),
        )
        .await;

        assert_eq!(answers, all_unknown(&signals()));
        assert!(chat.seen.lock().unwrap().is_empty());
    }
}
