//! Provider against a real socket.
//!
//! Parsing SSE and building the body are checked by unit tests, but HTTP lies
//! between them: headers, status, a connection that drops. Here a small server
//! is started and says exactly what the Messages API says.
//!
//! There is no live call to api.anthropic.com in the suite on purpose: a test
//! that needs a key and money stops being run in its second week.

use std::sync::Arc;
use std::time::Duration;

use yaca_core::error::ProviderError;
use yaca_core::event::StopReason;
use yaca_core::intent::Effort;
use yaca_core::message::Message;
use yaca_core::provider::registry::ProviderSpec;
use yaca_core::provider::{
    anthropic::AnthropicProvider, BlockKind, Provider, ProviderEvent, ProviderRequest,
};
use futures_util::StreamExt;

mod common;
use common::wire::{json_response, refusal, serve};

/// Built-in description, redirected to the started socket.
fn spec(base: impl Into<String>) -> ProviderSpec {
    ProviderSpec {
        base_url: base.into(),
        ..ProviderSpec::anthropic()
    }
}

fn request() -> ProviderRequest {
    ProviderRequest {
        model: "claude-opus-5".into(),
        system: "ты Yaca".into(),
        messages: vec![Message::user_text("привет")],
        tools: vec![],
        max_tokens: 64_000,
        effort: Some(Effort::Extra),
        thinking: true,
    }
}

const SSE_OK: &str = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Type: text/event-stream\r\n",
    "Connection: close\r\n",
    "\r\n",
    "event: message_start\n",
    r#"data: {"type":"message_start","message":{"model":"claude-opus-5","usage":{"input_tokens":12,"output_tokens":0,"cache_read_input_tokens":900}}}"#,
    "\n\n",
    "event: content_block_start\n",
    r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
    "\n\n",
    ": keep-alive\n\n",
    "event: content_block_delta\n",
    r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Ядро "}}"#,
    "\n\n",
    "event: content_block_delta\n",
    r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"на Rust."}}"#,
    "\n\n",
    "event: content_block_stop\n",
    r#"data: {"type":"content_block_stop","index":0}"#,
    "\n\n",
    "event: message_delta\n",
    r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":7}}"#,
    "\n\n",
    "event: message_stop\n",
    r#"data: {"type":"message_stop"}"#,
    "\n\n",
);

#[tokio::test]
async fn поток_проходит_по_настоящему_сокету() {
    let (base, seen) = serve(SSE_OK).await;
    let provider = Arc::new(AnthropicProvider::new(spec(base), Some("sk-test".into())));

    let mut stream = provider.stream(request()).await.unwrap();
    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        events.push(item.unwrap());
    }

    assert!(matches!(
        events.first(),
        Some(ProviderEvent::MessageStart { .. })
    ));
    assert!(events.contains(&ProviderEvent::BlockStart {
        index: 0,
        kind: BlockKind::Text
    }));

    let текст: String = events
        .iter()
        .filter_map(|e| match e {
            ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(текст, "Ядро на Rust.");

    assert!(events.iter().any(|e| matches!(
        e,
        ProviderEvent::MessageDelta {
            stop_reason: Some(StopReason::EndTurn),
            ..
        }
    )));
    assert_eq!(events.last(), Some(&ProviderEvent::MessageStop));

    // An SSE comment (`: keep-alive`) must not turn into an event.
    assert_eq!(events.len(), 7, "лишние события: {events:#?}");

    let seen = seen.await.unwrap();
    let h = seen.headers.to_ascii_lowercase();
    assert!(h.contains("x-api-key: sk-test"));
    assert!(h.contains("anthropic-version: 2023-06-01"));

    let body = seen.body;
    assert_eq!(body["stream"], true);
    assert_eq!(body["model"], "claude-opus-5");
    // The shape §5.3 is responsible for, and the current state of the Messages API.
    assert_eq!(body["thinking"]["type"], "adaptive");
    assert_eq!(body["thinking"]["display"], "summarized");
    assert_eq!(body["output_config"]["effort"], "xhigh");
    assert!(body["thinking"].get("budget_tokens").is_none());
    assert!(body.get("temperature").is_none());
    assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(
        body["messages"][0]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
}

#[tokio::test]
async fn лимит_запросов_доносит_retry_after() {
    let body = r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#;
    // The length is counted, not typed in by hand. A hand-typed length that has
    // drifted from the body gives exactly the picture the test is written for: an
    // empty error message instead of the cause.
    let too_many = format!(
        "HTTP/1.1 429 Too Many Requests\r\n\
         Content-Type: application/json\r\n\
         Retry-After: 3\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n{body}",
        body.len()
    );

    let (base, _seen) = serve(too_many).await;
    let provider = AnthropicProvider::new(spec(base), Some("sk-test".into()));

    let err = refusal(provider.stream(request()).await);
    match err {
        ProviderError::Http {
            status,
            message,
            retry_after_s,
        } => {
            assert_eq!(status, 429);
            assert_eq!(message, "slow down");
            // Without this the core would wait its half a second and get 429 again.
            assert_eq!(retry_after_s, Some(3));
        }
        other => panic!("не тот отказ: {other:?}"),
    }
    assert!(ProviderError::Http {
        status: 429,
        message: String::new(),
        retry_after_s: None
    }
    .is_retryable());
}

/// No key is not a refusal of its own (§10): the request goes out without the
/// header, and a server that wanted one says so. The refusal names the missing
/// key, so a bare 401 does not have to be puzzled out.
#[tokio::test]
async fn without_a_key_the_request_goes_out_and_the_refusal_names_the_key() {
    let body = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
    let (base, seen) = serve(json_response("401 Unauthorized", body)).await;
    let provider = AnthropicProvider::new(spec(base), Some("   ".into()));

    let err = refusal(
        tokio::time::timeout(Duration::from_secs(5), provider.stream(request()))
            .await
            .expect("hung on the network"),
    );

    let seen = seen.await.unwrap();
    assert!(
        !seen.headers.to_ascii_lowercase().contains("x-api-key"),
        "{}",
        seen.headers
    );
    match err {
        ProviderError::Http {
            status: 401,
            message,
            ..
        } => {
            assert!(message.contains("invalid x-api-key"), "{message}");
            assert!(message.contains("«anthropic»"), "{message}");
        }
        other => panic!("wrong refusal: {other:?}"),
    }
}
