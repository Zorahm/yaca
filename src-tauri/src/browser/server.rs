//! The browser as an MCP server, spoken to by the chat's session in-process
//! (`yaca_core::mcp::app`).
//!
//! The server side of the protocol is small: the handshake, the tool list and
//! tool calls. Calls are taken one at a time — two gestures interleaved on
//! one page make neither — and each marks the browser busy while it runs,
//! which is what the column's progress line and the globe's dot show.

use std::sync::{Arc, Weak};

use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;
use yaca_core::mcp::{AppServer, ServerEnd};

use super::tools;
use super::Browser;

/// Revisions this server speaks, newest first.
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// Holds its browser weakly: the browser belongs to the chat, and a session
/// that outlives its chat in the shell's table must not keep a webview open.
pub struct Server(pub Weak<Browser>);

impl AppServer for Server {
    fn open(&self, end: ServerEnd) {
        tokio::spawn(serve(self.0.clone(), end));
    }
}

async fn serve(browser: Weak<Browser>, mut end: ServerEnd) {
    while let Some(message) = end.incoming.recv().await {
        // Notifications want no answer, and an answer to a request of ours
        // cannot come: the server asks nothing.
        let (Some(id), Some(method)) = (message.get("id").cloned(), message["method"].as_str()) else {
            continue;
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => {
                let asked = params["protocolVersion"].as_str().unwrap_or_default();
                let version = VERSIONS.iter().find(|v| **v == asked).unwrap_or(&VERSIONS[0]);
                answer(
                    &end.outgoing,
                    id,
                    json!({
                        "protocolVersion": version,
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "yaca-browser", "version": env!("CARGO_PKG_VERSION") },
                        "instructions": tools::INSTRUCTIONS,
                    }),
                );
            }
            "ping" => answer(&end.outgoing, id, json!({})),
            "tools/list" => answer(&end.outgoing, id, json!({ "tools": tools::list() })),
            "tools/call" => {
                let Some(browser) = browser.upgrade() else {
                    refuse(&end.outgoing, id, -32000, "the chat's browser is closed");
                    continue;
                };
                let out = end.outgoing.clone();
                tokio::spawn(async move {
                    let name = params["name"].as_str().unwrap_or_default().to_string();
                    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                    let reply = run(&browser, &name, &args).await;
                    answer(&out, id, json!({ "content": reply.content, "isError": reply.error }));
                });
            }
            _ => refuse(&end.outgoing, id, -32601, &format!("no method {method}")),
        }
    }
}

async fn run(browser: &Arc<Browser>, name: &str, args: &Value) -> tools::Reply {
    let _turn = browser.calls.lock().await;
    let _busy = browser.busy();
    tools::call(browser, name, args).await
}

fn answer(out: &UnboundedSender<Value>, id: Value, result: Value) {
    let _ = out.send(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
}

fn refuse(out: &UnboundedSender<Value>, id: Value, code: i64, message: &str) {
    let _ = out.send(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    }));
}
