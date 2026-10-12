//! An MCP server over Streamable HTTP, against a real socket.
//!
//! The stdio tests cover what a server's tool is to a session; this covers
//! what differs on the wire: a session id handed out at the handshake and
//! owed back with every request after, the protocol revision as a header, an
//! answer that arrives as an event stream, and a server with no stream to
//! listen on.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use yaca_core::event::AgentEvent;
use yaca_core::mcp::McpStatus;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;

use common::wire::{read_one, Seen};
use common::{ask, calls, is_turn_over, session, tool_result_of, until_within, TempProject};

type Log = Arc<Mutex<Vec<Seen>>>;

fn json_answer(body: &Value, extra_header: &str) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{extra_header}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn empty(status: &str) -> String {
    format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

/// The answer to a tool call as an event stream: a progress notice first, so
/// the client has to read past what it did not ask for.
fn stream_answer(id: &Value) -> String {
    let progress = json!({ "jsonrpc": "2.0", "method": "notifications/progress", "params": {} });
    let result = json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "content": [{ "type": "text", "text": "5" }] },
    });
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
         event: message\r\ndata: {progress}\r\n\r\nevent: message\r\ndata: {result}\r\n\r\n"
    )
}

fn answer_to(seen: &Seen) -> String {
    let request_line = seen.headers.lines().next().unwrap_or_default();
    if request_line.starts_with("GET") {
        // No stream to listen on: the client must carry on without one.
        return empty("405 Method Not Allowed");
    }
    if request_line.starts_with("DELETE") {
        return empty("200 OK");
    }
    let id = &seen.body["id"];
    let result = |result: Value| json!({ "jsonrpc": "2.0", "id": id, "result": result });
    match seen.body["method"].as_str().unwrap_or_default() {
        "initialize" => json_answer(
            &result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "web", "version": "2.0" },
            })),
            "Mcp-Session-Id: s-1\r\n",
        ),
        "tools/list" => json_answer(
            &result(json!({ "tools": [{
                "name": "sum", "description": "Adds two numbers.",
                "inputSchema": { "type": "object", "properties": {
                    "a": { "type": "number" }, "b": { "type": "number" },
                } },
                "annotations": { "readOnlyHint": true },
            }] })),
            "",
        ),
        "tools/call" => stream_answer(id),
        // Notifications.
        _ => empty("202 Accepted"),
    }
}

/// A server that answers every request on a connection of its own.
async fn serve() -> (String, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let log = Log::default();

    let seen_by_server = log.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let log = seen_by_server.clone();
            tokio::spawn(async move {
                let Some(seen) = read_one(&mut socket).await else {
                    return;
                };
                let answer = answer_to(&seen);
                log.lock().unwrap().push(seen);
                let _ = socket.write_all(answer.as_bytes()).await;
                let _ = socket.flush().await;
                // A shutdown rather than a drop, as in `wire`.
                let _ = socket.shutdown().await;
            });
        }
    });

    (format!("http://{addr}/mcp"), log)
}

fn sent(log: &Log, method: &str) -> Vec<String> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|seen| seen.body["method"] == method)
        .map(|seen| seen.headers.to_ascii_lowercase())
        .collect()
}

#[tokio::test]
async fn a_remote_servers_tool_is_called_over_http() {
    let (url, log) = serve().await;
    let dir = TempProject::new();
    dir.write(
        ".yaca/settings.json",
        &json!({ "mcpServers": { "web": {
            "type": "http",
            "url": url,
            "headers": {
                "Authorization": "Bearer t0ken",
                "X-Team": "${YACA_MCP_TEST_UNSET_VARIABLE:-fallback}",
            },
        } } })
        .to_string(),
    );
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "mcp__web__sum",
            json!({ "a": 2, "b": 3, "description": "Сложил" }),
        )])),
        Turn::Events(script_text("Five.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "add them").await;
    let events = until_within(&mut rx, Duration::from_secs(30), is_turn_over).await;
    assert!(events.iter().any(|e| matches!(e, AgentEvent::TurnFinished { .. })));

    let servers = handle.mcp();
    assert_eq!(servers[0].status, McpStatus::Ready, "{:?}", servers[0].reason);
    assert_eq!(servers[0].label.as_deref(), Some("web 2.0"));
    assert_eq!(servers[0].target, url);

    // The answer came as an event stream, behind a notice nobody asked for.
    assert_eq!(tool_result_of(&provider, "t1"), ("5".to_string(), false));

    // The handshake goes out bare; everything after it carries what the
    // handshake settled.
    let first = &sent(&log, "initialize")[0];
    assert!(!first.contains("mcp-session-id"), "{first}");
    assert!(first.contains("authorization: bearer t0ken"), "{first}");
    assert!(first.contains("x-team: fallback"), "{first}");

    let call = &sent(&log, "tools/call")[0];
    assert!(call.contains("mcp-session-id: s-1"), "{call}");
    assert!(call.contains("mcp-protocol-version: 2025-06-18"), "{call}");
    assert!(call.contains("authorization: bearer t0ken"), "{call}");
    assert_eq!(sent(&log, "notifications/initialized").len(), 1);

    // The caption stayed with the feed.
    let body = log
        .lock()
        .unwrap()
        .iter()
        .find(|seen| seen.body["method"] == "tools/call")
        .map(|seen| seen.body["params"].clone())
        .unwrap();
    assert_eq!(body, json!({ "name": "sum", "arguments": { "a": 2, "b": 3 } }));
}

#[tokio::test]
async fn an_address_nobody_answers_at_is_a_failed_server() {
    // Bound and dropped: the port is free, and nothing listens on it.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    drop(listener);

    let dir = TempProject::new();
    dir.write(
        ".yaca/settings.json",
        &json!({ "mcpServers": { "web": { "url": url } } }).to_string(),
    );
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("Hello."))]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "hi").await;
    until_within(&mut rx, Duration::from_secs(30), is_turn_over).await;

    let servers = handle.mcp();
    assert_eq!(servers[0].status, McpStatus::Failed);
    assert!(servers[0].reason.as_deref().unwrap_or_default().contains("сеть"));
    let offered = provider.seen.lock().unwrap()[0].tools.clone();
    assert!(offered.iter().all(|tool| !tool.name.starts_with("mcp__")));
}
