//! The engine on Windows: a child WebView2 of the main window, driven through
//! the DevTools Protocol.
//!
//! WebView2 is Chromium, so everything the tools need is there without a
//! browser of our own: screenshots of a page that is not on screen, scripts
//! with their results, trusted input events, the console and the network.
//!
//! The page lives in a profile of its own (`<config dir>/browser`), shared by
//! the browsers of all chats and by nobody else: the window's own storage and
//! the sites' cookies never meet.

mod cdp;
mod events;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use serde_json::{json, Value};
use tauri::webview::{DownloadEvent, NewWindowResponse, WebviewBuilder};
use tauri::{LogicalPosition, LogicalSize, Webview, WebviewUrl, Window};
use tokio::sync::{mpsc, watch};

use self::cdp::Cdp;
use super::engine::{Area, Button, Engine, Evaluated, Load, Location, Result, Shot, Step};
use super::keys::Press;
use super::layout::{Emulation, Rect};
use super::logs::Logs;

/// The page's helpers (`page.js`), loaded into an isolated world per document.
const PAGE_JS: &str = include_str!("../page.js");

/// The events the engine follows.
const EVENTS: &[&str] = &[
    "Page.frameNavigated",
    "Page.navigatedWithinDocument",
    "Page.frameStartedLoading",
    "Page.frameStoppedLoading",
    "Page.javascriptDialogOpening",
    "Runtime.executionContextsCleared",
    "Runtime.consoleAPICalled",
    "Runtime.exceptionThrown",
    "Network.requestWillBeSent",
    "Network.responseReceived",
    "Network.loadingFailed",
];

/// Chromium's switches for a page that has to keep drawing while it is out
/// of sight: no throttled timers, no backgrounded renderer, and no occlusion
/// tracking, which would take a webview past the window's edge for hidden.
/// The first three features are the ones wry turns off by default; passing
/// our own arguments replaces its, so they are repeated.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion \
     --disable-background-timer-throttling --disable-renderer-backgrounding \
     --disable-backgrounding-occluded-windows";

/// How long a navigation has to begin after a click before the click is
/// taken not to have navigated.
const NAVIGATION_GRACE: Duration = Duration::from_millis(400);

/// The teal of the interface, for the ring around what the model acts on.
const RING: (u8, u8, u8) = (35, 128, 126);

pub struct WebView2 {
    webview: Webview,
    cdp: Cdp,
    shared: Arc<Shared>,
}

/// What the event task and the engine both see.
pub(crate) struct Shared {
    pub location: Mutex<Location>,
    pub logs: Logs,
    pub main_frame: Mutex<String>,
    /// The helpers' isolated world in the current document, once made.
    pub world: Mutex<Option<i64>>,
    pub started: AtomicU64,
    pub stopped: watch::Sender<u64>,
    /// Told whenever the location changes, so the column can follow.
    pub changed: Box<dyn Fn() + Send + Sync>,
}

impl WebView2 {
    /// Makes the webview, as a child of `window` at `bounds`.
    pub async fn open(
        window: Window,
        label: String,
        profile: PathBuf,
        bounds: Rect,
        changed: Box<dyn Fn() + Send + Sync>,
    ) -> Result<Arc<WebView2>> {
        let (tx, rx) = mpsc::unbounded_channel::<cdp::Event>();
        let blank = "about:blank".parse().map_err(|e| format!("{e}"))?;
        let opens = tx.clone();
        let downloads = tx.clone();
        let titles = tx.clone();
        let builder = WebviewBuilder::new(label, WebviewUrl::External(blank))
            .data_directory(profile)
            .additional_browser_args(BROWSER_ARGS)
            .devtools(true)
            .focused(false)
            .on_navigation(super::allowed)
            .on_new_window(move |url, _| {
                // One page per chat: a link to a new window opens here.
                let _ = opens.send(("yaca.open", json!({ "url": url.as_str() })));
                NewWindowResponse::Deny
            })
            .on_download(move |_, event| {
                if let DownloadEvent::Requested { url, .. } = event {
                    let _ = downloads.send(("yaca.download", json!({ "url": url.as_str() })));
                }
                false
            })
            .on_document_title_changed(move |_, title| {
                let _ = titles.send(("yaca.title", json!({ "title": title })));
            });

        // `add_child` waits for the main thread, so it must not run on it.
        let webview = tokio::task::spawn_blocking(move || {
            window.add_child(
                builder,
                LogicalPosition::new(bounds.x, bounds.y),
                LogicalSize::new(bounds.width, bounds.height),
            )
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("the browser could not be opened: {e}"))?;

        let cdp = Cdp::new(webview.clone());
        cdp.subscribe(EVENTS, tx)?;
        let shared = Arc::new(Shared {
            location: Mutex::new(Location::default()),
            logs: Logs::default(),
            main_frame: Mutex::new(String::new()),
            world: Mutex::new(None),
            started: AtomicU64::new(0),
            stopped: watch::channel(0).0,
            changed,
        });
        tokio::spawn(events::follow(Arc::downgrade(&shared), cdp.clone(), rx));
        for domain in ["Page.enable", "Runtime.enable", "Network.enable", "DOM.enable", "Overlay.enable"] {
            cdp.call(domain, json!({})).await?;
        }
        let tree = cdp.call("Page.getFrameTree", json!({})).await?;
        if let Some(id) = tree["frameTree"]["frame"]["id"].as_str() {
            *shared.main_frame.lock().unwrap() = id.to_string();
        }
        Ok(Arc::new(WebView2 { webview, cdp, shared }))
    }

    async fn world(&self) -> Result<i64> {
        if let Some(id) = *self.shared.world.lock().unwrap() {
            return Ok(id);
        }
        let frame = self.shared.main_frame.lock().unwrap().clone();
        let made = self
            .cdp
            .call(
                "Page.createIsolatedWorld",
                json!({ "frameId": frame, "worldName": "yaca", "grantUniveralAccess": false }),
            )
            .await?;
        let id = made["executionContextId"]
            .as_i64()
            .ok_or("the page has no room for the browser's helpers")?;
        let loaded = self
            .cdp
            .call("Runtime.evaluate", json!({ "expression": PAGE_JS, "contextId": id }))
            .await?;
        thrown(&loaded)?;
        *self.shared.world.lock().unwrap() = Some(id);
        Ok(id)
    }

    async fn mouse(&self, kind: &str, x: f64, y: f64, extra: Value) -> Result<()> {
        let mut params = json!({ "type": kind, "x": x, "y": y });
        if let (Some(params), Some(extra)) = (params.as_object_mut(), extra.as_object()) {
            params.extend(extra.clone());
        }
        self.cdp.call("Input.dispatchMouseEvent", params).await.map(drop)
    }

    async fn wait_loaded(&self, since: u64, limit: Duration, grace: Duration) -> Load {
        let deadline = tokio::time::Instant::now() + limit;
        let begun = tokio::time::Instant::now() + grace.min(limit);
        while self.shared.started.load(Ordering::SeqCst) <= since {
            if tokio::time::Instant::now() >= begun {
                return Load::Nothing;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
        let mut stopped = self.shared.stopped.subscribe();
        let waited = tokio::time::timeout_at(deadline, async {
            loop {
                if *stopped.borrow_and_update() >= self.shared.started.load(Ordering::SeqCst) {
                    return;
                }
                if stopped.changed().await.is_err() {
                    return;
                }
            }
        })
        .await;
        if waited.is_ok() {
            Load::Done
        } else {
            Load::Pending
        }
    }
}

/// The script's exception, if it threw one.
fn thrown(answer: &Value) -> Result<()> {
    let Some(details) = answer.get("exceptionDetails") else {
        return Ok(());
    };
    let text = details["exception"]["description"]
        .as_str()
        .or_else(|| details["text"].as_str())
        .unwrap_or("the script failed");
    // The first line is the message; the rest is a stack into our own code.
    let first = text.lines().next().unwrap_or(text);
    Err(first.strip_prefix("Error: ").unwrap_or(first).to_string())
}

fn png_size(png: &[u8]) -> (u32, u32) {
    let read = |at: usize| {
        png.get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .unwrap_or(0)
    };
    // The IHDR chunk comes first, right after the eight-byte signature.
    (read(16), read(20))
}

#[async_trait]
impl Engine for WebView2 {
    async fn navigate(&self, url: &str, limit: Duration) -> Result<Load> {
        let since = self.loads();
        let answer = self.cdp.call("Page.navigate", json!({ "url": url })).await?;
        if let Some(why) = answer["errorText"].as_str().filter(|text| !text.is_empty()) {
            return Err(format!("{url} did not open: {why}"));
        }
        Ok(match self.wait_loaded(since, limit, NAVIGATION_GRACE).await {
            // A jump within the same document loads nothing and is done.
            Load::Nothing => Load::Done,
            other => other,
        })
    }

    async fn step(&self, step: Step, limit: Duration) -> Result<Load> {
        let since = self.loads();
        match step {
            Step::Back | Step::Forward => {
                let history = self.cdp.call("Page.getNavigationHistory", json!({})).await?;
                let at = history["currentIndex"].as_i64().unwrap_or(0);
                let to = if step == Step::Back { at - 1 } else { at + 1 };
                let entry = usize::try_from(to)
                    .ok()
                    .and_then(|to| history["entries"].get(to))
                    .and_then(|entry| entry["id"].as_i64())
                    .ok_or(if step == Step::Back {
                        "there is no page to go back to"
                    } else {
                        "there is no page to go forward to"
                    })?;
                self.cdp
                    .call("Page.navigateToHistoryEntry", json!({ "entryId": entry }))
                    .await?;
            }
            Step::Reload => {
                self.cdp.call("Page.reload", json!({})).await?;
            }
            Step::Stop => {
                self.cdp.call("Page.stopLoading", json!({})).await?;
                return Ok(Load::Done);
            }
        }
        Ok(match self.wait_loaded(since, limit, NAVIGATION_GRACE).await {
            Load::Nothing => Load::Done,
            other => other,
        })
    }

    async fn settle(&self, since: u64, limit: Duration) -> Result<Load> {
        Ok(self.wait_loaded(since, limit, NAVIGATION_GRACE).await)
    }

    fn loads(&self) -> u64 {
        self.shared.started.load(Ordering::SeqCst)
    }

    async fn evaluate(&self, expression: &str) -> Result<Evaluated> {
        let answer = self
            .cdp
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                    "replMode": true,
                    "userGesture": true,
                    "timeout": 30_000,
                }),
            )
            .await?;
        thrown(&answer)?;
        let result = &answer["result"];
        let description = result["description"]
            .as_str()
            .or_else(|| result["type"].as_str())
            .unwrap_or_default()
            .to_string();
        let value = match (result.get("value"), result["unserializableValue"].as_str()) {
            (_, Some(text)) => Some(Value::String(text.to_string())),
            // A node or a function comes by value as `{}`: its description
            // says more than that.
            (Some(value), _) if result["subtype"] != "node" && result["type"] != "function" => {
                Some(value.clone())
            }
            _ => None,
        };
        Ok(Evaluated { value, description })
    }

    async fn helper(&self, call: &str) -> Result<Value> {
        let expression = format!("__yaca.{call}");
        for attempt in 0..2 {
            let world = self.world().await?;
            let answer = self
                .cdp
                .call(
                    "Runtime.evaluate",
                    json!({
                        "expression": expression,
                        "contextId": world,
                        "returnByValue": true,
                        "awaitPromise": true,
                    }),
                )
                .await;
            match answer {
                // The document changed under the call, and its world with it.
                Err(why) if attempt == 0 && why.contains("context") => {
                    *self.shared.world.lock().unwrap() = None;
                }
                Err(why) => return Err(why),
                Ok(answer) => {
                    thrown(&answer)?;
                    return Ok(answer["result"]["value"].clone());
                }
            }
        }
        Err("the page kept changing under the call".into())
    }

    async fn screenshot(&self, area: Area) -> Result<Shot> {
        let metrics = self.cdp.call("Page.getLayoutMetrics", json!({})).await?;
        let view = &metrics["cssVisualViewport"];
        let content = &metrics["cssContentSize"];
        let number = |value: &Value| value.as_f64().unwrap_or(0.0);
        let ratio = self
            .evaluate("devicePixelRatio")
            .await
            .ok()
            .and_then(|e| e.value)
            .and_then(|v| v.as_f64())
            .filter(|r| *r > 0.0)
            .unwrap_or(1.0);
        let (clip, beyond, cut) = match area {
            Area::Viewport => (
                Rect {
                    x: number(&view["pageX"]),
                    y: number(&view["pageY"]),
                    width: number(&view["clientWidth"]),
                    height: number(&view["clientHeight"]),
                },
                false,
                false,
            ),
            Area::FullPage { max_height } => {
                let height = number(&content["height"]);
                (
                    Rect {
                        x: 0.0,
                        y: 0.0,
                        width: number(&content["width"]),
                        height: height.min(max_height),
                    },
                    true,
                    height > max_height,
                )
            }
            Area::Box(rect) => (rect, true, false),
        };
        if clip.width < 1.0 || clip.height < 1.0 {
            return Err("there is nothing to photograph: the area is empty".into());
        }
        let answer = self
            .cdp
            .call(
                "Page.captureScreenshot",
                json!({
                    "format": "png",
                    "captureBeyondViewport": beyond,
                    "fromSurface": true,
                    // CSS pixels, not the screen's: the model reads the page
                    // at the size it is laid out at.
                    "clip": {
                        "x": clip.x, "y": clip.y,
                        "width": clip.width, "height": clip.height,
                        "scale": 1.0 / ratio,
                    },
                }),
            )
            .await?;
        let data = answer["data"].as_str().ok_or("the browser returned no image")?;
        let png = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| format!("the image did not decode: {e}"))?;
        let (width, height) = png_size(&png);
        Ok(Shot { png, width, height, cut })
    }

    async fn click(&self, x: f64, y: f64, button: Button, count: u32) -> Result<()> {
        let name = match button {
            Button::Left => "left",
            Button::Right => "right",
        };
        self.mouse("mouseMoved", x, y, json!({})).await?;
        for n in 1..=count.max(1) {
            let press = json!({ "button": name, "clickCount": n, "buttons": 1 });
            self.mouse("mousePressed", x, y, press).await?;
            let release = json!({ "button": name, "clickCount": n, "buttons": 0 });
            self.mouse("mouseReleased", x, y, release).await?;
        }
        Ok(())
    }

    async fn hover(&self, x: f64, y: f64) -> Result<()> {
        self.mouse("mouseMoved", x, y, json!({})).await
    }

    async fn wheel(&self, x: f64, y: f64, dx: f64, dy: f64) -> Result<()> {
        self.mouse("mouseWheel", x, y, json!({ "deltaX": dx, "deltaY": dy })).await
    }

    async fn insert_text(&self, text: &str) -> Result<()> {
        self.cdp.call("Input.insertText", json!({ "text": text })).await.map(drop)
    }

    async fn press(&self, press: &Press) -> Result<()> {
        let mut down = json!({
            "type": if press.text.is_some() { "keyDown" } else { "rawKeyDown" },
            "key": press.key,
            "code": press.code,
            "windowsVirtualKeyCode": press.key_code,
            "nativeVirtualKeyCode": press.key_code,
            "modifiers": press.modifiers,
        });
        if let Some(text) = &press.text {
            down["text"] = json!(text);
            down["unmodifiedText"] = json!(text);
        }
        self.cdp.call("Input.dispatchKeyEvent", down).await?;
        let up = json!({
            "type": "keyUp",
            "key": press.key,
            "code": press.code,
            "windowsVirtualKeyCode": press.key_code,
            "nativeVirtualKeyCode": press.key_code,
            "modifiers": press.modifiers,
        });
        self.cdp.call("Input.dispatchKeyEvent", up).await.map(drop)
    }

    async fn highlight(&self, rect: Rect) -> Result<()> {
        let (r, g, b) = RING;
        self.cdp
            .call(
                "Overlay.highlightRect",
                json!({
                    "x": rect.x.round() as i64,
                    "y": rect.y.round() as i64,
                    "width": rect.width.round().max(1.0) as i64,
                    "height": rect.height.round().max(1.0) as i64,
                    "color": { "r": r, "g": g, "b": b, "a": 0.18 },
                    "outlineColor": { "r": r, "g": g, "b": b, "a": 1.0 },
                }),
            )
            .await?;
        let cdp = self.cdp.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(900)).await;
            let _ = cdp.call("Overlay.hideHighlight", json!({})).await;
        });
        Ok(())
    }

    async fn emulate(&self, emulation: Option<Emulation>) -> Result<()> {
        match emulation {
            Some(e) => {
                self.cdp
                    .call(
                        "Emulation.setDeviceMetricsOverride",
                        json!({
                            "width": e.size.width.round() as i64,
                            "height": e.size.height.round() as i64,
                            "deviceScaleFactor": 0,
                            "mobile": e.mobile,
                            "scale": e.scale,
                        }),
                    )
                    .await?;
                self.cdp
                    .call(
                        "Emulation.setTouchEmulationEnabled",
                        json!({ "enabled": e.mobile, "maxTouchPoints": 5 }),
                    )
                    .await
                    .map(drop)
            }
            None => {
                self.cdp.call("Emulation.clearDeviceMetricsOverride", json!({})).await?;
                self.cdp
                    .call("Emulation.setTouchEmulationEnabled", json!({ "enabled": false }))
                    .await
                    .map(drop)
            }
        }
    }

    fn set_bounds(&self, bounds: Rect) -> Result<()> {
        self.webview
            .set_bounds(tauri::Rect {
                position: LogicalPosition::new(bounds.x, bounds.y).into(),
                size: LogicalSize::new(bounds.width, bounds.height).into(),
            })
            .map_err(|e| e.to_string())
    }

    fn location(&self) -> Location {
        self.shared.location.lock().unwrap().clone()
    }

    fn logs(&self) -> &Logs {
        &self.shared.logs
    }

    fn open_devtools(&self) {
        self.webview.open_devtools();
    }

    fn close(&self) {
        let _ = self.webview.close();
    }
}
