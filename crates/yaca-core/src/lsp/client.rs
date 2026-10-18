//! One connection to one language server: requests matched to answers, the
//! handshake, document sync, and the diagnostics the server pushes.
//!
//! Documents are synced by the whole text. Incremental sync would save bytes
//! on huge files, and cost a diff against what the server last saw; the tools
//! that sync a file have just read it anyway.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::{CancellationToken, DropGuard};

use super::config::{language_id, ServerSpec};
use super::wire::{Incoming, Wire};
use super::LspError;
use crate::uri::file_uri;

type Waiting = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, LspError>>>>>;

/// What a server said on its own, for whoever keeps its state.
#[derive(Debug)]
pub(crate) enum Notice {
    Closed(String),
}

/// What the server last published for one document, and how many times it
/// has published for it. The count is what a caller that just changed the
/// document waits on: a newer number is an answer to the change.
#[derive(Default)]
struct Published {
    count: u64,
    items: Vec<Value>,
}

#[derive(Default)]
struct Diagnostics {
    by_uri: Mutex<HashMap<String, Published>>,
    tick: watch::Sender<u64>,
}

/// An open document: its version and a hash of the text the server was last
/// told, so an unchanged file is not sent again.
struct Doc {
    version: i64,
    hash: blake3::Hash,
}

pub(crate) struct Client {
    wire: Arc<Wire>,
    waiting: Waiting,
    next_id: AtomicU64,
    docs: tokio::sync::Mutex<HashMap<String, Doc>>,
    diagnostics: Arc<Diagnostics>,
    /// `name version` as the server introduces itself.
    pub(crate) label: Option<String>,
    /// The server answers `textDocument/diagnostic` (pull diagnostics, LSP
    /// 3.17). Some servers only answer when asked and never publish.
    pulls: bool,
    /// Ends the reading task when the client goes.
    _alive: DropGuard,
}

impl Client {
    /// Starts the server and shakes hands. Not bounded in time here: the
    /// caller owns the startup deadline.
    pub(crate) async fn connect(
        spec: &ServerSpec,
        root: &Path,
        notices: mpsc::UnboundedSender<Notice>,
    ) -> Result<Client, LspError> {
        let (inbox, incoming) = mpsc::unbounded_channel();
        let wire = Arc::new(Wire::spawn(
            &spec.settings.command,
            &spec.settings.args,
            &spec.settings.env,
            root,
            inbox,
        )?);
        let waiting = Waiting::default();
        let diagnostics = Arc::new(Diagnostics::default());
        let alive = CancellationToken::new();
        tokio::spawn(read(
            incoming,
            wire.clone(),
            waiting.clone(),
            diagnostics.clone(),
            root.to_path_buf(),
            notices,
            alive.clone(),
        ));

        let mut client = Client {
            wire,
            waiting,
            next_id: AtomicU64::new(1),
            docs: Default::default(),
            diagnostics,
            label: None,
            pulls: false,
            _alive: alive.drop_guard(),
        };
        client.initialize(root).await?;
        Ok(client)
    }

    async fn initialize(&mut self, root: &Path) -> Result<(), LspError> {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let uri = file_uri(root);
        let never = CancellationToken::new();
        let result = self
            .request(
                "initialize",
                json!({
                    "processId": std::process::id(),
                    "clientInfo": { "name": "yaca", "version": env!("CARGO_PKG_VERSION") },
                    "rootUri": uri,
                    "workspaceFolders": [{ "uri": uri, "name": name }],
                    "capabilities": {
                        // Text edits only: `Rename` applies no file
                        // operations, and a server told so does not send them.
                        "workspace": {
                            "workspaceFolders": true,
                            "configuration": true,
                            "workspaceEdit": { "documentChanges": true, "resourceOperations": [] },
                        },
                        "textDocument": {
                            "synchronization": { "didSave": true },
                            "publishDiagnostics": {},
                            "definition": { "linkSupport": true },
                            "references": {},
                            "hover": { "contentFormat": ["plaintext", "markdown"] },
                            "documentSymbol": { "hierarchicalDocumentSymbolSupport": true },
                            "rename": { "prepareSupport": true },
                            "diagnostic": { "dynamicRegistration": false },
                        },
                    },
                }),
                // The caller's deadline is the real one.
                Duration::from_secs(3_600),
                &never,
            )
            .await?;
        self.pulls = result
            .get("capabilities")
            .and_then(|c| c.get("diagnosticProvider"))
            .is_some_and(|p| !p.is_null() && p != &Value::Bool(false));
        self.label = result
            .get("serverInfo")
            .and_then(|info| {
                let name = info.get("name")?.as_str()?;
                let version = info.get("version").and_then(Value::as_str).unwrap_or("");
                Some(format!("{name} {version}").trim().to_string())
            })
            .filter(|label| !label.is_empty());
        self.notify("initialized", json!({}))
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), LspError> {
        self.wire
            .send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
    }

    /// One request and its answer. Giving up — by the deadline or by `cancel`
    /// — tells the server so with `$/cancelRequest`.
    pub(crate) async fn request(
        &self,
        method: &str,
        params: Value,
        deadline: Duration,
        cancel: &CancellationToken,
    ) -> Result<Value, LspError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.waiting.lock().unwrap().insert(id, tx);

        let sent = self.wire.send(
            &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }),
        );
        if let Err(e) = sent {
            self.waiting.lock().unwrap().remove(&id);
            return Err(e);
        }

        let gave_up = tokio::select! {
            result = rx => {
                return result.unwrap_or_else(|_| Err(LspError::Closed("connection closed".into())));
            }
            _ = cancel.cancelled() => LspError::Cancelled,
            _ = tokio::time::sleep(deadline) => LspError::Timeout(deadline.as_secs()),
        };
        self.waiting.lock().unwrap().remove(&id);
        let _ = self.notify("$/cancelRequest", json!({ "id": id }));
        Err(gave_up)
    }

    /// Tells the server what `path` holds now. Opens the document the first
    /// time and changes it after; does nothing for text it already has.
    ///
    /// Returns the address the server knows the document by, and whether the
    /// server was told anything new — an answer to wait for.
    pub(crate) async fn sync(&self, path: &Path, text: &str) -> Result<(String, bool), LspError> {
        let uri = file_uri(path);
        let hash = blake3::hash(text.as_bytes());
        // Held across the sends: two calls syncing one file must reach the
        // server in the order their versions were taken.
        let mut docs = self.docs.lock().await;
        let mut told = true;
        match docs.get_mut(&uri) {
            Some(doc) if doc.hash == hash => told = false,
            Some(doc) => {
                doc.version += 1;
                doc.hash = hash;
                self.notify(
                    "textDocument/didChange",
                    json!({
                        "textDocument": { "uri": uri, "version": doc.version },
                        "contentChanges": [{ "text": text }],
                    }),
                )?;
                self.notify(
                    "textDocument/didSave",
                    json!({ "textDocument": { "uri": uri } }),
                )?;
            }
            None => {
                let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                self.notify(
                    "textDocument/didOpen",
                    json!({ "textDocument": {
                        "uri": uri,
                        "languageId": language_id(extension),
                        "version": 1,
                        "text": text,
                    } }),
                )?;
                docs.insert(uri.clone(), Doc { version: 1, hash });
            }
        }
        Ok((uri, told))
    }

    /// How many times the server has published diagnostics for `uri`.
    pub(crate) fn published(&self, uri: &str) -> u64 {
        self.diagnostics
            .by_uri
            .lock()
            .unwrap()
            .get(uri)
            .map_or(0, |p| p.count)
    }

    /// What the server last published for `uri`, once it has published more
    /// than `after` times — or what there is, when `wait` runs out first.
    /// The flag says whether a publication arrived in time.
    pub(crate) async fn diagnostics(
        &self,
        uri: &str,
        after: u64,
        wait: Duration,
        cancel: &CancellationToken,
    ) -> (Vec<Value>, bool) {
        // A server that answers when asked is asked: what it says is about the
        // text it was just sent, with no publication to wait for. Not every
        // pull server publishes at all. A pull that fails falls back to what
        // was published.
        if self.pulls {
            let ask = self.request(
                "textDocument/diagnostic",
                serde_json::json!({ "textDocument": { "uri": uri } }),
                wait.max(Duration::from_secs(2)),
                cancel,
            );
            if let Ok(report) = ask.await {
                if let Some(items) = report.get("items").and_then(Value::as_array) {
                    return (items.clone(), true);
                }
            }
        }
        let mut ticks = self.diagnostics.tick.subscribe();
        let deadline = tokio::time::sleep(wait);
        tokio::pin!(deadline);
        let arrived = loop {
            if self.published(uri) > after {
                break true;
            }
            tokio::select! {
                _ = cancel.cancelled() => break false,
                _ = &mut deadline => break false,
                changed = ticks.changed() => {
                    if changed.is_err() {
                        break false;
                    }
                }
            }
        };
        let items = self
            .diagnostics
            .by_uri
            .lock()
            .unwrap()
            .get(uri)
            .map(|p| p.items.clone())
            .unwrap_or_default();
        (items, arrived)
    }
}

/// Everything that arrives from the server, until the client or the server
/// goes.
async fn read(
    mut incoming: mpsc::UnboundedReceiver<Incoming>,
    wire: Arc<Wire>,
    waiting: Waiting,
    diagnostics: Arc<Diagnostics>,
    root: PathBuf,
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
            None => break "connection closed".to_string(),
        };

        let method = message.get("method").and_then(Value::as_str);
        match (method, message.get("id")) {
            // A request of the server's own. Answered at once and never
            // waited for: nothing here depends on what it was asking.
            (Some(method), Some(id)) if !id.is_null() => {
                let reply = match reply_to(method, &message["params"], &root) {
                    Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                    Err(()) => json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32601, "message": format!("method not supported: {method}") },
                    }),
                };
                let _ = wire.send(&reply);
            }
            (Some("textDocument/publishDiagnostics"), _) => {
                let params = &message["params"];
                if let Some(uri) = params.get("uri").and_then(Value::as_str) {
                    let items = params
                        .get("diagnostics")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    {
                        let mut by_uri = diagnostics.by_uri.lock().unwrap();
                        let slot = by_uri.entry(uri.to_string()).or_default();
                        slot.count += 1;
                        slot.items = items;
                    }
                    diagnostics.tick.send_modify(|n| *n += 1);
                }
            }
            // Logs, progress, other notifications: nothing here keeps them.
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

    for (_, waiter) in waiting.lock().unwrap().drain() {
        let _ = waiter.send(Err(LspError::Closed(closed.clone())));
    }
    let _ = notices.send(Notice::Closed(closed));
}

/// The few things a server asks of a client that it must answer to keep
/// working. `Err` for the rest.
fn reply_to(method: &str, params: &Value, root: &Path) -> Result<Value, ()> {
    match method {
        // One null per requested item: "no settings for you".
        "workspace/configuration" => {
            let asked = params
                .get("items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            Ok(Value::Array(vec![Value::Null; asked]))
        }
        "workspace/workspaceFolders" => Ok(json!([{
            "uri": file_uri(root),
            "name": root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        }])),
        "client/registerCapability"
        | "client/unregisterCapability"
        | "window/workDoneProgress/create"
        | "window/showMessageRequest" => Ok(Value::Null),
        _ => Err(()),
    }
}

fn answer_of(message: &Value) -> Result<Value, LspError> {
    if let Some(error) = message.get("error") {
        return Err(LspError::Rpc {
            code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("error without message")
                .to_string(),
        });
    }
    Ok(message.get("result").cloned().unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_is_answered_with_one_null_per_item() {
        let got = reply_to(
            "workspace/configuration",
            &json!({ "items": [{ "section": "a" }, { "section": "b" }] }),
            Path::new("/p"),
        );
        assert_eq!(got, Ok(json!([null, null])));
    }

    #[test]
    fn an_unknown_request_is_refused_rather_than_answered_with_nothing() {
        assert!(reply_to("workspace/applyEdit", &json!({}), Path::new("/p")).is_err());
    }

    #[test]
    fn an_error_answer_keeps_the_servers_words() {
        let err = answer_of(&json!({ "id": 1, "error": { "code": -32801, "message": "content modified" } }))
            .unwrap_err();
        assert!(matches!(err, LspError::Rpc { code: -32801, ref message } if message == "content modified"));
    }
}
