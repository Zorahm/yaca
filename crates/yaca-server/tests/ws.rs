//! The server through a real socket, the way a phone reaches it.
//!
//! Each test starts its own server on an ephemeral port over one shared
//! configuration directory (the host reads it from the environment), and talks
//! to it with a plain WebSocket client: the wire is the contract, so the
//! client here knows nothing of the host's types.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use yaca_server::tokens::Tokens;
use yaca_server::{router, Server};

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// One configuration directory for the whole binary: the host finds it through
/// `XDG_CONFIG_HOME`, which is process-wide, so the tests share it and keep out
/// of each other's way by using their own token names and project folders.
fn config() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("yaca-server-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Before anything reads it, and only here.
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("HOME", &dir);
        dir
    })
}

fn unique(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    format!("{prefix}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

async fn start() -> (String, String) {
    config();
    let dir = yaca_core::provider::registry::config_dir().unwrap();
    let token = Tokens::in_dir(&dir).add(&unique("device")).unwrap();
    let server = Server::new(&dir);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum_serve(listener, router(server)).await });
    (format!("ws://{addr}/ws"), token)
}

async fn axum_serve(listener: TcpListener, app: axum::Router) {
    axum::serve(listener, app).await.unwrap();
}

async fn connect(url: &str) -> Socket {
    tokio_tungstenite::connect_async(url).await.unwrap().0
}

async fn send(ws: &mut Socket, message: Value) {
    ws.send(Message::Text(message.to_string().into())).await.unwrap();
}

async fn next(ws: &mut Socket) -> Option<Value> {
    loop {
        match tokio::time::timeout(Duration::from_secs(10), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => return Some(serde_json::from_str(&text).unwrap()),
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) | Ok(Some(Err(_))) => return None,
            Ok(Some(Ok(_))) => continue,
            Err(_) => panic!("nothing arrived in ten seconds"),
        }
    }
}

async fn login(url: &str, token: &str) -> Socket {
    let mut ws = connect(url).await;
    send(&mut ws, json!({"t": "auth", "token": token, "client": "test"})).await;
    assert_eq!(next(&mut ws).await.unwrap()["t"], "ready");
    ws
}

/// A call and its answer. Events that arrive in between are skipped.
async fn call(ws: &mut Socket, id: u64, cmd: &str, args: Value) -> Result<Value, String> {
    send(ws, json!({"t": "call", "id": id, "cmd": cmd, "args": args})).await;
    loop {
        let message = next(ws).await.expect("the socket closed during a call");
        if message["t"] == "result" && message["id"] == id {
            return if message["ok"] == true {
                Ok(message["value"].clone())
            } else {
                Err(message["error"].as_str().unwrap().to_string())
            };
        }
    }
}

#[tokio::test]
async fn a_wrong_token_is_refused_and_told_so() {
    let (url, _) = start().await;
    let mut ws = connect(&url).await;
    send(&mut ws, json!({"t": "auth", "token": "yct_wrong"})).await;
    let bye = next(&mut ws).await.unwrap();
    assert_eq!(bye, json!({"t": "bye", "reason": "unauthorized"}));
}

#[tokio::test]
async fn a_call_before_authorizing_gets_nothing() {
    let (url, _) = start().await;
    let mut ws = connect(&url).await;
    send(&mut ws, json!({"t": "call", "id": 1, "cmd": "projects", "args": {}})).await;
    assert_eq!(next(&mut ws).await.unwrap()["reason"], "unauthorized");
}

#[tokio::test]
async fn commands_answer_and_errors_say_what_failed() {
    let (url, token) = start().await;
    let mut ws = login(&url, &token).await;

    assert_eq!(call(&mut ws, 1, "projects", json!({})).await.unwrap(), json!([]));

    let err = call(&mut ws, 2, "no_such_command", json!({})).await.unwrap_err();
    assert!(err.contains("unknown command"), "{err}");

    // A required argument that is missing names itself.
    let err = call(&mut ws, 3, "add_project", json!({})).await.unwrap_err();
    assert!(err.contains("path"), "{err}");

    let listing = call(&mut ws, 4, "list_dir", json!({"path": config().display().to_string()}))
        .await
        .unwrap();
    assert_eq!(listing["path"], config().canonicalize().unwrap().display().to_string());
}

#[tokio::test]
async fn preferences_set_here_are_heard_as_an_event() {
    let (url, token) = start().await;
    let mut ws = login(&url, &token).await;

    let mut prefs = call(&mut ws, 1, "preferences", json!({})).await.unwrap();
    prefs["compact"] = json!(true);
    send(&mut ws, json!({"t": "call", "id": 2, "cmd": "set_preferences", "args": {"prefs": prefs}})).await;

    let mut heard = false;
    let mut answered = false;
    while !(heard && answered) {
        let message = next(&mut ws).await.unwrap();
        match message["t"].as_str().unwrap() {
            "event" if message["channel"] == "yaca://preferences" => {
                assert_eq!(message["payload"]["compact"], true);
                heard = true;
            }
            "result" => {
                assert_eq!(message["ok"], true, "{message}");
                answered = true;
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn a_second_device_takes_the_streams_over() {
    let (url, token) = start().await;
    let mut first = login(&url, &token).await;
    let _second = login(&url, &token).await;

    let bye = next(&mut first).await.unwrap();
    assert_eq!(bye, json!({"t": "bye", "reason": "replaced"}));
    assert!(next(&mut first).await.is_none(), "the old connection stays open");
}

#[tokio::test]
async fn a_revoked_device_is_cut_off_at_its_next_ping() {
    let (url, token) = start().await;
    let mut ws = login(&url, &token).await;

    let tokens = Tokens::in_dir(&yaca_core::provider::registry::config_dir().unwrap());
    let name = tokens.check(&token).unwrap();
    tokens.revoke(&name).unwrap();

    send(&mut ws, json!({"t": "ping"})).await;
    assert_eq!(next(&mut ws).await.unwrap()["reason"], "revoked");
    assert!(next(&mut ws).await.is_none());
}

/// A model that answers every request with the same short reply.
async fn fake_model(reply: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                // The request: headers, then as many body bytes as they promise.
                let mut seen = Vec::new();
                let mut chunk = [0u8; 8192];
                let body_at = loop {
                    let n = socket.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    seen.extend_from_slice(&chunk[..n]);
                    if let Some(at) = seen.windows(4).position(|w| w == b"\r\n\r\n") {
                        break at + 4;
                    }
                };
                let head = String::from_utf8_lossy(&seen[..body_at]).to_lowercase();
                let want: usize = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
                while seen.len() < body_at + want {
                    let n = socket.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    seen.extend_from_slice(&chunk[..n]);
                }
                let body = format!(
                    concat!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                        "data: {{\"id\":\"1\",\"model\":\"m\",\"choices\":[{{\"index\":0,\"delta\":{{\"role\":\"assistant\",\"content\":\"\"}}}}]}}\n\n",
                        "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{}\"}}}}]}}\n\n",
                        "data: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\n",
                        "data: {{\"choices\":[],\"usage\":{{\"prompt_tokens\":10,\"completion_tokens\":3}}}}\n\n",
                        "data: [DONE]\n\n",
                    ),
                    reply
                );
                let _ = socket.write_all(body.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}")
}

/// The whole road: a provider is configured from the client, a chat is opened
/// in a folder of the server, a message goes in, the turn's events come out,
/// and a client that comes back after the line dropped finds the chat and its
/// feed where it left them.
#[tokio::test]
async fn a_chat_runs_over_the_wire_and_survives_the_line_dropping() {
    let base = fake_model("Hello from the server.").await;
    let (url, token) = start().await;
    let mut ws = login(&url, &token).await;

    let provider = unique("fake");
    let spec = json!({
        "id": provider, "displayName": "Fake", "protocol": "openai", "baseUrl": base,
        "headers": {},
        "models": [{
            "id": "m", "providerId": provider, "displayName": "M", "contextMax": 100000,
            "maxOutput": 8000, "pricing": {"input": 0.0, "output": 0.0}, "supportsEffort": false
        }]
    });
    call(&mut ws, 1, "save_provider", json!({"spec": spec})).await.unwrap();

    let project = config().join(unique("project"));
    std::fs::create_dir_all(&project).unwrap();
    let project = project.display().to_string();
    let startup = json!({"mode": "auto", "provider": provider, "model": "m", "effort": "medium"});
    let opened = call(&mut ws, 2, "open_project", json!({"path": project, "startup": startup}))
        .await
        .unwrap();
    let session = opened["id"].as_str().unwrap().to_string();

    send(
        &mut ws,
        json!({"t": "call", "id": 3, "cmd": "send_command", "args": {
            "session": session, "command": {"type": "sendMessage", "text": "hi", "attachments": []}
        }}),
    )
    .await;

    // Events arrive in frames; the turn is over when one of them says so.
    let mut text = String::new();
    'turn: loop {
        let message = next(&mut ws).await.expect("closed before the turn ended");
        if message["t"] == "result" && message["id"] == 3 {
            assert_eq!(message["ok"], true, "{message}");
        }
        if message["t"] == "event" && message["channel"] == "yaca://events" {
            assert_eq!(message["payload"]["session"], session);
            for event in message["payload"]["events"].as_array().unwrap() {
                if let Some(delta) = event["text"].as_str().filter(|_| event["type"] == "textDelta") {
                    text.push_str(delta);
                }
                if event["type"] == "turnFinished" {
                    break 'turn;
                }
            }
        }
    }
    assert_eq!(text, "Hello from the server.");

    // The line drops. The chat does not.
    drop(ws);
    let mut again = login(&url, &token).await;
    let attached = call(&mut again, 1, "attach_chats", json!({})).await.unwrap();
    let chat = attached
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == session)
        .expect("the chat is gone after a reconnect");
    assert_eq!(chat["project"], project);
    assert!(chat["epoch"].as_u64().unwrap() >= 1);
    let replayed = serde_json::to_string(&chat["events"]).unwrap();
    assert!(replayed.contains("Hello from the server."), "{replayed}");

    // The key typed on the phone lands on the server, whether it has a keychain
    // or only the private file, and is reported as stored without being returned.
    let asked = call(&mut again, 2, "set_api_key", json!({"provider": provider, "key": "sk-test"}))
        .await
        .unwrap();
    assert!(!asked.to_string().contains("sk-test"));
    let source = call(&mut again, 3, "api_key_source", json!({"provider": provider})).await.unwrap();
    assert_eq!(source, "keychain");
    call(&mut again, 4, "forget_api_key", json!({"provider": provider})).await.unwrap();
    let source = call(&mut again, 5, "api_key_source", json!({"provider": provider})).await.unwrap();
    assert_eq!(source, "none");
}
