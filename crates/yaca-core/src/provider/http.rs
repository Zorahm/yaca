//! What the two protocols share: headers, parsing of an unsuccessful response and
//! pumping SSE into a channel.
//!
//! A separate file, because they differ only in the body's shape and in parsing
//! events. A second copy of `bytes_stream` would be two places to fix
//! a dropped connection.

use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tokio_stream::wrappers::ReceiverStream;

use super::registry::ProviderSpec;
use super::sse::SseDecoder;
use super::{ProviderEvent, ProviderStream};
use crate::error::ProviderError;

/// Provider headers in reqwest's form.
///
/// Built through `HeaderMap`, not a chain of `.header()`: a provider description may
/// override the protocol's defaults, and `.header()` adds a second value
/// instead of replacing it — the request would go out with two `authorization`s.
pub(super) fn header_map(
    spec: &ProviderSpec,
    key: Option<&str>,
) -> Result<HeaderMap, ProviderError> {
    let mut map = HeaderMap::new();
    for (name, value) in spec.headers_for(key) {
        let name = HeaderName::try_from(name.as_str())
            .map_err(|e| ProviderError::Config(format!("header «{name}»: {e}")))?;
        let value = HeaderValue::try_from(value.as_str())
            .map_err(|e| ProviderError::Config(format!("header value «{name}»: {e}")))?;
        map.insert(name, value);
    }
    Ok(map)
}

/// Checks the status and turns a failure into an error with the body inside.
///
/// `spec` and `key` are what the request went out with: a refusal of a
/// request sent without the key its description asks for says so.
pub(super) async fn check_status(
    resp: reqwest::Response,
    spec: &ProviderSpec,
    key: Option<&str>,
) -> Result<reqwest::Response, ProviderError> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }

    let retry_after_s = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    // Not `unwrap_or_default`: a silently swallowed cause turns into
    // an empty error block that gives the human nothing to decide on.
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => format!("the response body could not be read: {e}"),
    };
    Err(ProviderError::Http {
        status: status.as_u16(),
        message: name_missing_key(extract_api_message(&text), status.as_u16(), spec, key),
        retry_after_s,
    })
}

/// A 401 or 403 to a request that went without its key names the key.
///
/// No key is not a refusal of its own (see `registry`): the request goes, and
/// the server decides. When it refuses, its own words — `invalid x-api-key`,
/// `Incorrect API key provided` — do not say that no key was sent at all, and
/// the fix is in Yaca's settings rather than at the provider.
fn name_missing_key(
    message: String,
    status: u16,
    spec: &ProviderSpec,
    key: Option<&str>,
) -> String {
    let keyless = key.is_none_or(|k| k.trim().is_empty());
    if keyless && spec.needs_key() && (status == 401 || status == 403) {
        format!("{message} — API key for «{}» is not set", spec.id)
    } else {
        message
    }
}

/// Error text from the body of an unsuccessful response.
///
/// The `{"error":{"message":…}}` shape is the same for both protocols — it is
/// the only thing they agreed on without any coordination.
pub(super) fn extract_api_message(body: &str) -> String {
    if let Some(message) = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(str::to_owned)
        })
    {
        return message;
    }
    let raw: String = body.trim().chars().take(400).collect();
    if raw.is_empty() {
        // A cut-off response does happen. An empty line in the error block (§5.10)
        // does not happen: the human would have nothing to read.
        "the provider sent no details".into()
    } else {
        raw
    }
}

/// Parsing of one form of stream.
///
/// A trait, not a function: the OpenAI shape gives neither block indexes nor an
/// explicit end of the message, so parsing needs memory between payloads and a
/// tail that is appended once the stream has ended.
pub(super) trait StreamDecoder: Send + 'static {
    fn payload(&mut self, payload: &str) -> Result<Vec<ProviderEvent>, ProviderError>;

    /// The bytes have run out. A shape that closes the message itself adds nothing.
    fn finish(&mut self) -> Vec<ProviderEvent> {
        Vec::new()
    }
}

/// Pumps SSE into a channel, parsing each payload.
///
/// A channel instead of a hand-written `Stream`: parsing SSE is state on top of
/// state, and writing `poll_next` just to save one task is not worth it.
pub(super) fn pump(resp: reqwest::Response, mut decode: impl StreamDecoder) -> ProviderStream {
    let (tx, rx) = tokio::sync::mpsc::channel(256);
    tokio::spawn(async move {
        let mut decoder = SseDecoder::new();
        let mut bytes = resp.bytes_stream();

        while let Some(chunk) = bytes.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(ProviderError::Network(e.to_string()))).await;
                    return;
                }
            };
            for payload in decoder.push(&chunk) {
                match decode.payload(&payload) {
                    Ok(events) => {
                        for ev in events {
                            if tx.send(Ok(ev)).await.is_err() {
                                return; // the reader has gone — the turn is cancelled
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                }
            }
        }

        for ev in decode.finish() {
            if tx.send(Ok(ev)).await.is_err() {
                return;
            }
        }
    });

    Box::pin(ReceiverStream::new(rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::registry::{Protocol, ProviderSpec};
    use std::collections::BTreeMap;

    fn spec(protocol: Protocol) -> ProviderSpec {
        ProviderSpec {
            id: "gateway".into(),
            display_name: "Gateway".into(),
            protocol,
            base_url: "https://gate.example.com".into(),
            headers: BTreeMap::new(),
            models: vec![],
        }
    }

    #[test]
    fn extracts_the_error_text_from_the_body() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"no"}}"#;
        assert_eq!(extract_api_message(body), "no");
    }

    #[test]
    fn an_empty_body_does_not_give_an_empty_message() {
        assert!(!extract_api_message("").is_empty());
        assert_eq!(extract_api_message("Bad Gateway"), "Bad Gateway");
    }

    #[test]
    fn an_overridden_header_is_sent_once() {
        let mut s = spec(Protocol::OpenAi);
        s.headers
            .insert("authorization".into(), "Basic svoj".into());
        let map = header_map(&s, None).unwrap();
        assert_eq!(map.get_all("authorization").iter().count(), 1);
        assert_eq!(map["authorization"], "Basic svoj");
    }

    #[test]
    fn an_invalid_header_name_is_a_config_error() {
        let mut s = spec(Protocol::OpenAi);
        s.headers.insert("not a header".into(), "x".into());
        assert!(matches!(
            header_map(&s, Some("k")),
            Err(ProviderError::Config(_))
        ));
    }
}
