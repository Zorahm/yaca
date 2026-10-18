//! The seven tools of the browser, as the model sees them, and what each does.
//!
//! Few on purpose: every description rides along in every request, so a
//! small set that covers the work is cheaper than a tool per gesture.
//! `act` holds the gestures (`act.rs`).
//!
//! Every result opens with where the page is and the size it is laid out at:
//! the human may have clicked in the page since the last call, and the model
//! finds out here, before it acts on a page that is not there any more.

use std::sync::Arc;

use base64::Engine as _;
use serde_json::{json, Value};

use super::engine::{Area, Engine, Load, Step, LOAD_LIMIT};
use super::layout::{self, Rect, ViewportSource};
use super::Browser;

/// What a tool call returns: MCP content, and whether it failed.
pub struct Reply {
    pub content: Vec<Value>,
    pub error: bool,
}

impl Reply {
    fn text(text: String) -> Reply {
        Reply {
            content: vec![json!({ "type": "text", "text": text })],
            error: false,
        }
    }

    pub fn failed(text: String) -> Reply {
        Reply {
            error: true,
            ..Reply::text(text)
        }
    }
}

/// How much of a text answer reaches the model.
const TEXT_LIMIT: usize = 40_000;
const EVAL_LIMIT: usize = 20_000;
/// A full-page screenshot stops this many viewports down: past that the
/// image is shrunk until nothing in it can be read.
const FULL_PAGE_VIEWPORTS: f64 = 3.0;

pub const INSTRUCTIONS: &str = "This is the browser of the Yaca window: one page per chat, shown \
in the browser column next to the conversation. The human sees what you do there and may click \
in the page too, so look at the url at the top of each result before you act: if it is not the \
page you left, the human moved it.\n\
A usual loop: navigate, read_page to see what is there and get refs, act on refs, screenshot to \
check how it looks. read_page is for reading, screenshot for appearance. eval_js runs in the \
page; use it to pull out, filter or sort data.\n\
Every result gives the size the page is laid out at, which is also the size of a screenshot. \
With the browser column closed the page is laid out at 1280×800; viewport sets another size.\n\
Background subagents of this chat share the same page: two of you acting on it at once get in \
each other's way.";

/// The tools, for `tools/list`.
pub fn list() -> Value {
    let read_only = json!({ "readOnlyHint": true });
    let changes = json!({ "readOnlyHint": false });
    json!([
        {
            "name": "navigate",
            "description": "Open a URL in the browser, or go back, forward or reload. Waits for the page to load and returns its URL and title; read_page or screenshot show what is on it. A bare host:port such as localhost:5173 opens over http.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The address to open." },
                    "action": { "type": "string", "enum": ["back", "forward", "reload"], "description": "Instead of url: a step through history, or a reload." },
                },
            },
            "annotations": changes,
        },
        {
            "name": "screenshot",
            "description": "Take a screenshot of the page: the visible part by default, the whole document with full_page, or one element by ref or selector. The image is in CSS pixels, at the size the page is laid out at.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "full_page": { "type": "boolean", "description": "The whole document, not only the visible part. Very long pages are cut off." },
                    "ref": { "type": "string", "description": "An element's ref from read_page." },
                    "selector": { "type": "string", "description": "A CSS selector of the element." },
                },
            },
            "annotations": read_only,
        },
        {
            "name": "read_page",
            "description": "Read the page as text. mode \"snapshot\" (the default) lists headings, text and interactive elements, each element with a ref like e12 to use in act. mode \"text\" gives the readable content as Markdown. selector narrows either to a part of the page.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "enum": ["snapshot", "text"] },
                    "selector": { "type": "string", "description": "A CSS selector of the part to read." },
                    "limit": { "type": "integer", "minimum": 20, "maximum": 2000, "description": "Most lines of a snapshot; 400 by default." },
                },
            },
            "annotations": read_only,
        },
        {
            "name": "act",
            "description": "Do something on the page: click, double_click, right_click, hover, type, press, scroll, select, or wait. Aim at an element by ref from read_page (best), by CSS selector, or at x/y from a screenshot. Always say in element what you are acting on, in a few words such as \"Sign in button\": the human and the safety check read that, not the page. If the action makes the page navigate, the call waits for it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["click", "double_click", "right_click", "hover", "type", "press", "scroll", "select", "wait"] },
                    "element": { "type": "string", "description": "What you act on, in a few human words." },
                    "ref": { "type": "string" },
                    "selector": { "type": "string" },
                    "x": { "type": "number", "description": "Viewport x in CSS pixels, as in a screenshot." },
                    "y": { "type": "number" },
                    "text": { "type": "string", "description": "type: the text to enter. press: a key such as Enter, Escape, Control+a. wait: text to wait for on the page." },
                    "clear": { "type": "boolean", "description": "type: replace what the field holds (default true)." },
                    "submit": { "type": "boolean", "description": "type: press Enter afterwards." },
                    "values": { "type": "array", "items": { "type": "string" }, "description": "select: the options to choose, by value or by label." },
                    "dx": { "type": "number", "description": "scroll: pixels to the right." },
                    "dy": { "type": "number", "description": "scroll: pixels down (600 by default; negative goes up)." },
                    "timeout_ms": { "type": "integer", "minimum": 100, "maximum": 30000, "description": "wait: how long to wait, 5000 by default." },
                },
                "required": ["action", "element"],
            },
            "annotations": changes,
        },
        {
            "name": "eval_js",
            "description": "Run JavaScript in the page, with the page's own globals, and return the value of the last expression as JSON. Promises are awaited and top-level await works. For pulling data out of the page, filtering and sorting it.",
            "inputSchema": {
                "type": "object",
                "properties": { "code": { "type": "string" } },
                "required": ["code"],
            },
            "annotations": changes,
        },
        {
            "name": "logs",
            "description": "Read the console messages or the network requests of the page since it loaded.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["console", "network"] },
                    "errors_only": { "type": "boolean", "description": "console: errors, warnings and exceptions. network: failed requests and HTTP errors." },
                    "filter": { "type": "string", "description": "Only entries containing this text." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500, "description": "The newest this many; 100 by default." },
                },
                "required": ["kind"],
            },
            "annotations": read_only,
        },
        {
            "name": "viewport",
            "description": "Set the size the page is laid out at: a preset (desktop 1280×800, tablet 820×1180, mobile 390×844, the last two with mobile emulation), or width and height in CSS pixels. Preset \"panel\" goes back to following the browser column. A page larger than the column is shown to the human scaled down.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "preset": { "type": "string", "enum": ["desktop", "tablet", "mobile", "panel"] },
                    "width": { "type": "number" },
                    "height": { "type": "number" },
                },
            },
            "annotations": read_only,
        },
    ])
}

/// Runs one tool.
pub async fn call(browser: &Arc<Browser>, name: &str, args: &Value) -> Reply {
    let result = match name {
        "navigate" => navigate(browser, args).await,
        "screenshot" => screenshot(browser, args).await,
        "read_page" => read_page(browser, args).await,
        "act" => super::act::act(browser, args).await,
        "eval_js" => eval_js(browser, args).await,
        "logs" => logs(browser, args).await,
        "viewport" => viewport(browser, args).await,
        other => Err(format!("there is no tool {other}")),
    };
    match result {
        Ok(reply) => reply,
        Err(why) => Reply::failed(format!("{}{why}", status(browser))),
    }
}

/// The lines every result begins with.
pub fn status(browser: &Browser) -> String {
    let mut out = String::new();
    if let Some(engine) = browser.live() {
        let location = engine.location();
        out.push_str(&format!("url: {}\n", location.url));
        if !location.title.is_empty() {
            out.push_str(&format!("title: {}\n", location.title));
        }
        if location.loading {
            out.push_str("loading: yes\n");
        }
        for note in engine.logs().take_notes() {
            out.push_str(&format!("note: {note}\n"));
        }
    }
    let plan = browser.plan();
    let why = match plan.source {
        ViewportSource::Panel => "the browser column, as the human sees it",
        ViewportSource::Default => "the default: the browser column is closed",
        ViewportSource::Model => "set with viewport",
    };
    out.push_str(&format!(
        "viewport: {}×{} ({why})\n",
        plan.viewport.width, plan.viewport.height
    ));
    out
}

/// The engine of a page that is open, or the error that says to open one.
pub fn open_page(browser: &Browser) -> Result<Arc<dyn Engine>, String> {
    browser
        .live()
        .ok_or_else(|| "no page is open yet: open one with navigate".to_string())
}

fn string<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty())
}

async fn navigate(browser: &Arc<Browser>, args: &Value) -> Result<Reply, String> {
    let engine = browser.engine().await?;
    let load = match (string(args, "url"), string(args, "action")) {
        (Some(url), None) => engine.navigate(&super::normalize(url)?, LOAD_LIMIT).await?,
        (None, Some(action)) => {
            let step = match action {
                "back" => Step::Back,
                "forward" => Step::Forward,
                "reload" => Step::Reload,
                other => return Err(format!("unknown action {other}")),
            };
            engine.step(step, LOAD_LIMIT).await?
        }
        _ => return Err("give either url or action".into()),
    };
    let tail = match load {
        Load::Pending => "The page is still loading.",
        _ => "Loaded.",
    };
    Ok(Reply::text(format!("{}{tail}", status(browser))))
}

/// `{"ref": …}` or `{"selector": …}` from the arguments, as page.js reads it.
pub fn target(args: &Value) -> Option<Value> {
    if let Some(r) = string(args, "ref") {
        return Some(json!({ "ref": r }));
    }
    string(args, "selector").map(|s| json!({ "selector": s }))
}

async fn screenshot(browser: &Arc<Browser>, args: &Value) -> Result<Reply, String> {
    let engine = open_page(browser)?;
    let plan = browser.plan();
    let (area, of) = match (target(args), args["full_page"].as_bool() == Some(true)) {
        (Some(target), _) => {
            let found = engine.helper(&format!("box({target})")).await?;
            let rect = Rect {
                x: found["x"].as_f64().unwrap_or(0.0),
                y: found["y"].as_f64().unwrap_or(0.0),
                width: found["width"].as_f64().unwrap_or(0.0),
                height: found["height"].as_f64().unwrap_or(0.0),
            };
            (Area::Box(rect), found["label"].as_str().unwrap_or("the element").to_string())
        }
        (None, true) => (
            Area::FullPage {
                max_height: plan.viewport.height * FULL_PAGE_VIEWPORTS,
            },
            "the whole page".to_string(),
        ),
        (None, false) => (Area::Viewport, "the visible part of the page".to_string()),
    };
    let shot = engine.screenshot(area).await?;
    let mut text = format!("{}screenshot of {of}: {}×{}", status(browser), shot.width, shot.height);
    if shot.cut {
        text.push_str(&format!(
            " (cut off {} viewports down; scroll and take another for the rest)",
            FULL_PAGE_VIEWPORTS
        ));
    }
    Ok(Reply {
        content: vec![
            json!({ "type": "text", "text": text }),
            json!({
                "type": "image",
                "data": base64::engine::general_purpose::STANDARD.encode(&shot.png),
                "mimeType": "image/png",
            }),
        ],
        error: false,
    })
}

async fn read_page(browser: &Arc<Browser>, args: &Value) -> Result<Reply, String> {
    let engine = open_page(browser)?;
    let selector = string(args, "selector");
    let text = match string(args, "mode").unwrap_or("snapshot") {
        "text" => {
            let html = engine.helper(&format!("html({})", json!({ "selector": selector }))).await?;
            let markdown = htmd::convert(html.as_str().unwrap_or_default())
                .map_err(|e| format!("the page did not convert to text: {e}"))?;
            cut(markdown.trim(), TEXT_LIMIT)
        }
        "snapshot" => {
            let limit = args["limit"].as_u64().unwrap_or(400).clamp(20, 2000);
            let snapshot = engine
                .helper(&format!("snapshot({})", json!({ "selector": selector, "limit": limit })))
                .await?;
            let mut text = snapshot["text"].as_str().unwrap_or_default().to_string();
            let more = snapshot["more"].as_u64().unwrap_or(0);
            if more > 0 {
                text.push_str(&format!(
                    "\n… {more} more lines: raise limit, or narrow with selector"
                ));
            }
            if text.is_empty() {
                "(nothing visible)".to_string()
            } else {
                text
            }
        }
        other => return Err(format!("unknown mode {other}")),
    };
    Ok(Reply::text(format!("{}\n{text}", status(browser))))
}

async fn eval_js(browser: &Arc<Browser>, args: &Value) -> Result<Reply, String> {
    let engine = open_page(browser)?;
    let code = string(args, "code").ok_or("code is empty")?;
    let since = engine.loads();
    let evaluated = engine.evaluate(code).await?;
    let shown = match &evaluated.value {
        Some(Value::String(text)) => text.clone(),
        Some(value) => serde_json::to_string_pretty(value).unwrap_or_default(),
        None => evaluated.description.clone(),
    };
    let navigated = engine.settle(since, LOAD_LIMIT).await? != Load::Nothing;
    let mut text = format!("{}\n{}", status(browser), cut(&shown, EVAL_LIMIT));
    if navigated {
        text.push_str("\n(the page navigated)");
    }
    Ok(Reply::text(text))
}

async fn logs(browser: &Arc<Browser>, args: &Value) -> Result<Reply, String> {
    let engine = open_page(browser)?;
    let errors = args["errors_only"].as_bool() == Some(true);
    let filter = string(args, "filter").map(str::to_lowercase);
    let keep = |line: &str| filter.as_ref().is_none_or(|f| line.to_lowercase().contains(f));
    let limit = args["limit"].as_u64().unwrap_or(100).clamp(1, 500) as usize;
    let lines: Vec<String> = match string(args, "kind") {
        Some("console") => engine
            .logs()
            .console_entries()
            .into_iter()
            .filter(|e| !errors || matches!(e.level.as_str(), "error" | "warning" | "assert" | "exception"))
            .map(|e| format!("[{}] {}", e.level, cut(&e.text, 2_000)))
            .filter(|line| keep(line))
            .collect(),
        Some("network") => engine
            .logs()
            .network_entries()
            .into_iter()
            .filter(|e| !errors || e.is_error())
            .map(|e| {
                let outcome = match (&e.failed, e.status) {
                    (Some(why), _) => format!("failed {why}"),
                    (None, Some(status)) => status.to_string(),
                    (None, None) => "pending".to_string(),
                };
                format!("{} {outcome} {} ({})", e.method, e.url, e.kind)
            })
            .filter(|line| keep(line))
            .collect(),
        _ => return Err("kind must be console or network".into()),
    };
    let skipped = lines.len().saturating_sub(limit);
    let mut text = status(browser);
    if lines.is_empty() {
        text.push_str("(nothing logged)");
    } else {
        if skipped > 0 {
            text.push_str(&format!("({skipped} older entries left out)\n"));
        }
        text.push_str(&lines[skipped..].join("\n"));
    }
    Ok(Reply::text(text))
}

async fn viewport(browser: &Arc<Browser>, args: &Value) -> Result<Reply, String> {
    let forced = layout::forced(
        string(args, "preset"),
        args["width"].as_f64(),
        args["height"].as_f64(),
    )?;
    browser.force(forced).await?;
    Ok(Reply::text(status(browser)))
}

/// `text` cut to `limit` characters, saying so.
pub fn cut(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        None => text.to_string(),
        Some((at, _)) => format!(
            "{}\n… cut at {limit} characters; narrow the request for the rest",
            &text[..at]
        ),
    }
}
