//! The browser of a chat: a real page the model works in through MCP tools
//! and the human watches, and may click in, in the browser column.
//!
//! One per chat, made the first time the page is needed: a chat that never
//! browses costs nothing. All of them share one profile, so a login made in
//! one chat holds in the others, and none of them shares anything with the
//! window's own webview.
//!
//! The model reaches the browser through an MCP server of the application
//! (`server`), the human through the commands at the bottom of this file. The
//! two are equals: there is no mode in which one of them is locked out.
//!
//! Windows only for now. The tools and the layout are the same everywhere;
//! what is missing elsewhere is an engine (`engine`), and until there is one
//! the window offers no browser at all rather than one that cannot open.

mod act;
mod engine;
mod keys;
pub mod layout;
mod logs;
mod server;
mod tools;
#[cfg(windows)]
mod webview2;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, Url};
use yaca_core::id::SessionId;
use yaca_core::mcp::AppServerSpec;

use engine::{Engine, Step, LOAD_LIMIT};
use layout::{Emulation, Forced, Plan, Rect, ViewportSource};

/// What the column hears about a browser. One channel for all of them: the
/// payload names its chat.
pub const EVENT: &str = "yaca://browser";

/// The server's name, and so the tools' prefix: `mcp__browser__navigate`.
const SERVER_NAME: &str = "browser";

/// Whether this build has an engine.
pub fn supported() -> bool {
    cfg!(windows)
}

/// A browser as the column draws it.
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct BrowserState {
    pub session: String,
    /// A page exists. Until then the column shows its empty state.
    pub opened: bool,
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// The size the page is laid out at, and what decides it.
    pub width: f64,
    pub height: f64,
    pub source: ViewportSource,
    /// A tool call of the model is running.
    pub busy: bool,
}

/// Where the column wants the page, and the size the model asked for.
#[derive(Default)]
struct Place {
    shown: Option<Rect>,
    forced: Option<Forced>,
    /// The emulation last sent, so a column being dragged does not resend it
    /// on every pixel.
    applied: Option<Option<Emulation>>,
}

pub struct Browser {
    app: AppHandle,
    me: Weak<Browser>,
    label: String,
    session: Mutex<Option<SessionId>>,
    /// Held while the page is being made, so two first calls make one.
    making: tokio::sync::Mutex<()>,
    live: Mutex<Option<Arc<dyn Engine>>>,
    place: Mutex<Place>,
    busy: AtomicUsize,
    /// One tool call at a time (`server`).
    calls: tokio::sync::Mutex<()>,
}

impl Browser {
    fn new(app: &AppHandle, label: String) -> Arc<Browser> {
        Arc::new_cyclic(|me| Browser {
            app: app.clone(),
            me: me.clone(),
            label,
            session: Mutex::new(None),
            making: tokio::sync::Mutex::new(()),
            live: Mutex::new(None),
            place: Mutex::new(Place::default()),
            busy: AtomicUsize::new(0),
            calls: tokio::sync::Mutex::new(()),
        })
    }

    /// The MCP server to hand the chat's session.
    fn server(self: &Arc<Self>) -> AppServerSpec {
        AppServerSpec {
            name: SERVER_NAME.into(),
            target: "Yaca".into(),
            server: Arc::new(server::Server(Arc::downgrade(self))),
        }
    }

    pub fn live(&self) -> Option<Arc<dyn Engine>> {
        self.live.lock().unwrap().clone()
    }

    pub fn plan(&self) -> Plan {
        let place = self.place.lock().unwrap();
        layout::plan(place.shown, place.forced)
    }

    /// The page, made now if there is none yet.
    pub async fn engine(&self) -> Result<Arc<dyn Engine>, String> {
        let _making = self.making.lock().await;
        if let Some(engine) = self.live() {
            return Ok(engine);
        }
        let engine = self.make(self.plan()).await?;
        *self.live.lock().unwrap() = Some(engine.clone());
        self.place.lock().unwrap().applied = None;
        self.apply().await;
        Ok(engine)
    }

    #[cfg(windows)]
    async fn make(&self, plan: Plan) -> Result<Arc<dyn Engine>, String> {
        let window = self.app.get_window("main").ok_or("the window is gone")?;
        let me = self.me.clone();
        let changed = Box::new(move || {
            if let Some(browser) = me.upgrade() {
                browser.emit();
            }
        });
        let engine = webview2::WebView2::open(window, self.label.clone(), profile()?, plan.bounds, changed).await?;
        Ok(engine)
    }

    #[cfg(not(windows))]
    async fn make(&self, _plan: Plan) -> Result<Arc<dyn Engine>, String> {
        Err("this build has no browser engine".into())
    }

    /// Puts the page where the plan says, and tells the column.
    async fn apply(&self) {
        let plan = self.plan();
        if let Some(engine) = self.live() {
            let _ = engine.set_bounds(plan.bounds);
            let resend = {
                let mut place = self.place.lock().unwrap();
                let resend = place.applied != Some(plan.emulate);
                place.applied = Some(plan.emulate);
                resend
            };
            if resend {
                let _ = engine.emulate(plan.emulate).await;
            }
        }
        self.emit();
    }

    /// The column shows this page at `shown`, or not at all.
    async fn show(&self, shown: Option<Rect>) {
        let same = {
            let mut place = self.place.lock().unwrap();
            let same = place.shown == shown;
            place.shown = shown;
            same
        };
        if !same {
            self.apply().await;
        }
    }

    /// The size the model asked for; `None` follows the column again.
    pub async fn force(&self, forced: Option<Forced>) -> Result<(), String> {
        self.place.lock().unwrap().forced = forced;
        self.apply().await;
        Ok(())
    }

    /// Counts a tool call as running until the guard drops.
    fn busy(&self) -> Busy<'_> {
        self.busy.fetch_add(1, Ordering::SeqCst);
        self.emit();
        Busy(self)
    }

    pub fn state(&self) -> Option<BrowserState> {
        let session = self.session.lock().unwrap().clone()?;
        let plan = self.plan();
        let location = self.live().map(|engine| engine.location());
        Some(BrowserState {
            session: session.to_string(),
            opened: location.is_some(),
            url: location.as_ref().map(|l| l.url.clone()).unwrap_or_default(),
            title: location.as_ref().map(|l| l.title.clone()).unwrap_or_default(),
            loading: location.as_ref().is_some_and(|l| l.loading),
            can_go_back: location.as_ref().is_some_and(|l| l.can_go_back),
            can_go_forward: location.as_ref().is_some_and(|l| l.can_go_forward),
            width: plan.viewport.width,
            height: plan.viewport.height,
            source: plan.source,
            busy: self.busy.load(Ordering::SeqCst) > 0,
        })
    }

    fn emit(&self) {
        if let Some(state) = self.state() {
            let _ = self.app.emit(EVENT, state);
        }
    }

    fn close(&self) {
        if let Some(engine) = self.live.lock().unwrap().take() {
            engine.close();
        }
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        self.close();
    }
}

struct Busy<'a>(&'a Browser);

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.busy.fetch_sub(1, Ordering::SeqCst);
        self.0.emit();
    }
}

/// Where the browsers keep their cookies and storage.
fn profile() -> Result<PathBuf, String> {
    Ok(yaca_core::provider::registry::config_dir().map_err(|e| e.to_string())?.join("browser"))
}

/// The browsers of the open chats.
#[derive(Default)]
pub struct Browsers {
    chats: Mutex<HashMap<SessionId, Arc<Browser>>>,
    made: AtomicU64,
}

impl Browsers {
    /// A browser for a chat that is about to be opened, and the server to
    /// declare in its session. `None` where there is no engine.
    pub fn prepare(&self, app: &AppHandle) -> Option<(Arc<Browser>, AppServerSpec)> {
        if !supported() {
            return None;
        }
        // The chat's id is not known until its session exists; the label
        // only has to be unique among webviews.
        let label = format!("browser-{}", self.made.fetch_add(1, Ordering::Relaxed));
        let browser = Browser::new(app, label);
        let server = browser.server();
        Some((browser, server))
    }

    /// The session exists now: the browser is its chat's.
    pub fn bind(&self, id: SessionId, browser: Arc<Browser>) {
        *browser.session.lock().unwrap() = Some(id.clone());
        if let Some(old) = self.chats.lock().unwrap().insert(id, browser) {
            old.close();
        }
    }

    /// The chat is closed, and its page with it.
    pub fn forget(&self, id: &SessionId) {
        if let Some(browser) = self.chats.lock().unwrap().remove(id) {
            browser.close();
        }
    }

    fn get(&self, session: &str) -> Result<Arc<Browser>, String> {
        self.chats
            .lock()
            .unwrap()
            .get(&SessionId::from(session.to_string()))
            .cloned()
            .ok_or_else(|| format!("the chat {session} has no browser"))
    }

    fn all(&self) -> Vec<Arc<Browser>> {
        self.chats.lock().unwrap().values().cloned().collect()
    }
}

/// What the address field and `navigate` accept, as a URL to open.
///
/// A bare `localhost:5173` is a dev server and opens over http; any other
/// bare host is taken to be a site and opens over https.
pub fn normalize(input: &str) -> Result<String, String> {
    let text = input.trim();
    if text.is_empty() {
        return Err("the address is empty".into());
    }
    let local = |host: &str| {
        let host = host.split([':', '/']).next().unwrap_or_default();
        host == "localhost"
            || host.ends_with(".localhost")
            || host.ends_with(".test")
            || host == "127.0.0.1"
            || host == "[::1]"
            || host == "0.0.0.0"
    };
    let schemed = text
        .split_once(':')
        .filter(|(scheme, rest)| {
            !scheme.is_empty()
                && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
                // `localhost:5173` is a host and a port, not a scheme.
                && !rest.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    let url = match schemed.as_deref() {
        Some("http" | "https" | "file" | "about" | "data") => text.to_string(),
        Some(other) => return Err(format!("{other}: pages open over http, https or file only")),
        None if local(text) => format!("http://{text}"),
        None => format!("https://{text}"),
    };
    Url::parse(&url).map_err(|e| format!("{text} is not an address: {e}"))?;
    Ok(url)
}

/// Whether the browser may go to `url`.
///
/// Nowhere the window itself is served from: a page loaded from there would
/// count as the application's own, and the application's commands answer
/// such pages. Everything else a browser would open, it opens.
pub fn allowed(url: &Url) -> bool {
    match url.scheme() {
        "http" | "https" => {
            let host = url.host_str().unwrap_or_default();
            if matches!(host, "tauri.localhost" | "ipc.localhost" | "asset.localhost") {
                return false;
            }
            // The window's dev server, in a build that loads from one.
            !(cfg!(debug_assertions) && host == "127.0.0.1" && url.port() == Some(1420))
        }
        "about" | "data" | "blob" | "file" => true,
        _ => false,
    }
}

// --- the column's commands -------------------------------------------------

#[tauri::command]
pub fn browser_supported() -> bool {
    supported()
}

/// Every chat's browser, for a window that was reloaded.
#[tauri::command]
pub fn browser_states(browsers: State<'_, Browsers>) -> Vec<BrowserState> {
    browsers.all().iter().filter_map(|b| b.state()).collect()
}

/// The column shows `session`'s page at `rect`; every other page goes out of
/// sight. Both `None`: the column is closed.
#[tauri::command]
pub async fn browser_place(
    browsers: State<'_, Browsers>,
    session: Option<String>,
    rect: Option<Rect>,
) -> Result<(), String> {
    let shown = session.zip(rect);
    for browser in browsers.all() {
        let mine = match (&shown, browser.session.lock().unwrap().as_ref()) {
            (Some((session, rect)), Some(id)) if id.to_string() == *session => Some(*rect),
            _ => None,
        };
        browser.show(mine).await;
    }
    Ok(())
}

/// The address field: opens the page, making it if this is the first.
///
/// Fails with a code the column words for the human: `bad_address` for what
/// is not an address, `failed: <why>` for a page that did not open.
#[tauri::command]
pub async fn browser_open(browsers: State<'_, Browsers>, session: String, url: String) -> Result<(), String> {
    let browser = browsers.get(&session)?;
    let url = normalize(&url).map_err(|_| "bad_address".to_string())?;
    let engine = browser.engine().await.map_err(|why| format!("failed: {why}"))?;
    engine
        .navigate(&url, LOAD_LIMIT)
        .await
        .map(drop)
        .map_err(|why| format!("failed: {why}"))
}

#[tauri::command]
pub async fn browser_go(browsers: State<'_, Browsers>, session: String, step: String) -> Result<(), String> {
    let step = match step.as_str() {
        "back" => Step::Back,
        "forward" => Step::Forward,
        "reload" => Step::Reload,
        "stop" => Step::Stop,
        other => return Err(format!("unknown step {other}")),
    };
    let engine = tools::open_page(&*browsers.get(&session)?)?;
    engine.step(step, LOAD_LIMIT).await.map(drop)
}

/// Opens the page in the system's own browser.
#[tauri::command]
pub fn browser_external(app: AppHandle, browsers: State<'_, Browsers>, session: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let engine = tools::open_page(&*browsers.get(&session)?)?;
    let url = engine.location().url;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("only a web page opens in another browser".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn browser_devtools(browsers: State<'_, Browsers>, session: String) -> Result<(), String> {
    tools::open_page(&*browsers.get(&session)?)?.open_devtools();
    Ok(())
}

/// The cross in the header: the page follows the column again.
#[tauri::command]
pub async fn browser_reset_viewport(browsers: State<'_, Browsers>, session: String) -> Result<(), String> {
    browsers.get(&session)?.force(None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_addresses_get_the_scheme_they_mean() {
        assert_eq!(normalize("localhost:5173").unwrap(), "http://localhost:5173");
        assert_eq!(normalize(" 127.0.0.1:8080/app ").unwrap(), "http://127.0.0.1:8080/app");
        assert_eq!(normalize("shop.test").unwrap(), "http://shop.test");
        assert_eq!(normalize("example.com/docs").unwrap(), "https://example.com/docs");
        assert_eq!(normalize("https://example.com").unwrap(), "https://example.com");
        assert_eq!(normalize("about:blank").unwrap(), "about:blank");
    }

    #[test]
    fn other_schemes_and_nothing_are_refused() {
        assert!(normalize("").is_err());
        assert!(normalize("javascript:alert(1)").is_err());
        assert!(normalize("ftp://example.com").is_err());
    }

    #[test]
    fn the_windows_own_pages_are_out_of_reach() {
        let ok = |url: &str| allowed(&Url::parse(url).unwrap());
        assert!(ok("http://localhost:5173/"));
        assert!(ok("https://example.com/"));
        assert!(ok("about:blank"));
        assert!(!ok("http://tauri.localhost/index.html"));
        assert!(!ok("http://ipc.localhost/plugin:event|listen"));
        assert!(!ok("tauri://localhost/"));
        assert!(!ok("javascript:alert(1)"));
        assert_eq!(ok("http://127.0.0.1:1420/"), !cfg!(debug_assertions));
    }
}
