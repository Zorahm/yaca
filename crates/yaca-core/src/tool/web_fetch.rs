//! Fetching a page from the web (§5.7, the `Reach` row of the §8 matrix).
//!
//! The permission is asked about a domain, and it is asked before the request
//! goes out. That single fact shapes everything here: redirects are followed by
//! hand rather than by the client, because a client that follows a redirect to
//! another host turns "allow docs.rs" into "allow anywhere" without anyone
//! being asked (§0).
//!
//! Turning the body into text lives in [`super::page`]. What is left here is
//! the network; the client, the body ceiling and the untrusted-content
//! sentence live in [`super::net`].

use async_trait::async_trait;
use reqwest::header::{CONTENT_TYPE, LOCATION};
use reqwest::Url;
use serde_json::{json, Value};

use super::net;
use super::page;
use super::robots;
use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::{PermissionKind, PermissionNeed};
use crate::workspace::Workspace;

const MAX_REDIRECTS: usize = 5;

pub struct WebFetch;

/// Host of a URL that [`parse`] has already vetted.
///
/// `http` and `https` are what parse allows, and the URL parser requires a host
/// for both, so the fallback is unreachable.
fn host(url: &Url) -> &str {
    url.host_str().unwrap_or_default()
}

fn parse(input: &Value) -> Result<Url, ToolError> {
    let raw = input["url"]
        .as_str()
        .ok_or_else(|| ToolError::failed("не задан url"))?;
    let url =
        Url::parse(raw).map_err(|e| ToolError::failed(format!("плохой адрес `{raw}`: {e}")))?;

    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(ToolError::failed(format!(
            "схема `{other}` не поддерживается, нужен http или https"
        ))),
    }
}

#[async_trait]
impl Tool for WebFetch {
    fn name(&self) -> &'static str {
        "WebFetch"
    }

    fn description(&self) -> &'static str {
        "Fetch a page from the web and read it as text. HTML comes back as \
         Markdown; JSON, plain text and XML come back as they are. GET only, \
         http and https only. Binary responses are refused."
    }

    fn properties(&self) -> Value {
        json!({
            "url": {
                "type": "string",
                "description": "Absolute http or https URL of the page to fetch.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["url"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    fn intents(&self) -> &'static [Intent] {
        // Reading a page changes nothing in the project, and looking something
        // up in the documentation is half of what planning is.
        &[Intent::Plan, Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["url"].as_str().map(str::to_owned)
    }

    fn permission(&self, _ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let url = parse(input).ok()?;
        let host = host(&url).to_string();
        Some(
            PermissionNeed::new(PermissionKind::FetchUrl, self.name(), host.clone())
                .detail(url.as_str())
                .domain(host),
        )
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let start = parse(&input)?;

        // До первого запроса за страницей: `robots.txt` — это про то, куда
        // ходить, и спрашивать его после того, как сходил, поздно (§10).
        if ctx.respect_robots {
            robots::check(ctx, &start).await?;
        }

        let (url, response) = follow(ctx, start).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::failed(format!(
                "HTTP {} · {}",
                status.as_u16(),
                host(&url)
            )));
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let mime = page::mime(&content_type).to_string();

        let body = net::read_body(ctx, response).await?;

        // Parsing a megabyte of HTML is work for a thread, not for the reactor
        // the rest of the session is sharing.
        let rendered = tokio::task::spawn_blocking(move || page::render(&body, &content_type))
            .await
            .map_err(|e| ToolError::failed(format!("разбор страницы не завершился: {e}")))??;

        // The feed gets the page, the model gets the page under a header. The
        // header says the content is data and not instructions, and that
        // sentence is for the model — in the feed it would be noise.
        ctx.progress.push(rendered.text.clone());
        let lines = rendered.text.lines().count() as u32;

        let untrusted = net::UNTRUSTED;
        let text = format!(
            "{url} · {} · {} · {} chars\n{untrusted}\n\n{}",
            status.as_u16(),
            mime,
            rendered.chars,
            rendered.text
        );

        Ok(ToolOutput::new(text)
            .lines(lines)
            .truncated(rendered.truncated))
    }
}

/// Walks the redirect chain by hand and returns the response that ends it,
/// together with the URL it actually came from.
///
/// Leaving the host is refused, not followed: the human answered a question
/// about this domain. It comes back as [`ToolError::Rejected`] rather than
/// `Failed` because nothing broke — a boundary held (§5.9) — and the new
/// address travels to the model so it can ask for it properly.
async fn follow(ctx: &ToolCtx, start: Url) -> Result<(Url, reqwest::Response), ToolError> {
    let mut url = start.clone();

    for _ in 0..=MAX_REDIRECTS {
        ctx.check_cancelled()?;

        let response =
            net::client().get(url.clone()).send().await.map_err(|e| {
                ToolError::failed(format!("не удалось загрузить {}: {e}", host(&url)))
            })?;

        if !response.status().is_redirection() {
            return Ok((url, response));
        }

        let next = response
            .headers()
            .get(LOCATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| url.join(v).ok())
            .ok_or_else(|| {
                ToolError::failed(format!("HTTP {} без Location", response.status().as_u16()))
            })?;

        // Редирект внутри хоста ведёт на другой путь, а правила пишутся по
        // путям: разрешённый адрес мог бы увести на запрещённый.
        if ctx.respect_robots && host(&next) == host(&start) {
            robots::check(ctx, &next).await?;
        }

        if host(&next) != host(&start) {
            return Err(ToolError::Rejected(format!(
                "редирект с {} на другой хост: {next}. Разрешение выдано на {}. \
                 Повтори вызов с новым адресом, если он нужен.",
                host(&start),
                host(&start)
            )));
        }

        url = next;
    }

    Err(ToolError::failed(format!(
        "больше {MAX_REDIRECTS} редиректов подряд"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::AgentEvent;
    use crate::permission::ScopeKey;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::{ctx_for, ctx_with_events};
    use crate::tool::net::MAX_BODY_BYTES;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn response(status: &str, headers: &[(&str, &str)], body: &str) -> String {
        let mut out = format!("HTTP/1.1 {status}\r\n");
        for (name, value) in headers {
            out.push_str(&format!("{name}: {value}\r\n"));
        }
        // Every answer closes its connection so the next request in a redirect
        // chain arrives as a new `accept` instead of being pipelined into a
        // socket the test has already finished with.
        out.push_str(&format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ));
        out
    }

    /// The requests a test server was sent, whole and unparsed.
    type Requests = Arc<Mutex<Vec<String>>>;

    /// A server that answers a fixed list of canned responses, one per request
    /// for a page, and keeps what it was asked.
    ///
    /// A real socket rather than a mocked client: a mock would only prove the
    /// mock redirects the way the test thinks it does — and it would say
    /// nothing about the headers that actually leave the process.
    ///
    /// `robots` is answered out of band and does not eat an entry of the list:
    /// every fetch asks for `/robots.txt` first now (§10), and threading that
    /// through the canned list would make every test about robots. `None` is
    /// the common case — a host with no such file, answering 404.
    async fn serve_robots(robots: Option<String>, responses: Vec<String>) -> (String, Requests) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let seen: Requests = Arc::new(Mutex::new(Vec::new()));
        let recorder = seen.clone();
        let missing = response("404 Not Found", &[], "");

        tokio::spawn(async move {
            let mut next = responses.into_iter();
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = [0u8; 4096];
                let read = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..read]).into_owned();
                recorder.lock().unwrap().push(request.clone());

                let body = if request.starts_with("GET /robots.txt") {
                    robots.clone().unwrap_or_else(|| missing.clone())
                } else {
                    match next.next() {
                        Some(body) => body,
                        None => return,
                    }
                };
                let _ = socket.write_all(body.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });

        (format!("http://{addr}"), seen)
    }

    /// The same for a host that keeps no `robots.txt`.
    async fn serve_seen(responses: Vec<String>) -> (String, Requests) {
        serve_robots(None, responses).await
    }

    /// And the same again, for the tests that only care about the answer.
    async fn serve(responses: Vec<String>) -> String {
        serve_seen(responses).await.0
    }

    fn html(body: &str) -> String {
        response(
            "200 OK",
            &[("Content-Type", "text/html; charset=utf-8")],
            body,
        )
    }

    fn robots(body: &str) -> String {
        response("200 OK", &[("Content-Type", "text/plain")], body)
    }

    /// Запрет из `robots.txt` — не сбой, а сработавшая граница: `Rejected`,
    /// как у редиректа на чужой хост. И страницу никто не трогает.
    #[tokio::test]
    async fn a_disallowed_path_is_refused_before_the_page_is_asked_for() {
        let (base, seen) = serve_robots(
            Some(robots("User-agent: *
Disallow: /private
")),
            vec![html("<p>секрет</p>")],
        )
        .await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = WebFetch
            .run(&ctx, json!({ "url": format!("{base}/private/page") }))
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::Rejected(_)), "{err:?}");
        assert!(err.to_string().contains("/private"), "{err}");

        let requests = seen.lock().unwrap();
        assert_eq!(requests.len(), 1, "за страницей ходить было нельзя");
        assert!(requests[0].contains("GET /robots.txt"), "{:?}", requests[0]);
    }

    /// Путь, которого запрет не касается, забирается как обычно.
    #[tokio::test]
    async fn an_allowed_path_goes_through_with_robots_in_place() {
        let (base, seen) = serve_robots(
            Some(robots("User-agent: *
Disallow: /private
")),
            vec![html("<p>Открыто.</p>")],
        )
        .await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let out = WebFetch
            .run(&ctx, json!({ "url": format!("{base}/public") }))
            .await
            .unwrap();

        assert!(out.text.contains("Открыто."));
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    /// Выключатель §10 выключает: запрет читается, но не применяется.
    #[tokio::test]
    async fn the_setting_turns_the_whole_check_off() {
        let (base, seen) = serve_seen(vec![html("<p>секрет</p>")]).await;
        let dir = TempDir::new();
        let (mut ctx, _ws) = ctx_for(&dir);
        ctx.respect_robots = false;

        let out = WebFetch
            .run(&ctx, json!({ "url": format!("{base}/private/page") }))
            .await
            .unwrap();

        assert!(out.text.contains("секрет"));
        // Ни одного лишнего запроса: выключено — значит, `robots.txt` даже
        // не спрашивают.
        let requests = seen.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].contains("GET /private/page"), "{:?}", requests[0]);
    }

    /// Не ответивший `robots.txt` разрешает, а не запрещает: человек только
    /// что сам разрешил этот домен, и упавший на секунду сервер не должен
    /// закрывать страницу молча.
    #[tokio::test]
    async fn an_unreachable_robots_file_allows_rather_than_blocks() {
        let (base, _seen) = serve_robots(
            Some(response("503 Service Unavailable", &[], "увы")),
            vec![html("<p>Страница.</p>")],
        )
        .await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let out = WebFetch
            .run(&ctx, json!({ "url": format!("{base}/page") }))
            .await
            .unwrap();

        assert!(out.text.contains("Страница."));
    }

    /// Файл читается один раз на хост: без кэша каждая страница стоила бы
    /// двух запросов.
    #[tokio::test]
    async fn robots_is_read_once_per_host() {
        let (base, seen) = serve_robots(
            Some(robots("User-agent: *
Disallow: /private
")),
            vec![html("<p>Раз.</p>"), html("<p>Два.</p>")],
        )
        .await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        for path in ["one", "two"] {
            WebFetch
                .run(&ctx, json!({ "url": format!("{base}/{path}") }))
                .await
                .unwrap();
        }

        let requests = seen.lock().unwrap();
        let asked = requests
            .iter()
            .filter(|r| r.contains("GET /robots.txt"))
            .count();
        assert_eq!(asked, 1, "{requests:?}");
    }

    /// The request goes out under Yaca's own name, not a browser's: what is
    /// knocking is visible in the site's logs, and a 403 earned honestly is
    /// better than a page fetched under a false name.
    #[tokio::test]
    async fn the_request_names_yaca_in_its_user_agent() {
        let (base, seen) = serve_seen(vec![html("<p>Текст.</p>")]).await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        WebFetch
            .run(&ctx, json!({ "url": format!("{base}/page") }))
            .await
            .unwrap();

        let request = seen.lock().unwrap().first().cloned().unwrap();
        let lowered = request.to_lowercase();
        assert!(
            lowered.contains("user-agent: yacatraveler/"),
            "нет заголовка или не то имя:
{request}"
        );
        // Не браузер: строка Mozilla открыла бы больше дверей, но соврав.
        assert!(!lowered.contains("mozilla"), "{request}");
    }

    #[tokio::test]
    async fn a_page_reaches_the_model_as_markdown_under_a_warning() {
        let base = serve(vec![html("<h1>Заголовок</h1><p>Текст.</p>")]).await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let out = WebFetch
            .run(&ctx, json!({ "url": format!("{base}/page") }))
            .await
            .unwrap();

        assert!(out.text.contains("# Заголовок"));
        assert!(out.text.contains(net::UNTRUSTED));
        assert!(out.text.contains("· 200 · text/html ·"));
        assert!(out.text.contains(&format!("{base}/page")));
        assert!(!out.meta.truncated);
    }

    #[tokio::test]
    async fn the_feed_gets_the_page_without_the_warning() {
        // §5.5.1 shows the human what was on the page. The line about
        // untrusted content is addressed to the model and would read as
        // gibberish in the feed.
        let base = serve(vec![html("<h1>Заголовок</h1>")]).await;
        let dir = TempDir::new();
        let (ctx, mut events) = ctx_with_events(&dir);

        WebFetch.run(&ctx, json!({ "url": base })).await.unwrap();

        let chunk = loop {
            match events.recv().await.unwrap() {
                AgentEvent::ToolCallProgress { chunk, .. } => break chunk,
                _ => continue,
            }
        };
        assert_eq!(chunk, "# Заголовок");
        assert!(ctx.progress.used(), "лента получит текст дважды");
    }

    #[tokio::test]
    async fn a_redirect_inside_the_host_is_followed() {
        let base = serve(vec![
            response("301 Moved Permanently", &[("Location", "/moved")], ""),
            html("<h1>Приехали</h1>"),
        ])
        .await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let out = WebFetch.run(&ctx, json!({ "url": base })).await.unwrap();

        assert!(out.text.contains("# Приехали"));
        // The final URL is what the model is told, not the one it asked for.
        assert!(out.text.contains("/moved"), "{}", out.text);
    }

    #[tokio::test]
    async fn a_redirect_to_another_host_is_rejected_with_the_new_address() {
        let base = serve(vec![response(
            "302 Found",
            &[("Location", "https://elsewhere.example/page")],
            "",
        )])
        .await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = WebFetch
            .run(&ctx, json!({ "url": base }))
            .await
            .unwrap_err();

        // Not a failure: the domain the human allowed held (§5.9).
        assert!(matches!(err, ToolError::Rejected(_)), "{err:?}");
        assert!(err.to_string().contains("https://elsewhere.example/page"));
    }

    #[tokio::test]
    async fn a_redirect_without_a_location_fails() {
        let base = serve(vec![response("302 Found", &[], "")]).await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = WebFetch
            .run(&ctx, json!({ "url": base }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("без Location"));
    }

    #[tokio::test]
    async fn an_error_status_is_a_failure_naming_the_host() {
        let base = serve(vec![response("404 Not Found", &[], "nope")]).await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = WebFetch
            .run(&ctx, json!({ "url": base }))
            .await
            .unwrap_err();
        assert!(err.to_string().starts_with("HTTP 404 · 127.0.0.1"), "{err}");
    }

    #[tokio::test]
    async fn a_body_over_the_ceiling_stops() {
        let huge = "a".repeat(MAX_BODY_BYTES + 1024);
        let base = serve(vec![response(
            "200 OK",
            &[("Content-Type", "text/plain")],
            &huge,
        )])
        .await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = WebFetch
            .run(&ctx, json!({ "url": base }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("больше 5 МиБ"), "{err}");
    }

    #[tokio::test]
    async fn only_http_and_https_get_as_far_as_a_socket() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        for bad in ["ftp://example.com/x", "file:///etc/passwd"] {
            let err = WebFetch.run(&ctx, json!({ "url": bad })).await.unwrap_err();
            assert!(
                err.to_string().contains("не поддерживается"),
                "{bad}: {err}"
            );
        }

        let err = WebFetch
            .run(&ctx, json!({ "url": "не адрес вовсе" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("плохой адрес"), "{err}");
    }

    #[test]
    fn the_question_names_the_host_and_offers_the_domain_first() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let input = json!({ "url": "https://docs.rs/htmd/latest/htmd/" });

        let need = WebFetch.permission(&ws, &input).unwrap();
        assert_eq!(need.kind, PermissionKind::FetchUrl);
        // §5.7: the domain in tier two, the full URL in tier three.
        assert_eq!(need.target, "docs.rs");
        assert_eq!(need.detail, "https://docs.rs/htmd/latest/htmd/");

        assert_eq!(
            need.scopes(dir.path()),
            vec![
                ScopeKey::Domain {
                    host: "docs.rs".into()
                },
                ScopeKey::Tool {
                    name: "WebFetch".into()
                },
            ]
        );
    }

    #[test]
    fn a_broken_url_asks_nothing_and_lets_the_tool_explain() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        assert!(WebFetch.permission(&ws, &json!({})).is_none());
        assert!(WebFetch
            .permission(&ws, &json!({ "url": "ftp://example.com" }))
            .is_none());
    }
}
