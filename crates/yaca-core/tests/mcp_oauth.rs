//! OAuth for a remote MCP server, end to end against a real socket.
//!
//! One fake host plays all the parts: the MCP server (401 without a token),
//! its protected-resource metadata, the authorization server's metadata, the
//! client registration and the token endpoints. The browser is the test: it
//! reads the login page's address from the server's summary and goes straight
//! to the redirect with a code, as an authorization server would send it.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use yaca_core::mcp::oauth::TokenStore;
use yaca_core::mcp::{McpServerSummary, McpStatus};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, SessionConfig, SessionHandle};

use common::TempProject;

const ROPE: Duration = Duration::from_secs(30);

/// What the fake host saw: request lines, and the token requests' forms.
#[derive(Default)]
struct Seen {
    lines: Vec<String>,
    token_forms: Vec<String>,
}

struct Request {
    line: String,
    headers: String,
    body: String,
}

async fn read(socket: &mut TcpStream) -> Option<Request> {
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = socket.read(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        raw.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&raw).into_owned();
        let Some(end) = text.find("\r\n\r\n") else { continue };
        let head = &text[..end];
        let length: usize = head
            .lines()
            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)))
            .unwrap_or(0);
        if raw.len() - (end + 4) < length {
            continue;
        }
        let mut lines = head.lines();
        return Some(Request {
            line: lines.next().unwrap_or_default().to_string(),
            headers: lines.collect::<Vec<_>>().join("\n").to_ascii_lowercase(),
            body: String::from_utf8_lossy(&raw[end + 4..end + 4 + length]).into_owned(),
        });
    }
}

fn json_answer(status: &str, body: &Value, extra: &str) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn answer(base: &str, request: &Request, seen: &Mutex<Seen>) -> String {
    let path = request.line.split_whitespace().nth(1).unwrap_or("/").to_string();
    seen.lock().unwrap().lines.push(request.line.clone());
    match path.as_str() {
        "/.well-known/oauth-protected-resource/mcp" | "/.well-known/oauth-protected-resource" => json_answer(
            "200 OK",
            &json!({ "resource": format!("{base}/mcp"), "authorization_servers": [base] }),
            "",
        ),
        "/.well-known/oauth-authorization-server" => json_answer(
            "200 OK",
            &json!({
                "issuer": base,
                "authorization_endpoint": format!("{base}/authorize"),
                "token_endpoint": format!("{base}/token"),
                "registration_endpoint": format!("{base}/register"),
            }),
            "",
        ),
        "/register" => json_answer("201 Created", &json!({ "client_id": "yaca-test" }), ""),
        "/token" => {
            seen.lock().unwrap().token_forms.push(request.body.clone());
            if request.body.contains("code=good") && request.body.contains("code_verifier=") {
                json_answer(
                    "200 OK",
                    &json!({ "access_token": "tok-1", "token_type": "Bearer", "expires_in": 3600, "refresh_token": "r-1" }),
                    "",
                )
            } else {
                json_answer("400 Bad Request", &json!({ "error": "invalid_grant" }), "")
            }
        }
        "/mcp" => {
            if !request.headers.contains("authorization: bearer tok-1") {
                return json_answer(
                    "401 Unauthorized",
                    &json!({ "error": "unauthorized" }),
                    &format!(
                        "WWW-Authenticate: Bearer resource_metadata=\"{base}/.well-known/oauth-protected-resource/mcp\"\r\n"
                    ),
                );
            }
            if request.line.starts_with("GET") || request.line.starts_with("DELETE") {
                return "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into();
            }
            let message: Value = serde_json::from_str(&request.body).unwrap_or(Value::Null);
            let id = message["id"].clone();
            let result = |result: Value| json!({ "jsonrpc": "2.0", "id": id, "result": result });
            match message["method"].as_str().unwrap_or_default() {
                "initialize" => json_answer(
                    "200 OK",
                    &result(json!({
                        "protocolVersion": "2025-06-18",
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "guarded", "version": "1.0" },
                    })),
                    "",
                ),
                "tools/list" => json_answer("200 OK", &result(json!({ "tools": [] })), ""),
                _ => "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
            }
        }
        _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
    }
}

async fn serve() -> (String, Arc<Mutex<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Seen::default()));
    let (base_task, seen_task) = (base.clone(), seen.clone());
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let (base, seen) = (base_task.clone(), seen_task.clone());
            tokio::spawn(async move {
                let Some(request) = read(&mut socket).await else { return };
                let reply = answer(&base, &request, &seen);
                let _ = socket.write_all(reply.as_bytes()).await;
                let _ = socket.flush().await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (base, seen)
}

fn open(dir: &TempProject, tokens: &TempProject, url: &str) -> SessionHandle {
    dir.write(
        ".yaca/settings.json",
        &json!({ "mcpServers": { "guarded": { "type": "http", "url": url } } }).to_string(),
    );
    let mut config = SessionConfig::new(dir.path());
    config.mode = Mode::Auto;
    config.journal = None;
    config.mcp_tokens = TokenStore::Dir(tokens.path().to_path_buf());
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("ok"))]));
    yaca_core::spawn(config, provider).unwrap()
}

async fn until(handle: &SessionHandle, ok: impl Fn(&McpServerSummary) -> bool) -> McpServerSummary {
    tokio::time::timeout(ROPE, async {
        loop {
            if let Some(server) = handle.mcp().into_iter().find(|s| ok(s)) {
                return server;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the server never got there")
}

fn param(url: &str, key: &str) -> String {
    let parsed = reqwest::Url::parse(url).unwrap();
    parsed
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| panic!("no {key} in {url}"))
}

#[tokio::test]
async fn a_server_that_asks_for_a_login_gets_one_and_connects() {
    let (base, seen) = serve().await;
    let url = format!("{base}/mcp");
    let dir = TempProject::new();
    let tokens = TempProject::new();
    let handle = open(&dir, &tokens, &url);

    let waiting = until(&handle, |s| s.status == McpStatus::NeedsLogin).await;
    assert!(!waiting.logged_in);

    handle.send(Command::LoginMcpServer { server: "guarded".into() }).await.unwrap();
    let page = until(&handle, |s| s.login_url.is_some()).await.login_url.unwrap();
    assert!(page.starts_with(&format!("{base}/authorize?")), "{page}");
    assert_eq!(param(&page, "client_id"), "yaca-test");
    assert_eq!(param(&page, "code_challenge_method"), "S256");
    assert_eq!(param(&page, "resource"), url);

    // The browser, after the person said yes.
    let back = format!("{}?code=good&state={}", param(&page, "redirect_uri"), param(&page, "state"));
    let landing = reqwest::get(&back).await.unwrap().text().await.unwrap();
    assert!(landing.contains("logged in"), "{landing}");

    let ready = until(&handle, |s| s.status == McpStatus::Ready).await;
    assert!(ready.logged_in);
    let form = seen.lock().unwrap().token_forms[0].clone();
    assert!(form.contains("grant_type=authorization_code") && form.contains("redirect_uri="), "{form}");
    assert!(std::fs::read_dir(tokens.path()).unwrap().count() == 1, "the login was kept");

    handle.send(Command::LogoutMcpServer { server: "guarded".into() }).await.unwrap();
    until(&handle, |s| s.status == McpStatus::NeedsLogin).await;
    assert_eq!(std::fs::read_dir(tokens.path()).unwrap().count(), 0, "the login was forgotten");
}

#[tokio::test]
async fn a_refused_login_leaves_the_server_waiting_with_the_reason() {
    let (base, _seen) = serve().await;
    let dir = TempProject::new();
    let tokens = TempProject::new();
    let handle = open(&dir, &tokens, &format!("{base}/mcp"));
    until(&handle, |s| s.status == McpStatus::NeedsLogin).await;

    handle.send(Command::LoginMcpServer { server: "guarded".into() }).await.unwrap();
    let page = until(&handle, |s| s.login_url.is_some()).await.login_url.unwrap();
    let back = format!(
        "{}?error=access_denied&state={}",
        param(&page, "redirect_uri"),
        param(&page, "state")
    );
    let _ = reqwest::get(&back).await.unwrap().text().await;

    let failed = until(&handle, |s| s.status == McpStatus::NeedsLogin && s.reason.as_deref().is_some_and(|r| r.contains("access_denied"))).await;
    assert!(!failed.logged_in);
}
