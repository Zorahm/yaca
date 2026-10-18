//! What the page tells the engine as it goes: where it is, whether it is
//! loading, what it logged, and the dialogs nobody is there to answer.

use std::sync::atomic::Ordering;
use std::sync::Weak;

use serde_json::{json, Value};
use tokio::sync::mpsc;

use super::cdp::{Cdp, Event};
use super::Shared;
use crate::browser::logs::{console_text, NetworkEntry};

/// Runs until the webview goes, which drops the last sender.
pub(crate) async fn follow(shared: Weak<Shared>, cdp: Cdp, mut events: mpsc::UnboundedReceiver<Event>) {
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
                shared.location.lock().unwrap().url = url;
                history(&shared, &cdp).await;
            }
            "Page.navigatedWithinDocument" if is_main(&params["frameId"]) => {
                shared.location.lock().unwrap().url = params["url"].as_str().unwrap_or_default().into();
                history(&shared, &cdp).await;
            }
            "Page.frameStartedLoading" if is_main(&params["frameId"]) => {
                shared.started.fetch_add(1, Ordering::SeqCst);
                shared.location.lock().unwrap().loading = true;
                (shared.changed)();
            }
            "Page.frameStoppedLoading" if is_main(&params["frameId"]) => {
                shared.stopped.send_modify(|n| *n += 1);
                shared.location.lock().unwrap().loading = false;
                (shared.changed)();
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
                let args = params["args"].as_array().map(Vec::as_slice).unwrap_or_default();
                shared
                    .logs
                    .console(params["type"].as_str().unwrap_or("log"), console_text(args));
            }
            "Runtime.exceptionThrown" => {
                let details = &params["exceptionDetails"];
                let text = details["exception"]["description"]
                    .as_str()
                    .or_else(|| details["text"].as_str())
                    .unwrap_or("an exception");
                shared.logs.console("exception", text.to_string());
            }
            "Network.requestWillBeSent" => {
                let kind = params["type"].as_str().unwrap_or("Other");
                if kind == "Document" && is_main(&params["frameId"]) {
                    shared.logs.new_page();
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
                shared.logs.failure(params["requestId"].as_str().unwrap_or_default(), why);
            }
            "yaca.title" => {
                shared.location.lock().unwrap().title = params["title"].as_str().unwrap_or_default().into();
                (shared.changed)();
            }
            "yaca.open" => {
                if let Some(url) = params["url"].as_str() {
                    let _ = cdp.call("Page.navigate", json!({ "url": url })).await;
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
    (shared.changed)();
}
