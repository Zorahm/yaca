//! OpenAI shape against a real socket.
//!
//! The same harness as for the Messages API, and the same reason: between building the body and
//! parsing the stream lies HTTP, and that is where everything lives that arbitrary providers
//! exist for — the address, the headers, the substituted key.

use std::collections::BTreeMap;
use std::sync::Arc;

use yaca_core::error::ProviderError;
use yaca_core::event::StopReason;
use yaca_core::intent::Effort;
use yaca_core::message::Message;
use yaca_core::provider::openai::OpenAiProvider;
use yaca_core::provider::registry::{Protocol, ProviderSpec};
use yaca_core::provider::{BlockKind, Provider, ProviderEvent, ProviderRequest};
use yaca_core::usage::TokenCounts;
use futures_util::StreamExt;

mod common;
use common::wire::{refusal, serve, serve_all};

fn spec(base: impl Into<String>) -> ProviderSpec {
    ProviderSpec {
        id: "gateway".into(),
        display_name: "Gateway".into(),
        protocol: Protocol::OpenAi,
        base_url: base.into(),
        headers: BTreeMap::new(),
        models: vec![],
    }
}

fn request() -> ProviderRequest {
    ProviderRequest {
        model: "gpt-oss-120b".into(),
        system: "you are Yaca".into(),
        messages: vec![Message::user_text("hello")],
        tools: vec![],
        max_tokens: 8_000,
        effort: Some(Effort::Max),
        thinking: true,
    }
}

const SSE_OK: &str = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Type: text/event-stream\r\n",
    "Connection: close\r\n",
    "\r\n",
    // The first chunk declares the role and carries no text.
    r#"data: {"id":"1","model":"gpt-oss-120b","choices":[{"index":0,"delta":{"role":"assistant","content":""}}]}"#,
    "\n\n",
    r#"data: {"choices":[{"delta":{"reasoning_content":"need to read the file"}}]}"#,
    "\n\n",
    ": keep-alive\n\n",
    r#"data: {"choices":[{"delta":{"content":"Reading "}}]}"#,
    "\n\n",
    r#"data: {"choices":[{"delta":{"content":"file."}}]}"#,
    "\n\n",
    r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"Read","arguments":"{\"path\":"}}]}}]}"#,
    "\n\n",
    r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"src/a.rs\"}"}}]}}]}"#,
    "\n\n",
    r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
    "\n\n",
    r#"data: {"choices":[],"usage":{"prompt_tokens":120,"completion_tokens":9,"prompt_tokens_details":{"cached_tokens":20}}}"#,
    "\n\n",
    "data: [DONE]\n\n",
);

#[tokio::test]
async fn stream_passes_through_a_real_socket() {
    let (base, seen) = serve(SSE_OK).await;
    let provider = Arc::new(OpenAiProvider::new(spec(base), Some("sk-test".into())));

    let mut stream = provider.stream(request()).await.unwrap();
    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        events.push(item.unwrap());
    }

    assert_eq!(
        events,
        vec![
            ProviderEvent::MessageStart {
                model: "gpt-oss-120b".into(),
                usage: TokenCounts::default(),
            },
            ProviderEvent::BlockStart {
                index: 0,
                kind: BlockKind::Thinking,
            },
            ProviderEvent::ThinkingDelta {
                index: 0,
                text: "need to read the file".into(),
            },
            ProviderEvent::BlockStop { index: 0 },
            ProviderEvent::BlockStart {
                index: 1,
                kind: BlockKind::Text,
            },
            ProviderEvent::TextDelta {
                index: 1,
                text: "Reading ".into(),
            },
            ProviderEvent::TextDelta {
                index: 1,
                text: "file.".into(),
            },
            ProviderEvent::BlockStop { index: 1 },
            ProviderEvent::BlockStart {
                index: 2,
                kind: BlockKind::ToolUse {
                    id: "call_1".into(),
                    name: "Read".into(),
                },
            },
            ProviderEvent::ToolInputDelta {
                index: 2,
                partial_json: "{\"path\":".into(),
            },
            ProviderEvent::ToolInputDelta {
                index: 2,
                partial_json: "\"src/a.rs\"}".into(),
            },
            ProviderEvent::BlockStop { index: 2 },
            ProviderEvent::MessageDelta {
                stop_reason: Some(StopReason::ToolUse),
                usage: TokenCounts {
                    input: 100,
                    output: 9,
                    cache_write: 0,
                    cache_read: 20,
                },
            },
            ProviderEvent::MessageStop,
        ]
    );

    let seen = seen.await.unwrap();
    assert!(seen.has_header("authorization", "Bearer sk-test"));
    assert!(seen.has_header("content-type", "application/json"));
    assert_eq!(seen.body["stream_options"]["include_usage"], true);
    assert_eq!(seen.body["messages"][0]["role"], "system");
    assert_eq!(seen.body["reasoning_effort"], "high");
}

#[tokio::test]
async fn own_header_travels_with_the_substituted_key() {
    let (base, seen) = serve(SSE_OK).await;
    let mut s = spec(base);
    s.headers.insert("x-title".into(), "Yaca".into());
    s.headers.insert("x-token".into(), "prefix-{key}".into());
    let provider = OpenAiProvider::new(s, Some("sk-live".into()));

    let mut stream = provider.stream(request()).await.unwrap();
    while stream.next().await.is_some() {}

    let seen = seen.await.unwrap();
    assert!(seen.has_header("x-title", "Yaca"));
    assert!(seen.has_header("x-token", "prefix-sk-live"));
}

#[tokio::test]
async fn a_gateway_without_authorization_needs_no_key() {
    let (base, seen) = serve(SSE_OK).await;
    let mut s = spec(base);
    // A local Ollama: the authorization header is removed altogether.
    s.headers.insert("authorization".into(), String::new());
    assert!(!s.needs_key());

    let provider = OpenAiProvider::new(s, None);
    let mut stream = provider.stream(request()).await.unwrap();
    while stream.next().await.is_some() {}

    assert!(seen.await.is_ok());
}

/// Unsloth Studio, LM Studio: the protocol's `authorization: Bearer {key}` is
/// still in the description, no key was ever stored, and the server never
/// asked for one. Nothing stands between them (§10).
#[tokio::test]
async fn a_local_server_is_reached_without_a_key() {
    let (base, seen) = serve(SSE_OK).await;
    let provider = OpenAiProvider::new(spec(base), None);

    let mut stream = provider.stream(request()).await.unwrap();
    while stream.next().await.is_some() {}

    let seen = seen.await.unwrap();
    assert!(
        !seen.headers.to_ascii_lowercase().contains("authorization"),
        "{}",
        seen.headers
    );
}

#[tokio::test]
async fn rate_limit_carries_retry_after() {
    let body = r#"{"error":{"message":"rate limit","type":"rate_limit_error"}}"#;
    // The length is counted, not typed in by hand: a `Content-Length` that has drifted
    // would check the arithmetic in the test, not the provider.
    let too_many = format!(
        "HTTP/1.1 429 Too Many Requests\r\n\
         Content-Type: application/json\r\n\
         Retry-After: 30\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n{body}",
        body.len()
    );
    let (base, _seen) = serve(too_many).await;
    let provider = OpenAiProvider::new(spec(base), Some("sk-test".into()));

    let err = refusal(provider.stream(request()).await);
    match err {
        ProviderError::Http {
            status,
            message,
            retry_after_s,
        } => {
            assert_eq!(status, 429);
            assert_eq!(message, "rate limit");
            assert_eq!(retry_after_s, Some(30));
        }
        other => panic!("wrong error: {other:?}"),
    }
}

/// The refusal from new OpenAI models: code 400 and the field name right in the text.
const REFUSES_MAX_TOKENS: &str = concat!(
    "HTTP/1.1 400 Bad Request\r\n",
    "Content-Type: application/json\r\n",
    "Connection: close\r\n",
    "\r\n",
    r#"{"error":{"message":"Unsupported parameter: 'max_tokens' is not supported with this model. Use 'max_completion_tokens' instead.","type":"invalid_request_error"}}"#,
);

const REFUSES_FOR_ANOTHER_REASON: &str = concat!(
    "HTTP/1.1 400 Bad Request\r\n",
    "Content-Type: application/json\r\n",
    "Connection: close\r\n",
    "\r\n",
    r#"{"error":{"message":"model `gpt-nope` does not exist"}}"#,
);

#[tokio::test]
async fn the_provider_asks_again_with_the_field_it_named() {
    let (base, seen) = serve_all(vec![REFUSES_MAX_TOKENS.into(), SSE_OK.into()]).await;
    let provider = Arc::new(OpenAiProvider::new(spec(base), Some("sk-test".into())));

    // The turn does not fail: the refusal was about a field name, and the provider was asked for the name.
    let mut stream = provider.stream(request()).await.unwrap();
    while let Some(item) = stream.next().await {
        item.unwrap();
    }

    let seen = seen.await.unwrap();
    assert_eq!(seen.len(), 2, "asked again exactly once");

    // First the way it is accepted almost everywhere.
    assert_eq!(seen[0].body["max_tokens"], 8_000);
    assert!(seen[0].body.get("max_completion_tokens").is_none());

    // Then with the one the provider named — and again with a single field.
    assert_eq!(seen[1].body["max_completion_tokens"], 8_000);
    assert!(
        seen[1].body.get("max_tokens").is_none(),
        "both fields at once is the same 400"
    );
}

#[tokio::test]
async fn a_learned_field_holds_to_the_end_of_the_session() {
    let (base, seen) = serve_all(vec![
        REFUSES_MAX_TOKENS.into(),
        SSE_OK.into(),
        SSE_OK.into(),
    ])
    .await;
    let provider = Arc::new(OpenAiProvider::new(spec(base), Some("sk-test".into())));

    for _ in 0..2 {
        let mut stream = provider.stream(request()).await.unwrap();
        while let Some(item) = stream.next().await {
            item.unwrap();
        }
    }

    let seen = seen.await.unwrap();
    // Three requests, not four: the second turn already knows what to ask with.
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[2].body["max_completion_tokens"], 8_000);
}

#[tokio::test]
async fn another_refusal_is_not_repeated() {
    let (base, seen) = serve_all(vec![REFUSES_FOR_ANOTHER_REASON.into()]).await;
    let provider = Arc::new(OpenAiProvider::new(spec(base), Some("sk-test".into())));

    let err = refusal(provider.stream(request()).await);
    assert!(
        matches!(err, ProviderError::Http { status: 400, .. }),
        "{err:?}"
    );
    assert_eq!(seen.await.unwrap().len(), 1, "there was nothing to ask again with");
}
