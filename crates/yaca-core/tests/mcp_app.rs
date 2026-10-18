//! A server the application runs in its own process (`mcp::app`), end to end:
//! a session is handed one, the model calls its tool, and the session's end
//! is the server's end.
//!
//! The server here is a few lines of JSON-RPC over the channels, the way the
//! desktop window's browser is.

mod common;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use yaca_core::mcp::{AppServer, AppServerSpec, McpSource, McpStatus, McpTransport, ServerEnd};
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Mode, SessionConfig, SessionHandle};

use common::{ask, calls, is_turn_over, tool_result_of, until_within, TempProject};

const ROPE: Duration = Duration::from_secs(30);

/// A one-pixel PNG, the smallest image a server could send.
const PIXEL: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

#[derive(Default)]
struct Echo {
    opened: AtomicUsize,
    closed: Arc<AtomicBool>,
}

impl AppServer for Echo {
    fn open(&self, end: ServerEnd) {
        self.opened.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(serve(end, self.closed.clone()));
    }
}

async fn serve(mut end: ServerEnd, closed: Arc<AtomicBool>) {
    while let Some(message) = end.incoming.recv().await {
        let Some(id) = message.get("id").cloned() else {
            continue;
        };
        let result = match message["method"].as_str().unwrap_or_default() {
            "initialize" => json!({
                "protocolVersion": message["params"]["protocolVersion"],
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "echo", "version": "1" },
                "instructions": "Use echo to hear a text back.",
            }),
            "tools/list" => json!({ "tools": [
                {
                    "name": "echo",
                    "description": "Repeats the text back.",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "text": { "type": "string" } },
                        "required": ["text"],
                    },
                    "annotations": { "readOnlyHint": true },
                },
                {
                    "name": "shot",
                    "description": "Returns a picture.",
                    "inputSchema": { "type": "object", "properties": {} },
                    "annotations": { "readOnlyHint": true },
                },
            ] }),
            "tools/call" if message["params"]["name"] == "shot" => json!({
                "content": [
                    { "type": "text", "text": "a picture" },
                    { "type": "image", "data": PIXEL, "mimeType": "image/png" },
                ],
            }),
            "tools/call" => json!({
                "content": [{ "type": "text", "text": message["params"]["arguments"]["text"] }],
            }),
            _ => json!({}),
        };
        let _ = end
            .outgoing
            .send(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }
    closed.store(true, Ordering::SeqCst);
}

fn session(dir: &TempProject, provider: Arc<MockProvider>, server: Arc<Echo>) -> SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Manual;
    cfg.journal = None;
    cfg.app_servers = vec![AppServerSpec {
        name: "app".into(),
        target: "the window".into(),
        server,
    }];
    yaca_core::spawn(cfg, provider).unwrap()
}

async fn ready(handle: &SessionHandle) {
    tokio::time::timeout(ROPE, async {
        while handle.mcp().iter().any(|s| s.status == McpStatus::Connecting) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the server is still connecting");
}

#[tokio::test]
async fn an_application_server_is_a_server_like_any_other() {
    let dir = TempProject::new();
    let server = Arc::new(Echo::default());
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "t1",
            "mcp__app__echo",
            json!({ "text": "привет", "description": "Эхо" }),
        )])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = session(&dir, provider.clone(), server.clone());
    let mut rx = handle.subscribe();
    ready(&handle).await;

    let listed = handle.mcp();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "app");
    assert_eq!(listed[0].status, McpStatus::Ready, "{:?}", listed[0].reason);
    assert_eq!(listed[0].source, McpSource::App);
    assert_eq!(listed[0].transport, McpTransport::App);
    assert_eq!(listed[0].target, "the window");
    assert_eq!(listed[0].label.as_deref(), Some("echo 1"));

    ask(&handle, "say it back").await;
    let events = until_within(&mut rx, ROPE, is_turn_over).await;
    let kinds: Vec<_> = events.iter().map(|e| e.kind()).collect();
    assert!(!kinds.contains(&"permissionRequested"), "{kinds:?}");
    assert_eq!(tool_result_of(&provider, "t1"), ("привет".to_string(), false));
    let system = provider.seen.lock().unwrap()[0].system.clone();
    assert!(system.contains("Use echo to hear a text back."), "{system}");
    assert_eq!(server.opened.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_server_declared_in_a_file_takes_the_applications_name() {
    let dir = TempProject::new();
    dir.write(
        ".yaca/settings.json",
        &json!({ "mcpServers": { "app": { "command": "no-such-program", "disabled": true } } })
            .to_string(),
    );
    let server = Arc::new(Echo::default());
    let provider = Arc::new(MockProvider::new(vec![]));
    let handle = session(&dir, provider, server.clone());
    ready(&handle).await;

    let listed = handle.mcp();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].source, McpSource::Project);
    assert_eq!(server.opened.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn the_server_is_let_go_with_the_session() {
    let dir = TempProject::new();
    let server = Arc::new(Echo::default());
    let handle = session(&dir, Arc::new(MockProvider::new(vec![])), server.clone());
    ready(&handle).await;
    assert!(!server.closed.load(Ordering::SeqCst));

    drop(handle);
    tokio::time::timeout(ROPE, async {
        while !server.closed.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the server was never told the session ended");
}

#[tokio::test]
async fn a_picture_the_server_returns_is_kept_for_the_feed() {
    let dir = TempProject::new();
    let server = Arc::new(Echo::default());
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("t1", "mcp__app__shot", json!({ "description": "Снимок" }))])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = session(&dir, provider, server);
    let mut rx = handle.subscribe();
    ready(&handle).await;

    ask(&handle, "show me").await;
    let events = until_within(&mut rx, ROPE, is_turn_over).await;
    let pictures = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::ToolCallFinished { meta, .. } => Some(meta.pictures.clone()),
            _ => None,
        })
        .expect("the call did not finish");
    assert_eq!(pictures.len(), 1, "{pictures:?}");
    // Relative to the project, in the window's spelling, and really there.
    assert!(pictures[0].starts_with(".yaca/pasted/"), "{}", pictures[0]);
    assert!(dir.path().join(&pictures[0]).is_file());
}
