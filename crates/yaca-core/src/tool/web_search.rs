//! Searching the web through Tavily (the `Reach` row of the §8 matrix).
//!
//! Only [`WebFetch`](super::web_fetch::WebFetch) existed before, so the model
//! guessed URLs instead of looking them up. A search is the other half of the
//! same pair: one question, asked of one fixed host — `api.tavily.com` — so
//! the permission is always about that one domain, whatever the query is.
//!
//! The call goes out with the key from the keychain (§10) in a bearer header,
//! and that key must never surface anywhere a human or a log can read. It is
//! put into the header alone, a redirect is refused rather than followed (a
//! bearer token handed to another host is how a key leaks), and no error text
//! carries it. Every failure here returns as a [`ToolError`], which the turn
//! turns into a `tool_result` — a failing search must not kill the turn.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::net;
use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::{PermissionKind, PermissionNeed};
use crate::workspace::Workspace;

/// Host and base of the Tavily search endpoint. `POST {endpoint}/search` with
/// the key as `Authorization: Bearer <key>` — their API reference fixes both,
/// and the free tier is 1000 credits a month, which covers an owner's session.
const TAVILY_HOST: &str = "api.tavily.com";
const TAVILY_ENDPOINT: &str = "https://api.tavily.com";

/// The API allows `max_results` up to twenty, but every result costs tokens
/// in every later turn of the history. Ten is enough to find the page the
/// model will then fetch and read in full with WebFetch.
const MAX_RESULTS: u64 = 10;
const DEFAULT_RESULTS: u64 = 5;

/// Snippets are how the model decides which result to fetch; a longer one is
/// the whole page wearing a smaller name, and that page has a URL of its own.
const MAX_SNIPPET_CHARS: usize = 600;

/// Titles from the wild are pages too; one woven over several hundred
/// characters fills a feed row for nothing.
const MAX_TITLE_CHARS: usize = 200;

/// Текст о недостающем ключе читает человек в ленте — §5.5, а не модель.
const NO_KEY: &str = "Ключ поиска (Tavily) не задан. Сохраните его командой \
                      `yaca key set --provider tavily` или в переменной \
                      окружения `TAVILY_API_KEY`.";

/// Where the key comes from. A function rather than a fixed value: the key is
/// looked up fresh on every call, so one stored mid-session is picked up
/// without a restart, and tests swap one in that answers with a fixture key
/// or always `None` — a machine with a real key in its keychain must not
/// change what a test running on it observes, and it must never be one.
type KeyLookup = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct WebSearch {
    /// The base the request is POSTed to. A field rather than the constant
    /// above so the tests can answer with a local socket, like web fetch's
    /// do; this constructor is the only place that sets the default.
    endpoint: String,
    lookup: KeyLookup,
}

impl Default for WebSearch {
    fn default() -> Self {
        WebSearch {
            endpoint: TAVILY_ENDPOINT.to_string(),
            lookup: Arc::new(crate::secrets::tavily_key),
        }
    }
}

/// The query as it was asked, whitespace gone, blanks unnamed.
///
/// Shared by [`run`](Tool::run) and [`permission`](Tool::permission): both
/// must agree on what a query is, or a call that the run refuses would be a
/// call the permission asked about.
fn asked(input: &Value) -> Option<&str> {
    input["query"]
        .as_str()
        .map(str::trim)
        .filter(|q| !q.is_empty())
}

fn query(input: &Value) -> Result<String, ToolError> {
    asked(input)
        .ok_or_else(|| ToolError::failed("не задан запрос поиска"))
        .map(str::to_owned)
}

fn max_results(input: &Value) -> u64 {
    // Clamped rather than refused: a number slightly off is a preference the
    // model can afford to have guessed, and a stray 50 would only bloat the
    // answer. Refusal is for things that cannot be guessed.
    let asked = input["max_results"].as_u64().unwrap_or(DEFAULT_RESULTS);
    asked.clamp(1, MAX_RESULTS)
}

/// The list the model reads. English, like everything it is sent.
///
/// `raw_content` is not asked for: Tavily can return each page's full text
/// in the same call, but ten whole pages at once would fill the context for
/// results the model mostly does not read. Reading a chosen page is
/// [`WebFetch`](super::web_fetch::WebFetch)'s job.
fn render(query: &str, results: &[Value]) -> String {
    if results.is_empty() {
        return format!("No results for \"{query}\".");
    }
    entries(results)
}

/// What the expanded row shows (§5.5): Russian wording, Russian because the
/// human reads it. The findings themselves are content, like a fetched page
/// is, and go as they are.
fn feed_text(results: &[Value]) -> String {
    if results.is_empty() {
        return "По запросу ничего не найдено.".into();
    }
    entries(results)
}

fn entries(results: &[Value]) -> String {
    let mut out = String::new();
    for (i, r) in results.iter().enumerate() {
        let title = r["title"].as_str().unwrap_or("(no title)");
        let title: String = title.chars().take(MAX_TITLE_CHARS).collect();
        let url = r["url"].as_str().unwrap_or_default();
        let content = r["content"].as_str().unwrap_or_default();
        let snippet: String = content.trim().chars().take(MAX_SNIPPET_CHARS).collect();
        out.push_str(&format!("{}. {title}\n   {url}\n   {snippet}\n", i + 1));
    }
    out
}

#[async_trait]
impl Tool for WebSearch {
    fn name(&self) -> &'static str {
        "WebSearch"
    }

    fn description(&self) -> &'static str {
        "Search the web with the Tavily API and read short snippets of the \
         results. Use for current events and anything you need a URL for \
         first; fetch a result's page with WebFetch to read it in full."
    }

    fn properties(&self) -> Value {
        json!({
            "query": {
                "type": "string",
                "description": "The search query. A web-style question or \
                                keywords, not a URL.",
            },
            "max_results": {
                "type": "integer",
                "description": "Maximum number of results, 1 to 10. \
                                Optional; defaults to 5.",
                "minimum": 1,
                "maximum": 10,
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["query"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    fn intents(&self) -> &'static [Intent] {
        // Looking something up in the world is half of what planning is, and
        // a search changes nothing in the project.
        &[Intent::Plan, Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["query"].as_str().map(str::to_owned)
    }

    fn permission(&self, _ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        // A broken query asks nothing and lets the tool explain, like a
        // broken URL does for web fetch.
        Some(
            PermissionNeed::new(PermissionKind::FetchUrl, self.name(), TAVILY_HOST)
                .detail(asked(input)?.to_string())
                .domain(TAVILY_HOST),
        )
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let query = query(&input)?;
        let count = max_results(&input);

        ctx.check_cancelled()?;
        let key = (self.lookup)().filter(|k| !k.trim().is_empty())
            .ok_or_else(|| ToolError::failed(NO_KEY))?;

        let url = format!("{}/search", self.endpoint.trim_end_matches('/'));
        let response = net::client()
            .post(url)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {key}"))
            .json(&json!({ "query": query, "max_results": count }))
            .send()
            .await
            .map_err(|e| ToolError::failed(format!("не удалось выполнить поиск: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            // The body is what the server said about the refusal; the key is
            // never quoted back — it went out as a header, not as content,
            // and nothing from the request text is echoed.
            let body = net::read_body(ctx, response).await?;
            let text = String::from_utf8_lossy(&body);
            return Err(ToolError::failed_with(
                format!("HTTP {} · Tavily отказала в поиске", status.as_u16()),
                refusal(&text),
            ));
        }

        let body = net::read_body(ctx, response).await?;
        let parsed: Value = serde_json::from_slice(&body)
            .map_err(|e| ToolError::failed(format!("Tavily прислала не JSON: {e}")))?;

        let results: Vec<Value> = parsed["results"].as_array().cloned().unwrap_or_default();

        // The feed and the model see the same findings, but the wording
        // around them is not the same copy: the wording is Russian for the
        // human (§5.5) and English for the model. The untrusted-data
        // sentence is addressed to the model only — in the feed it would be
        // noise.
        let feed = feed_text(&results);
        ctx.progress.push(format!("{}\n", feed.trim_end()));
        let list = render(&query, &results);

        let untrusted = net::UNTRUSTED;
        let text = format!("{untrusted}\n\n{list}");
        Ok(ToolOutput::new(text).lines(list.lines().count() as u32))
    }
}

/// The server's own words about a refusal, at most.
///
/// Tavily puts its message in `{"detail":{"error":…}}`; anything else is
/// cut to a line, so a refusal names a cause and not the whole body a
/// server may have filled with more.
fn refusal(text: &str) -> String {
    if let Some(cause) = serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| v.pointer("/detail/error")?.as_str().map(str::to_owned))
    {
        return cause;
    }
    let raw = text.trim();
    if raw.is_empty() {
        // An empty refusal line is not a thing for the feed to show.
        "причин в ответе не прислано".into()
    } else {
        raw.chars().take(400).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::AgentEvent;
    use crate::permission::ScopeKey;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::{ctx_for, ctx_with_events};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    /// The fixture key. Shape of a real one, value a lie: a test must work on
    /// a machine where the keychain holds a live key and must not, in any
    /// event a human can read, be the key.
    const KEY: &str = "tvly-fixture-nothing-real";

    fn with_key() -> KeyLookup {
        Arc::new(|| Some(KEY.to_string()))
    }

    fn without_key() -> KeyLookup {
        Arc::new(|| None)
    }

    fn at(addr: &str, lookup: KeyLookup) -> WebSearch {
        WebSearch {
            endpoint: addr.to_string(),
            lookup,
        }
    }

    fn response(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn search_ok(results: &str) -> String {
        response("200 OK", &format!(r#"{{"query":"x","results":{results}}}"#))
    }

    /// A server that answers one request and hands back whatever it read.
    ///
    /// A real socket rather than a mocked client: a mock would only prove the
    /// mock sends the header the test thinks it does.
    async fn serve(body: String) -> (String, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut buf = [0u8; 16 * 1024];
            let _ = socket.read(&mut buf).await;
            let _ = tx.send(String::from_utf8_lossy(&buf).to_string());
            let _ = socket.write_all(body.as_bytes()).await;
            let _ = socket.shutdown().await;
        });

        (format!("http://{addr}"), rx)
    }

    fn events_text(rx: &mut tokio::sync::broadcast::Receiver<AgentEvent>) -> String {
        let mut seen = String::new();
        while let Ok(event) = rx.try_recv() {
            seen.push_str(&format!("{event:?}\n"));
        }
        seen
    }

    #[tokio::test]
    async fn the_key_goes_out_as_a_bearer_header_and_results_reach_the_model() {
        let (endpoint, rx) = serve(search_ok(
            r#"[{"title":"Ответ","url":"https://example.ru/i","content":"Текст.","score":0.9}]"#,
        ))
        .await;
        let dir = TempDir::new();
        let (ctx, mut events) = ctx_with_events(&dir);

        let out = at(&endpoint, with_key())
            .run(&ctx, json!({ "query": "Обои для консоли" }))
            .await
            .unwrap();

        let request = rx.await.unwrap();
        let auth = request
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
            .unwrap_or_default();
        // The whole key in the header, and nowhere else in the request line.
        assert_eq!(auth.to_ascii_lowercase(), format!("authorization: bearer {KEY}"), "{request}");
        assert!(request.contains("Обои для консоли"), "{request}");
        assert!(request.contains(r#""max_results":5"#), "{request}");

        assert!(out.text.contains("Ответ"));
        assert!(out.text.contains("https://example.ru/i"));
        assert!(out.text.contains(net::UNTRUSTED));

        // The key is a request header, not feed content: no event the human
        // or a log sees may carry it.
        let seen = events_text(&mut events);
        assert!(!seen.contains(KEY), "ключ утёк в события: {seen}");
        assert!(seen.contains("Ответ"), "{seen}");
    }

    #[tokio::test]
    async fn an_empty_result_set_is_an_answer_not_a_failure() {
        let (endpoint, _rx) = serve(search_ok("[]")).await;
        let dir = TempDir::new();
        let (ctx, mut events) = ctx_with_events(&dir);

        let out = at(&endpoint, with_key())
            .run(&ctx, json!({ "query": "nothing" }))
            .await
            .unwrap();

        // The model gets the sentence in its language.
        assert!(out.text.contains("No results for \"nothing\"."), "{}", out.text);

        // The feed gets its own wording: Russian, and not the model's copy.
        let seen = events_text(&mut events);
        assert!(seen.contains("По запросу ничего не найдено"), "{seen}");
        assert!(!seen.contains("No results for"), "{seen}");
    }

    #[tokio::test]
    async fn a_refusal_names_the_server_s_cause_and_not_the_key() {
        let (endpoint, _rx) = serve(response(
            "401 Unauthorized",
            r#"{"detail":{"error":"Unauthorized: missing or invalid API key."}}"#,
        ))
        .await;
        let dir = TempDir::new();
        let (ctx, mut events) = ctx_with_events(&dir);

        let err = at(&endpoint, with_key())
            .run(&ctx, json!({ "query": "x" }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("HTTP 401"), "{err}");
        // The server's cause rides in the detail the model gets: to the feed
        // it is one line, to the model the fix.
        assert!(err.for_model().contains("missing or invalid"), "{err}");
        // The key never comes back to the model or the feed: it went out as a
        // header and no error path echoes the request.
        assert!(!err.for_model().contains(KEY), "{err}");
        let seen = events_text(&mut events);
        assert!(!seen.contains(KEY), "ключ утёк в события: {seen}");

        // The turn is expected to go on: a Failed tool error comes back as a
        // tool_result, not as the end of the turn.
        assert!(matches!(err, ToolError::Failed { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn a_dead_endpoint_is_a_failure_the_turn_survives() {
        // An address with no listener: connection refused is the ordinary
        // network failure, not a panic in the session.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dead = listener.local_addr().unwrap();
        drop(listener);

        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = at(&format!("http://{dead}"), with_key())
            .run(&ctx, json!({ "query": "x" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("не удалось выполнить поиск"), "{err}");
    }

    #[tokio::test]
    async fn a_missing_key_tells_where_to_put_it() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        // The lookup always finds nothing, whatever a keychain on the test
        // machine holds, so the wording is what decides the test — not the
        // environment it ran in.
        let err = at(TAVILY_ENDPOINT, without_key())
            .run(&ctx, json!({ "query": "x" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("yaca key set --provider tavily"), "{err}");
        assert!(err.to_string().contains("TAVILY_API_KEY"), "{err}");
    }

    #[tokio::test]
    async fn a_count_off_the_scale_is_clamped_not_shouted_at() {
        let (endpoint, rx) = serve(search_ok("[]")).await;
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        at(&endpoint, with_key())
            .run(&ctx, json!({ "query": "x", "max_results": 50 }))
            .await
            .unwrap();
        let request = rx.await.unwrap();
        assert!(request.contains(r#""max_results":10"#), "{request}");
    }

    #[tokio::test]
    async fn an_empty_query_fails_inside_the_tool() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        for bad in [json!({}), json!({ "query": "   " })] {
            let err = at(TAVILY_ENDPOINT, with_key())
                .run(&ctx, bad)
                .await
                .unwrap_err();
            assert!(err.to_string().contains("не задан запрос"), "{err}");
        }
    }

    #[test]
    fn the_question_names_the_domain_once_for_all_queries() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let input = json!({ "query": "rust async channels" });

        let need = WebSearch::default().permission(&ws, &input).unwrap();
        assert_eq!(need.kind, PermissionKind::FetchUrl);
        assert_eq!(need.target, "api.tavily.com");
        assert_eq!(need.detail, "rust async channels");

        assert_eq!(
            need.scopes(dir.path()),
            vec![
                ScopeKey::Domain {
                    host: "api.tavily.com".into()
                },
                ScopeKey::Tool {
                    name: "WebSearch".into()
                },
            ]
        );
    }

    #[test]
    fn a_broken_query_asks_nothing_and_lets_the_tool_explain() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        assert!(WebSearch::default().permission(&ws, &json!({})).is_none());
        assert!(WebSearch::default()
            .permission(&ws, &json!({ "query": "" }))
            .is_none());
    }
}
