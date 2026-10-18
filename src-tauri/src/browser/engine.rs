//! What a browser engine has to offer the tools.
//!
//! The tools are written against this and nothing below it, so a second
//! engine — WKWebView on macOS, WebKitGTK on Linux — is a second
//! implementation, not a second set of tools. Windows has the only one for
//! now (`webview2`).
//!
//! Positions are CSS pixels of the page's viewport, the coordinates a
//! screenshot is taken in; boxes for screenshots are in document pixels.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use super::keys::Press;
use super::layout::{Emulation, Rect};
use super::logs::Logs;

pub type Result<T> = std::result::Result<T, String>;

/// A step through the page's history, or of its loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Back,
    Forward,
    Reload,
    Stop,
}

/// How a wait for loading ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Load {
    /// Nothing started loading.
    Nothing,
    Done,
    /// Still loading when the wait ran out.
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
}

/// What part of the page a screenshot covers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Area {
    Viewport,
    /// The whole document, cut off at this height.
    FullPage { max_height: f64 },
    /// A box in document pixels.
    Box(Rect),
}

/// A screenshot: a PNG and its size in pixels.
pub struct Shot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// The document was taller than a full-page shot may be.
    pub cut: bool,
}

/// What a script came to.
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluated {
    /// The value as JSON. `None` for what JSON cannot hold: `undefined`, a
    /// function, a DOM node.
    pub value: Option<Value>,
    /// How the console would show it, for when there is no value.
    pub description: String,
}

/// What a page is, as the header of the column and every tool result say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Location {
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

#[async_trait]
pub trait Engine: Send + Sync {
    /// Opens `url` and waits, at most `limit`, for it to load.
    async fn navigate(&self, url: &str, limit: Duration) -> Result<Load>;

    async fn step(&self, step: Step, limit: Duration) -> Result<Load>;

    /// Waits, at most `limit`, for a navigation that started after `since`
    /// (a value of [`Engine::loads`]) to finish.
    async fn settle(&self, since: u64, limit: Duration) -> Result<Load>;

    /// How many navigations have started so far.
    fn loads(&self) -> u64;

    /// Runs `expression` in the page's own world. A promise is waited for,
    /// and top-level `await` works.
    async fn evaluate(&self, expression: &str) -> Result<Evaluated>;

    /// Calls `__yaca.<call>` in the helpers' isolated world (`page.js`) and
    /// returns its value.
    async fn helper(&self, call: &str) -> Result<Value>;

    async fn screenshot(&self, area: Area) -> Result<Shot>;

    async fn click(&self, x: f64, y: f64, button: Button, count: u32) -> Result<()>;
    async fn hover(&self, x: f64, y: f64) -> Result<()>;
    async fn wheel(&self, x: f64, y: f64, dx: f64, dy: f64) -> Result<()>;
    async fn insert_text(&self, text: &str) -> Result<()>;
    async fn press(&self, press: &Press) -> Result<()>;

    /// Rings a box in viewport pixels for a moment, without touching the
    /// page's DOM: the human's view of where the model is acting.
    async fn highlight(&self, rect: Rect) -> Result<()>;

    /// Lays the page out at another size than its webview, or stops doing so.
    async fn emulate(&self, emulation: Option<Emulation>) -> Result<()>;

    /// Moves and sizes the webview in the window.
    fn set_bounds(&self, bounds: Rect) -> Result<()>;

    fn location(&self) -> Location;
    fn logs(&self) -> &Logs;

    fn open_devtools(&self);
    fn close(&self);
}

/// How long a page may take to load before a tool answers anyway.
pub const LOAD_LIMIT: Duration = Duration::from_secs(20);
