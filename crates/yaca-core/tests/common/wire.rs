//! A one-request HTTP server, for testing a provider against a real socket.
//!
//! Parsing SSE and building a body are covered by unit tests, but HTTP sits
//! between them: headers, status, a connection that ends. Both wire tests run
//! against this rather than against `api.anthropic.com` or anyone else — a
//! test that needs a key and money stops being run in its second week.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use yaca_core::error::ProviderError;
use yaca_core::provider::ProviderStream;

/// What the server saw in the request.
pub struct Seen {
    pub headers: String,
    /// `null` у запроса без тела — то есть у всякого `GET`.
    pub body: serde_json::Value,
}

impl Seen {
    /// Is the header there with this value?
    ///
    /// The name is compared case-blind — the case on the wire is the client's
    /// choice, not the test's — and the value exactly, because a key that
    /// arrived in the wrong case is a key that did not arrive.
    pub fn has_header(&self, name: &str, value: &str) -> bool {
        self.headers.lines().any(|line| match line.split_once(':') {
            Some((k, v)) => k.trim().eq_ignore_ascii_case(name) && v.trim() == value,
            None => false,
        })
    }
}

/// Serves one request with a prepared response. Returns the base URL.
pub async fn serve(response: impl Into<String>) -> (String, oneshot::Receiver<Seen>) {
    let response = response.into();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        if let Some(seen) = read_one(&mut socket).await {
            let _ = tx.send(seen);
        }
        answer(&mut socket, &response).await;
    });

    (format!("http://{addr}"), rx)
}

/// Serves a prepared answer to each request in turn, and reports all of them.
///
/// For the one thing a single request cannot show: a provider that refuses the
/// first attempt and accepts the second. The requests come back together at the
/// end, because what such a test asks is how the second one differed from the
/// first.
pub async fn serve_all(responses: Vec<String>) -> (String, oneshot::Receiver<Vec<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let mut seen = Vec::new();
        for response in &responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            if let Some(one) = read_one(&mut socket).await {
                seen.push(one);
            }
            answer(&mut socket, response).await;
        }
        let _ = tx.send(seen);
    });

    (format!("http://{addr}"), rx)
}

/// Reads one whole request off the socket. `None` — the peer hung up first.
pub async fn read_one(socket: &mut tokio::net::TcpStream) -> Option<Seen> {
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = socket.read(&mut buf).await.unwrap();
        if n == 0 {
            return None;
        }
        raw.extend_from_slice(&buf[..n]);

        let text = String::from_utf8_lossy(&raw).into_owned();
        let Some(head_end) = text.find("\r\n\r\n") else {
            continue;
        };
        let headers = text[..head_end].to_string();
        let body_start = head_end + 4;

        let expected: usize = headers
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse().unwrap_or(0))
            })
            .unwrap_or(0);

        if raw.len() - body_start < expected {
            continue;
        }

        // `GET` тела не несёт вовсе: у опроса моделей его нет, и пустое
        // тело — это `null`, а не «разбор не удался».
        let body = if expected == 0 {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&raw[body_start..body_start + expected]).unwrap()
        };
        return Some(Seen { headers, body });
    }
}

async fn answer(socket: &mut tokio::net::TcpStream, response: &str) {
    socket.write_all(response.as_bytes()).await.unwrap();
    socket.flush().await.unwrap();
    // Именно shutdown, а не drop: на Windows закрытие сокета даёт RST,
    // и тело ответа теряется по дороге. Поток без content-length
    // кончается корректным FIN.
    let _ = socket.shutdown().await;
}

/// A JSON answer with the given status line, its length counted rather than
/// typed: a length that drifted from the body is an empty error message, the
/// very thing such tests look for.
pub fn json_response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}
Content-Type: application/json
Content-Length: {}
Connection: close

{body}",
        body.len()
    )
}

/// `ProviderStream` is not `Debug`, so `unwrap_err` does not work on it.
pub fn refusal(result: Result<ProviderStream, ProviderError>) -> ProviderError {
    match result {
        Ok(_) => panic!("ожидался отказ, пришёл поток"),
        Err(e) => e,
    }
}
