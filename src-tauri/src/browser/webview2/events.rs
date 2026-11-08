//! What the page tells the engine as it goes: where it is, whether it is
//! loading, what it logged, and the dialogs nobody is there to answer.

use std::sync::atomic::Ordering;
use std::sync::Weak;

use serde_json::{json, Value};
use tauri::Url;
use tokio::sync::mpsc;

use super::cdp::{Cdp, Event};
use super::Shared;
use crate::browser::access::origin;
use crate::browser::engine::{PageError, LOAD_LIMIT};
use crate::browser::logs::{console_text, NetworkEntry};

/// Runs until the webview goes, which drops the last sender.
pub(crate) async fn follow(
    shared: Weak<Shared>,
    cdp: Cdp,
    mut events: mpsc::UnboundedReceiver<Event>,
) {
    let mut document = None;
    let mut form = None;
    while let Some((name, params)) = events.recv().await {
        let Some(shared) = shared.upgrade() else {
            return;
        };
        let main = shared.main_frame.lock().unwrap().clone();
        let is_main = |frame: &Value| frame.as_str() == Some(main.as_str());
        match name {
            "Page.frameNavigated" => {
                let frame = &params["frame"];
                if frame.get("parentId").is_some_and(|p| !p.is_null()) {
                    continue;
                }
                if let Some(id) = frame["id"].as_str() {
                    *shared.main_frame.lock().unwrap() = id.to_string();
                }
                let url = format!(
                    "{}{}",
                    frame["url"].as_str().unwrap_or_default(),
                    frame["urlFragment"].as_str().unwrap_or_default()
                );
                *shared.world.lock().unwrap() = None;
                {
                    let mut location = shared.location.lock().unwrap();
                    if frame.get("unreachableUrl").is_some() || url.starts_with("chrome-error:") {
                        if location.error.is_none() {
                            location.error = Some(PageError::Network);
                            if let Some(url) = frame["unreachableUrl"].as_str() {
                                location.url = url.to_string();
                            }
                        }
                    } else {
                        if location.url != url {
                            location.title.clear();
                            location.favicon = None;
                        }
                        location.url = url;
                        location.error = None;
                    }
                }
                history(&shared, &cdp).await;
            }
            "Page.navigatedWithinDocument" if is_main(&params["frameId"]) => {
                shared.location.lock().unwrap().url =
                    params["url"].as_str().unwrap_or_default().into();
                history(&shared, &cdp).await;
            }
            "Page.frameStartedLoading" if is_main(&params["frameId"]) => {
                shared.started.fetch_add(1, Ordering::SeqCst);
                shared.location.lock().unwrap().loading = true;
                shared.changed();
            }
            "Page.frameStoppedLoading" if is_main(&params["frameId"]) => {
                shared.stopped.send_replace(shared.started.load(Ordering::SeqCst));
                shared.location.lock().unwrap().loading = false;
                shared.changed();
            }
            "Page.frameRequestedNavigation" if is_main(&params["frameId"]) => {
                form = matches!(params["reason"].as_str(), Some("formSubmissionGet" | "formSubmissionPost"))
                    .then(|| params["url"].as_str().unwrap_or_default().to_string());
            }
            "Fetch.requestPaused" => {
                let request = &params["request"];
                let top = params["resourceType"] == "Document" && is_main(&params["frameId"]);
                let get_form = top && form.take().as_deref() == request["url"].as_str();
                tokio::spawn(intercept(shared, cdp.clone(), params, top, get_form));
            }
            "Page.javascriptDialogOpening" => {
                // Nobody is at the page to answer it, and an open dialog
                // freezes it. A prompt is refused: there is no text to give.
                let kind = params["type"].as_str().unwrap_or("alert");
                let accept = kind != "prompt";
                let _ = cdp
                    .call("Page.handleJavaScriptDialog", json!({ "accept": accept }))
                    .await;
                shared.logs.note(format!(
                    "A {kind} dialog appeared and was {}: \"{}\"",
                    if accept { "accepted" } else { "dismissed" },
                    params["message"].as_str().unwrap_or_default()
                ));
            }
            "Runtime.executionContextsCleared" => {
                *shared.world.lock().unwrap() = None;
            }
            "Runtime.consoleAPICalled" => {
                let args = params["args"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let level = params["type"].as_str().unwrap_or("log");
                shared.logs.console(level, console_text(args));
                if matches!(level, "error" | "assert") {
                    shared.changed();
                }
            }
            "Runtime.exceptionThrown" => {
                let details = &params["exceptionDetails"];
                let text = details["exception"]["description"]
                    .as_str()
                    .or_else(|| details["text"].as_str())
                    .unwrap_or("an exception");
                shared.logs.console("exception", text.to_string());
                shared.changed();
            }
            "Network.requestWillBeSent" => {
                let kind = params["type"].as_str().unwrap_or("Other");
                if kind == "Document" && is_main(&params["frameId"]) {
                    shared.logs.new_page();
                    document = Some((
                        params["requestId"].as_str().unwrap_or_default().to_string(),
                        params["request"]["url"].as_str().unwrap_or_default().to_string(),
                    ));
                    shared.changed();
                }
                shared.logs.request(NetworkEntry {
                    id: params["requestId"].as_str().unwrap_or_default().into(),
                    method: params["request"]["method"].as_str().unwrap_or("GET").into(),
                    url: params["request"]["url"].as_str().unwrap_or_default().into(),
                    kind: kind.into(),
                    status: None,
                    failed: None,
                });
            }
            "Network.responseReceived" => {
                let status = params["response"]["status"].as_u64().unwrap_or(0);
                shared.logs.response(
                    params["requestId"].as_str().unwrap_or_default(),
                    u16::try_from(status).unwrap_or(0),
                );
            }
            "Network.loadingFailed" => {
                let why = if params["canceled"].as_bool() == Some(true) {
                    "canceled".to_string()
                } else {
                    params["errorText"].as_str().unwrap_or("failed").to_string()
                };
                shared
                    .logs
                    .failure(params["requestId"].as_str().unwrap_or_default(), why.clone());
                if let Some((_, url)) = document.as_ref().filter(|(id, _)| params["requestId"] == id.as_str()) {
                    if why != "canceled" && why != "net::ERR_ABORTED" {
                        fail(&shared, url, classify(&why));
                    }
                }
            }
            "yaca.title" => {
                shared.location.lock().unwrap().title =
                    params["title"].as_str().unwrap_or_default().into();
                shared.changed();
            }
            "yaca.zoom" => {
                if let Some(percent) = params["percent"].as_u64() {
                    shared.location.lock().unwrap().zoom = percent as u32;
                    shared.changed();
                }
            }
            "yaca.favicon" => {
                {
                    let mut location = shared.location.lock().unwrap();
                    if location.url != params["url"] {
                        continue;
                    }
                    location.favicon = params["icon"].as_str().map(str::to_string);
                }
                shared.changed();
            }
            "yaca.blocked" => {
                fail(&shared, params["url"].as_str().unwrap_or_default(), PageError::Policy);
            }
            "yaca.open" => {
                if let (Some(browser), Some(url)) = (shared.browser.upgrade(), params["url"].as_str()) {
                    let agent = params["agent"].as_bool() == Some(true);
                    let url = url.to_string();
                    tokio::spawn(async move {
                        let _turn = browser.calls.lock().await;
                        if agent && browser.paused.load(Ordering::SeqCst) {
                            return;
                        }
                        let _busy = agent.then(|| browser.busy());
                        if browser.new_tab().is_ok() {
                            browser.apply().await;
                            if let Ok(engine) = browser.engine().await {
                                let _ = engine.navigate(&url, LOAD_LIMIT).await;
                            }
                        }
                    });
                }
            }
            "yaca.download" => {
                shared.logs.note(format!(
                    "The page tried to download {}; downloads are blocked in this browser.",
                    params["url"].as_str().unwrap_or("a file")
                ));
            }
            _ => {}
        }
    }
}

/// Whether there is a page behind or ahead, which the column's arrows show.
async fn history(shared: &Shared, cdp: &Cdp) {
    if let Ok(history) = cdp.call("Page.getNavigationHistory", json!({})).await {
        let at = history["currentIndex"].as_i64().unwrap_or(0);
        let count = history["entries"].as_array().map_or(0, Vec::len) as i64;
        let mut location = shared.location.lock().unwrap();
        location.can_go_back = at > 0;
        location.can_go_forward = at + 1 < count;
    }
    shared.changed();
}

pub(super) fn classify(error: &str) -> PageError {
    if error.contains("ERR_CERT") || error.contains("ERR_SSL") {
        PageError::Certificate
    } else if error.contains("ERR_BLOCKED") || error.contains("ERR_ACCESS_DENIED") {
        PageError::Policy
    } else {
        PageError::Network
    }
}

pub(super) fn fail(shared: &Shared, url: &str, error: PageError) {
    {
        let mut location = shared.location.lock().unwrap();
        // Chromium can send a generic loading failure after our policy denial.
        if location.error == Some(PageError::Policy) && location.url == url && !location.loading {
            return;
        }
        location.url = url.to_string();
        location.error = Some(error);
        location.title.clear();
        location.favicon = None;
        location.loading = false;
    }
    shared.stopped.send_replace(shared.started.load(Ordering::SeqCst));
    shared.changed();
}

async fn intercept(shared: std::sync::Arc<Shared>, cdp: Cdp, params: Value, top: bool, get_form: bool) {
    let request = &params["request"];
    let method = request["method"].as_str().unwrap_or_default();
    let raw_url = request["url"].as_str().unwrap_or_default();
    let submission = get_form || !matches!(method, "GET" | "HEAD" | "OPTIONS");
    let browser = shared.browser.upgrade();
    let agent = browser.as_ref().is_some_and(|browser| browser.busy.load(Ordering::SeqCst));
    let url = Url::parse(raw_url).ok();
    let allowed = if (top || submission) && agent {
        let from = Url::parse(&shared.location.lock().unwrap().url).ok().as_ref().and_then(origin);
        match (browser, url.as_ref()) {
            (Some(browser), Some(url)) => browser.authorize(url, from.as_deref(), method, submission).await,
            _ => false,
        }
    } else {
        !top || url.as_ref().is_some_and(crate::browser::allowed)
    };
    let id = &params["requestId"];
    if allowed {
        let _ = cdp.call("Fetch.continueRequest", json!({ "requestId": id })).await;
    } else {
        if top {
            fail(&shared, raw_url, PageError::Policy);
        }
        let _ = cdp.call("Fetch.failRequest", json!({ "requestId": id, "errorReason": "BlockedByClient" })).await;
    }
}
