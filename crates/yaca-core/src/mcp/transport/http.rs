//! Streamable HTTP: every message is a POST to one address, and the answer is
//! either a JSON body or an event stream that carries it.
//!
//! The server may also speak first — a changed tool list, a request of its
//! own — and for that the client keeps one GET stream open beside the POSTs. A
//! server that does not offer one answers 405, and then it simply never speaks
//! first.
//!
//! Authorization is whatever the declaration's headers say, or the bearer
//! token of an OAuth login (`mcp::oauth`), which the hub adds as a header. A
//! 401 is its own error, so the hub can ask the human to log in.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::{Inbox, Incoming, Transport};
use crate::mcp::McpError;
use crate::provider::sse::SseDecoder;

const SESSION_HEADER: &str = "mcp-session-id";
const VERSION_HEADER: &str = "mcp-protocol-version";

/// How long the listening stream waits before it is opened again after the
/// server closed it.
const LISTEN_AGAIN_AFTER: Duration = Duration::from_secs(2);

pub(crate) struct Http {
    link: Arc<Link>,
    /// Stops the listening stream and any answer still being read when the
    /// transport goes.
    alive: CancellationToken,
}

struct Link {
    http: reqwest::Client,
    url: String,
    /// The declaration's headers.
    headers: HeaderMap,
    /// Given by the server with its answer to `initialize`, and sent back with
    /// everything after.
    session: Mutex<Option<String>>,
    version: Mutex<Option<String>>,
    inbox: Inbox,
}

impl Http {
    pub(crate) fn new(
        url: String,
        headers: &BTreeMap<String, String>,
        inbox: Inbox,
    ) -> Result<Self, McpError> {
        let mut map = HeaderMap::new();
        for (name, value) in headers {
            let bad = |what: &str| McpError::Transport(format!("header «{name}»: {what}"));
            let key = HeaderName::try_from(name.as_str()).map_err(|e| bad(&e.to_string()))?;
            let value = HeaderValue::try_from(value.as_str()).map_err(|e| bad(&e.to_string()))?;
            map.insert(key, value);
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| McpError::Transport(e.to_string()))?;
        Ok(Http {
            link: Arc::new(Link {
                http,
                url,
                headers: map,
                session: Mutex::default(),
                version: Mutex::default(),
                inbox,
            }),
            alive: CancellationToken::new(),
        })
    }
}

impl Link {
    fn request(&self, method: reqwest::Method) -> reqwest::RequestBuilder {
        let mut request = self.http.request(method, &self.url).headers(self.headers.clone());
        if let Some(session) = self.session.lock().unwrap().as_deref() {
            request = request.header(SESSION_HEADER, session);
        }
        if let Some(version) = self.version.lock().unwrap().as_deref() {
            request = request.header(VERSION_HEADER, version);
        }
        request
    }

    fn deliver(&self, message: Value) {
        // A JSON body may be one message or a batch of them.
        match message {
            Value::Array(batch) => {
                for one in batch {
                    let _ = self.inbox.send(Incoming::Message(one));
                }
            }
            one => {
                let _ = self.inbox.send(Incoming::Message(one));
            }
        }
    }

    /// Reads an event stream to its end, handing up each message. Returns
    /// whether the message answering `awaited` was among them.
    async fn pump(&self, response: reqwest::Response, awaited: Option<&Value>) -> bool {
        let mut decoder = SseDecoder::new();
        let mut bytes = response.bytes_stream();
        let mut answered = false;
        while let Some(Ok(chunk)) = bytes.next().await {
            for data in decoder.push(&chunk) {
                let Ok(message) = serde_json::from_str::<Value>(&data) else {
                    continue;
                };
                if awaited.is_some() && message.get("id") == awaited && !is_request(&message) {
                    answered = true;
                }
                self.deliver(message);
            }
        }
        answered
    }
}

fn is_request(message: &Value) -> bool {
    message.get("method").is_some()
}

fn is_event_stream(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|kind| kind.starts_with("text/event-stream"))
}

#[async_trait]
impl Transport for Http {
    async fn send(&self, message: Value) -> Result<(), McpError> {
        let link = &self.link;
        // Set only for a request of ours: that is what an answer is owed to.
        let awaited = message.get("id").filter(|_| is_request(&message)).cloned();

        let response = link
            .request(reqwest::Method::POST)
            .header(ACCEPT, "application/json, text/event-stream")
            .json(&message)
            .send()
            .await
            .map_err(|e| McpError::Transport(format!("network: {e}")))?;

        let status = response.status();
        if status == StatusCode::NOT_FOUND && link.session.lock().unwrap().is_some() {
            // The protocol's way of saying the session is over. Everything
            // after this would get the same answer.
            let reason = "the server no longer knows this session".to_string();
            let _ = link.inbox.send(Incoming::Closed(reason.clone()));
            return Err(McpError::Closed(reason));
        }
        if status == StatusCode::UNAUTHORIZED {
            let challenge = response
                .headers()
                .get("www-authenticate")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            return Err(McpError::Unauthorized(challenge));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let body: String = body.chars().take(500).collect();
            return Err(McpError::Transport(format!("HTTP {}: {}", status.as_u16(), body.trim())));
        }
        if let Some(session) = response
            .headers()
            .get(SESSION_HEADER)
            .and_then(|value| value.to_str().ok())
        {
            *link.session.lock().unwrap() = Some(session.to_string());
        }

        // A notification or an answer of ours: accepted, nothing comes back.
        if status == StatusCode::ACCEPTED || awaited.is_none() {
            return Ok(());
        }

        if is_event_stream(&response) {
            // Read off this call: the stream stays open for as long as the
            // server works on the request, and the caller is already waiting
            // for the answer by its id.
            let link = link.clone();
            let alive = self.alive.clone();
            tokio::spawn(async move {
                let answered = tokio::select! {
                    _ = alive.cancelled() => return,
                    answered = link.pump(response, awaited.as_ref()) => answered,
                };
                if !answered {
                    // The stream ended without the answer. Said in its place,
                    // or the caller would wait out its whole timeout.
                    link.deliver(json!({
                        "jsonrpc": "2.0",
                        "id": awaited,
                        "error": { "code": -32000, "message": "the stream ended before the answer" },
                    }));
                }
            });
            return Ok(());
        }

        let body: Value = response
            .json()
            .await
            .map_err(|e| McpError::Protocol(format!("answer is not JSON: {e}")))?;
        link.deliver(body);
        Ok(())
    }

    fn negotiated(&self, version: &str) {
        *self.link.version.lock().unwrap() = Some(version.to_string());
        tokio::spawn(listen(self.link.clone(), self.alive.clone()));
    }
}

/// The stream the server speaks first on. Reopened when the server closes it,
/// given up on when the server refuses it.
async fn listen(link: Arc<Link>, alive: CancellationToken) {
    loop {
        let opened = tokio::select! {
            _ = alive.cancelled() => return,
            opened = link.request(reqwest::Method::GET).header(ACCEPT, "text/event-stream").send() => opened,
        };
        match opened {
            Ok(response) if response.status().is_success() && is_event_stream(&response) => {
                tokio::select! {
                    _ = alive.cancelled() => return,
                    _ = link.pump(response, None) => {}
                }
            }
            // 405 and anything else: this server has no such stream.
            Ok(_) => return,
            Err(err) => {
                tracing::debug!(%err, "MCP listening stream could not be opened");
                return;
            }
        }
        tokio::select! {
            _ = alive.cancelled() => return,
            _ = tokio::time::sleep(LISTEN_AGAIN_AFTER) => {}
        }
    }
}

impl Drop for Http {
    fn drop(&mut self) {
        self.alive.cancel();
        // The protocol asks a client that is done to say so. Best effort: the
        // session ends on the server's own clock either way.
        if self.link.session.lock().unwrap().is_none() {
            return;
        }
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let request = self.link.request(reqwest::Method::DELETE);
            runtime.spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(5), request.send()).await;
            });
        }
    }
}
