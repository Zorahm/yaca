//! The browser of a chat: a real page the model works in through MCP tools
//! and the human watches, and may click in, in the browser column.
//!
//! One tab set per chat, with webviews made when a page is needed. They
//! share one profile, so a login made in
//! one chat holds in the others, and none of them shares anything with the
//! window's own webview.
//!
//! The model reaches the browser through an MCP server of the application
//! (`server`), the human through the commands at the bottom of this file. The
//! agent and the human share a page until the human pauses agent access.
//!
//! Windows only for now. The tools and the layout are the same everywhere;
//! what is missing elsewhere is an engine (`engine`), and until there is one
//! the window offers no browser at all rather than one that cannot open.

mod access;
mod act;
mod commands;
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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Url};
#[cfg(windows)]
use tauri::Manager;
use yaca_core::id::SessionId;
use yaca_core::mcp::AppServerSpec;

use access::{Access, BrowserRequest};
pub(crate) use commands::*;
use engine::{Engine, PageError};
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
    pub paused: bool,
    pub taken: bool,
    pub request: Option<BrowserRequest>,
    pub error: Option<PageError>,
    pub console_errors: u32,
    pub zoom: u32,
    pub tabs: Vec<BrowserTab>,
    pub active_tab: u32,
    pub recent: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub id: u32,
    pub title: String,
    pub url: String,
    pub favicon: Option<String>,
}

#[derive(Default)]
struct Place {
    shown: Option<Rect>,
    forced: Option<Forced>,
}

#[derive(Default)]
struct Tab {
    id: u32,
    live: Option<Arc<dyn Engine>>,
    applied: Option<Option<Emulation>>,
}

#[derive(Default)]
struct Tabs {
    active: usize,
    next: u32,
    pages: Vec<Tab>,
    recent: Vec<String>,
    closed: bool,
}

impl Tabs {
    fn add(&mut self) {
        self.pages.push(Tab { id: self.next, ..Tab::default() });
        self.next += 1;
        self.active = self.pages.len() - 1;
    }
}

pub struct Browser {
    app: AppHandle,
    me: Weak<Browser>,
    label: String,
    session: Mutex<Option<SessionId>>,
    /// Held while the page is being made, so two first calls make one.
    making: tokio::sync::Mutex<()>,
    tabs: Mutex<Tabs>,
    place: Mutex<Place>,
    placing: tokio::sync::Mutex<()>,
    busy: AtomicBool,
    paused: AtomicBool,
    taken: AtomicBool,
    access: Access,
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
            tabs: Mutex::new(Tabs { next: 1, pages: vec![Tab::default()], ..Tabs::default() }),
            place: Mutex::new(Place::default()),
            placing: tokio::sync::Mutex::new(()),
            busy: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            taken: AtomicBool::new(false),
            access: Access::default(),
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
        let tabs = self.tabs.lock().unwrap();
        tabs.pages.get(tabs.active)?.live.clone()
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
        let id = {
            let tabs = self.tabs.lock().unwrap();
            tabs.pages.get(tabs.active).ok_or("the browser is closed")?.id
        };
        let engine = self.make(layout::plan(None, None), id).await?;
        {
            let mut tabs = self.tabs.lock().unwrap();
            if let Some(tab) = tabs.pages.iter_mut().find(|tab| tab.id == id) {
                tab.live = Some(engine.clone());
            } else {
                engine.close();
                return Err("the browser tab is closed".into());
            }
        }
        self.apply().await;
        Ok(engine)
    }

    #[cfg(windows)]
    fn make(&self, plan: Plan, id: u32) -> std::pin::Pin<Box<
        dyn std::future::Future<Output = Result<Arc<dyn Engine>, String>> + Send + '_,
    >> {
        // The WebView2 popup callback can create another page through this factory.
        Box::pin(async move {
            let window = self.app.get_window("main").ok_or("the window is gone")?;
            let engine = webview2::WebView2::open(
                window, format!("{}-{id}", self.label), profile()?, plan.bounds, self.me.clone(),
            ).await?;
            Ok(engine as Arc<dyn Engine>)
        })
    }

    #[cfg(not(windows))]
    async fn make(&self, _plan: Plan, _id: u32) -> Result<Arc<dyn Engine>, String> {
        Err("this build has no browser engine".into())
    }

    /// Puts the page where the plan says, and tells the column.
    async fn apply(&self) {
        let _placing = self.placing.lock().await;
        let plan = self.plan();
        let changes = {
            let mut tabs = self.tabs.lock().unwrap();
            let active = tabs.active;
            let away = layout::plan(None, None);
            tabs.pages.iter_mut().enumerate().filter_map(|(index, tab)| {
                let engine = tab.live.clone()?;
                let plan = if index == active && !engine.location().hides_page() { plan } else { away };
                let resend = tab.applied != Some(plan.emulate);
                tab.applied = Some(plan.emulate);
                Some((index == active, engine, plan, resend))
            }).collect::<Vec<_>>()
        };
        // Move the outgoing view first so it cannot cover the new active tab.
        for (_, engine, plan, _) in changes.iter().filter(|(active, ..)| !active) {
            let _ = engine.set_bounds(plan.bounds);
        }
        for (active, engine, plan, resend) in changes {
            if active {
                let _ = engine.set_bounds(plan.bounds);
            }
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
        self.busy.store(true, Ordering::SeqCst);
        self.emit();
        Busy(self)
    }

    pub fn state(&self) -> Option<BrowserState> {
        let session = self.session.lock().unwrap().clone()?;
        let plan = self.plan();
        let mut tabs = self.tabs.lock().unwrap();
        let tab = tabs.pages.get(tabs.active)?;
        let active_tab = tab.id;
        let location = tab.live.as_ref().map(|engine| engine.location());
        let console_errors = tab.live.as_ref().map_or(0, |engine| engine.logs().error_count());
        if let Some(page) = &location {
            if !page.loading && page.error.is_none() {
                if let Ok(mut url) = Url::parse(&page.url) {
                    if access::origin(&url).is_some() {
                        let _ = url.set_username("");
                        let _ = url.set_password(None);
                        url.set_query(None);
                        url.set_fragment(None);
                        let url = url.to_string();
                        tabs.recent.retain(|entry| entry != &url);
                        tabs.recent.insert(0, url);
                        tabs.recent.truncate(8);
                    }
                }
            }
        }
        Some(BrowserState {
            session: session.to_string(),
            opened: location.is_some(),
            url: location.as_ref().map(|l| l.url.clone()).unwrap_or_default(),
            title: location
                .as_ref()
                .map(|l| l.title.clone())
                .unwrap_or_default(),
            loading: location.as_ref().is_some_and(|l| l.loading),
            can_go_back: location.as_ref().is_some_and(|l| l.can_go_back),
            can_go_forward: location.as_ref().is_some_and(|l| l.can_go_forward),
            width: plan.viewport.width,
            height: plan.viewport.height,
            source: plan.source,
            busy: self.busy.load(Ordering::SeqCst),
            paused: self.paused.load(Ordering::SeqCst),
            taken: self.taken.load(Ordering::SeqCst),
            request: self.access.pending(),
            error: location.as_ref().and_then(|location| location.error),
            console_errors,
            zoom: location.as_ref().map_or(100, |location| location.zoom),
            tabs: tabs.pages.iter().map(|tab| {
                let page = tab.live.as_ref().map(|engine| engine.location()).unwrap_or_default();
                BrowserTab { id: tab.id, title: page.title, url: page.url, favicon: page.favicon }
            }).collect(),
            active_tab,
            recent: tabs.recent.clone(),
        })
    }

    fn emit(&self) {
        if let Some(state) = self.state() {
            let _ = self.app.emit(EVENT, state);
        }
    }

    fn close(&self) {
        self.access.deny_all();
        let pages = {
            let mut tabs = self.tabs.lock().unwrap();
            tabs.closed = true;
            std::mem::take(&mut tabs.pages)
        };
        for tab in pages {
            if let Some(engine) = tab.live {
                engine.close();
            }
        }
    }

    fn new_tab(&self) -> Result<(), String> {
        let mut tabs = self.tabs.lock().unwrap();
        if tabs.closed || tabs.pages.len() >= 16 {
            return Err("tab_limit".into());
        }
        tabs.add();
        Ok(())
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
        self.0.busy.store(false, Ordering::SeqCst);
        if self.0.taken.load(Ordering::SeqCst) {
            self.0.focus_page();
        }
        self.0.emit();
    }
}

impl Browser {
    fn focus_page(&self) {
        if self.place.lock().unwrap().shown.is_some() {
            if let Some(engine) = self.live().filter(|engine| !engine.location().hides_page()) {
                let _ = engine.focus();
            }
        }
    }
}

/// Where the browsers keep their cookies and storage.
fn profile() -> Result<PathBuf, String> {
    Ok(crate::config_dir()?.join("browser"))
}

/// The browsers of the open chats.
#[derive(Default)]
pub struct Browsers {
    chats: Mutex<HashMap<SessionId, Arc<Browser>>>,
    made: AtomicU64,
    placement: tokio::sync::Mutex<()>,
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
            if matches!(
                host,
                "tauri.localhost" | "ipc.localhost" | "asset.localhost"
            ) {
                return false;
            }
            // The window's dev server, in a build that loads from one.
            !(cfg!(debug_assertions) && host == "127.0.0.1" && url.port() == Some(1420))
        }
        "about" | "data" | "blob" | "file" => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_addresses_get_the_scheme_they_mean() {
        assert_eq!(
            normalize("localhost:5173").unwrap(),
            "http://localhost:5173"
        );
        assert_eq!(
            normalize(" 127.0.0.1:8080/app ").unwrap(),
            "http://127.0.0.1:8080/app"
        );
        assert_eq!(normalize("shop.test").unwrap(), "http://shop.test");
        assert_eq!(
            normalize("example.com/docs").unwrap(),
            "https://example.com/docs"
        );
        assert_eq!(
            normalize("https://example.com").unwrap(),
            "https://example.com"
        );
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
