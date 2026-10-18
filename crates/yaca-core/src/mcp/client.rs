//! One connection to one server: requests matched to their answers, the
//! handshake, and the few things a server may ask of a client.
//!
//! Sampling is not declared, so a server does not ask for it. Elicitation
//! is: a server's request for the human's input goes to the hub, which puts
//! a form on screen and answers with what the person did. Roots are: a file server needs to know where the project is, and the
//! answer is the session's own workspace roots.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::{CancellationToken, DropGuard};

use super::config::Launch;
use super::protocol::{
    CallResult, GetPromptResult, InitializeResult, PromptsPage, ReadResult, RemotePrompt,
    RemoteResource, RemoteTool, ResourcesPage, ToolsPage, PROMPTS_LIST_CHANGED, TOOLS_LIST_CHANGED,
    VERSIONS,
};
use super::transport::{self, Incoming, Transport};
use crate::uri::file_uri;
use super::McpError;

/// A server that pages forever is a server that is broken; stop asking.
const MAX_PAGES: usize = 50;

/// How long a request that is not a tool call may take once the server is up:
/// listing and reading are lookups.
pub(crate) const LOOKUP_TIMEOUT: Duration = Duration::from_secs(60);

type Waiting = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, McpError>>>>>;

/// What a server said on its own, for whoever keeps its state.
#[derive(Debug)]
pub(crate) enum Notice {
    ToolsChanged,
    PromptsChanged,
    /// `elicitation/create`: the params, and where the answer goes.
    Elicit {
        params: Value,
        answer: oneshot::Sender<Value>,
    },
    Closed(String),
}

/// What the handshake told about the server.
#[derive(Debug, Clone, Default)]
pub(crate) struct ServerInfo {
    /// `name version`, as the server introduces itself.
    pub label: Option<String>,
    pub instructions: Option<String>,
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
}

pub(crate) struct Client {
    transport: Arc<dyn Transport>,
    waiting: Waiting,
    next_id: AtomicU64,
    pub(crate) info: ServerInfo,
    /// Ends the reading task when the client goes, and with it the task's hold
    /// on the transport — which is what stops the server.
    _alive: DropGuard,
}

impl Client {
    /// Opens the transport and shakes hands. Not bounded in time here: the
    /// caller owns the startup deadline and drops this future when it passes.
    pub(crate) async fn connect(
        launch: Launch,
        roots: Vec<PathBuf>,
        notices: mpsc::UnboundedSender<Notice>,
    ) -> Result<Client, McpError> {
        let (inbox, incoming) = mpsc::unbounded_channel();
        let transport = transport::open(launch, inbox).await?;
        let waiting = Waiting::default();
        let alive = CancellationToken::new();
        tokio::spawn(read(
            incoming,
            transport.clone(),
            waiting.clone(),
            roots,
            notices,
            alive.clone(),
        ));

        let mut client = Client {
            transport,
            waiting,
            next_id: AtomicU64::new(1),
            info: ServerInfo::default(),
            _alive: alive.drop_guard(),
        };
        client.initialize().await?;
        Ok(client)
    }

    async fn initialize(&mut self) -> Result<(), McpError> {
        let never = CancellationToken::new();
        let result: InitializeResult = self
            .ask(
                "initialize",
                Some(json!({
                    "protocolVersion": VERSIONS[0],
                    "capabilities": { "roots": { "listChanged": false }, "elicitation": {} },
                    "clientInfo": { "name": "yaca", "version": env!("CARGO_PKG_VERSION") },
                })),
                // The caller's deadline is the real one.
                Duration::from_secs(3_600),
                &never,
            )
            .await?;

        if !VERSIONS.contains(&result.protocol_version.as_str()) {
            return Err(McpError::Protocol(format!(
                "сервер говорит на версии протокола {}, а клиент знает {}",
                result.protocol_version,
                VERSIONS.join(", ")
            )));
        }
        self.transport.negotiated(&result.protocol_version);
        self.notify("notifications/initialized", None).await?;

        self.info = ServerInfo {
            label: result
                .server_info
                .map(|info| format!("{} {}", info.name, info.version).trim().to_string())
                .filter(|label| !label.is_empty()),
            instructions: result
                .instructions
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty()),
            tools: result.capabilities.tools.is_some(),
            resources: result.capabilities.resources.is_some(),
            prompts: result.capabilities.prompts.is_some(),
        };
        Ok(())
    }

    async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpError> {
        self.transport.send(envelope(None, method, params)).await
    }

    /// One request and its answer.
    ///
    /// Giving up — by the deadline or by `cancel` — tells the server so, which
    /// is the protocol's way of stopping work nobody is waiting for.
    pub(crate) async fn request(
        &self,
        method: &str,
        params: Option<Value>,
        deadline: Duration,
        cancel: &CancellationToken,
    ) -> Result<Value, McpError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.waiting.lock().unwrap().insert(id, tx);

        // Sending is inside the race: over HTTP the POST itself may last as
        // long as the server takes to answer.
        let answered = async {
            self.transport.send(envelope(Some(id), method, params)).await?;
            rx.await
                .unwrap_or_else(|_| Err(McpError::Closed("соединение закрыто".into())))
        };
        let gave_up = tokio::select! {
            result = answered => {
                self.waiting.lock().unwrap().remove(&id);
                return result;
            }
            _ = cancel.cancelled() => McpError::Cancelled,
            _ = tokio::time::sleep(deadline) => McpError::Timeout(deadline.as_secs()),
        };

        self.waiting.lock().unwrap().remove(&id);
        let _ = self
            .notify(
                "notifications/cancelled",
                Some(json!({ "requestId": id, "reason": gave_up.to_string() })),
            )
            .await;
        Err(gave_up)
    }

    async fn ask<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Option<Value>,
        deadline: Duration,
        cancel: &CancellationToken,
    ) -> Result<T, McpError> {
        let value = self.request(method, params, deadline, cancel).await?;
        serde_json::from_value(value)
            .map_err(|e| McpError::Protocol(format!("ответ на {method} не разобрать: {e}")))
    }

    pub(crate) async fn list_tools(&self) -> Result<Vec<RemoteTool>, McpError> {
        if !self.info.tools {
            return Ok(Vec::new());
        }
        let never = CancellationToken::new();
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let params = cursor.as_ref().map(|cursor| json!({ "cursor": cursor }));
            let page: ToolsPage = self.ask("tools/list", params, LOOKUP_TIMEOUT, &never).await?;
            all.extend(page.tools);
            cursor = page.next_cursor.filter(|next| !next.is_empty());
            if cursor.is_none() {
                break;
            }
        }
        Ok(all)
    }

    pub(crate) async fn list_prompts(&self) -> Result<Vec<RemotePrompt>, McpError> {
        if !self.info.prompts {
            return Ok(Vec::new());
        }
        let never = CancellationToken::new();
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let params = cursor.as_ref().map(|cursor| json!({ "cursor": cursor }));
            let page: PromptsPage = self.ask("prompts/list", params, LOOKUP_TIMEOUT, &never).await?;
            all.extend(page.prompts);
            cursor = page.next_cursor.filter(|next| !next.is_empty());
            if cursor.is_none() {
                break;
            }
        }
        Ok(all)
    }

    pub(crate) async fn get_prompt(
        &self,
        name: &str,
        arguments: Value,
        cancel: &CancellationToken,
    ) -> Result<GetPromptResult, McpError> {
        let params = json!({ "name": name, "arguments": arguments });
        self.ask("prompts/get", Some(params), LOOKUP_TIMEOUT, cancel).await
    }

    pub(crate) async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        deadline: Duration,
        cancel: &CancellationToken,
    ) -> Result<CallResult, McpError> {
        let params = json!({ "name": name, "arguments": arguments });
        self.ask("tools/call", Some(params), deadline, cancel).await
    }

    pub(crate) async fn list_resources(
        &self,
        cancel: &CancellationToken,
    ) -> Result<Vec<RemoteResource>, McpError> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let params = cursor.as_ref().map(|cursor| json!({ "cursor": cursor }));
            let page: ResourcesPage =
                self.ask("resources/list", params, LOOKUP_TIMEOUT, cancel).await?;
            all.extend(page.resources);
            cursor = page.next_cursor.filter(|next| !next.is_empty());
            if cursor.is_none() {
                break;
            }
        }
        Ok(all)
    }

    pub(crate) async fn read_resource(
        &self,
        uri: &str,
        cancel: &CancellationToken,
    ) -> Result<ReadResult, McpError> {
        self.ask("resources/read", Some(json!({ "uri": uri })), LOOKUP_TIMEOUT, cancel)
            .await
    }
}

/// A request when it has an id, a notification when it has none. `params` is
/// left out rather than sent as `null`: some servers refuse the null.
fn envelope(id: Option<u64>, method: &str, params: Option<Value>) -> Value {
    let mut message = json!({ "jsonrpc": "2.0", "method": method });
    if let Some(id) = id {
        message["id"] = json!(id);
    }
    if let Some(params) = params {
        message["params"] = params;
    }
    message
}

/// Everything that arrives from the server, until the client or the server
/// goes.
async fn read(
    mut incoming: mpsc::UnboundedReceiver<Incoming>,
    transport: Arc<dyn Transport>,
    waiting: Waiting,
    roots: Vec<PathBuf>,
    notices: mpsc::UnboundedSender<Notice>,
    alive: CancellationToken,
) {
    let closed = loop {
        let next = tokio::select! {
            _ = alive.cancelled() => return,
            next = incoming.recv() => next,
        };
        let message = match next {
            Some(Incoming::Message(message)) => message,
            Some(Incoming::Closed(reason)) => break reason,
            None => break "соединение закрыто".to_string(),
        };

        let method = message.get("method").and_then(Value::as_str);
        match (method, message.get("id")) {
            // The human is asked; the answer goes back whenever it comes,
            // and the loop goes on reading meanwhile.
            (Some("elicitation/create"), Some(id)) if !id.is_null() => {
                let (tx, rx) = oneshot::channel();
                let params = message.get("params").cloned().unwrap_or(Value::Null);
                let _ = notices.send(Notice::Elicit { params, answer: tx });
                let transport = transport.clone();
                let id = id.clone();
                tokio::spawn(async move {
                    let result = rx.await.unwrap_or_else(|_| json!({ "action": "cancel" }));
                    let _ = transport
                        .send(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
                        .await;
                });
            }
            // A request of the server's own.
            (Some(method), Some(id)) if !id.is_null() => {
                let reply = match method {
                    "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
                    "roots/list" => json!({
                        "jsonrpc": "2.0", "id": id, "result": { "roots": roots_of(&roots) },
                    }),
                    other => json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32601, "message": format!("method not supported: {other}") },
                    }),
                };
                // Not awaited here: over HTTP a send can take a while, and the
                // next answer may be the one a call is waiting for.
                let transport = transport.clone();
                tokio::spawn(async move {
                    let _ = transport.send(reply).await;
                });
            }
            (Some(TOOLS_LIST_CHANGED), _) => {
                let _ = notices.send(Notice::ToolsChanged);
            }
            (Some(PROMPTS_LIST_CHANGED), _) => {
                let _ = notices.send(Notice::PromptsChanged);
            }
            // Progress, log lines, changed resources: nothing here keeps them.
            (Some(_), _) => {}
            (None, Some(id)) => {
                let Some(waiter) = id.as_u64().and_then(|id| waiting.lock().unwrap().remove(&id))
                else {
                    continue;
                };
                let _ = waiter.send(answer_of(&message));
            }
            (None, None) => {}
        }
    };

    // Whoever is still waiting gets the reason rather than their deadline.
    for (_, waiter) in waiting.lock().unwrap().drain() {
        let _ = waiter.send(Err(McpError::Closed(closed.clone())));
    }
    let _ = notices.send(Notice::Closed(closed));
}

fn answer_of(message: &Value) -> Result<Value, McpError> {
    if let Some(error) = message.get("error") {
        return Err(McpError::Rpc {
            code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("ошибка без текста")
                .to_string(),
        });
    }
    Ok(message.get("result").cloned().unwrap_or(Value::Null))
}

fn roots_of(roots: &[PathBuf]) -> Vec<Value> {
    roots
        .iter()
        .map(|root| {
            let name = root
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            json!({ "uri": file_uri(root), "name": name })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_without_params_carries_no_params_key() {
        let message = envelope(Some(3), "tools/list", None);
        assert_eq!(message, json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }));
        let note = envelope(None, "notifications/initialized", None);
        assert!(note.get("id").is_none());
    }

    #[test]
    fn an_error_answer_keeps_the_servers_words() {
        let err = answer_of(&json!({ "id": 1, "error": { "code": -32602, "message": "no such tool" } }))
            .unwrap_err();
        assert!(matches!(err, McpError::Rpc { code: -32602, ref message } if message == "no such tool"));
    }
}
