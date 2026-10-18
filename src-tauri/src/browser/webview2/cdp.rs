//! The DevTools Protocol of one WebView2, from any thread.
//!
//! WebView2 is a COM object of the UI thread: every call goes there through
//! `with_webview`, and the answer comes back on a channel. Events arrive on
//! the UI thread too and are handed to a channel as they come.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tauri::Webview;
use tokio::sync::{mpsc, oneshot};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2DevToolsProtocolEventReceivedEventArgs,
};
use webview2_com::{
    take_pwstr, CallDevToolsProtocolMethodCompletedHandler, DevToolsProtocolEventReceivedEventHandler,
};
use windows::core::{HSTRING, PWSTR};

use crate::browser::engine::Result;

/// A call the browser does not answer in this time is not going to be
/// answered: the page is frozen or the webview is gone.
const LIMIT: Duration = Duration::from_secs(45);

/// One DevTools event: its name and its parameters.
pub type Event = (&'static str, Value);

#[derive(Clone)]
pub struct Cdp {
    webview: Webview,
}

impl Cdp {
    pub fn new(webview: Webview) -> Cdp {
        Cdp { webview }
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let (tx, rx) = oneshot::channel::<Result<String>>();
        // Either the handler or the failed call answers, whichever comes.
        let tx = Arc::new(Mutex::new(Some(tx)));
        let answer = move |tx: &Arc<Mutex<Option<oneshot::Sender<Result<String>>>>>, result| {
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(result);
            }
        };
        let name = method.to_string();
        let body = params.to_string();
        let in_handler = tx.clone();
        self.webview
            .with_webview(move |platform| {
                let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
                    move |status, json: String| {
                        answer(
                            &in_handler,
                            match status {
                                Ok(()) => Ok(json),
                                Err(err) => Err(failure(&json, &err)),
                            },
                        );
                        Ok(())
                    },
                ));
                // SAFETY: on the UI thread, where `with_webview` runs us, as
                // WebView2 requires; the strings outlive the call.
                let called = unsafe {
                    platform.controller().CoreWebView2().and_then(|core| {
                        core.CallDevToolsProtocolMethod(&HSTRING::from(&name), &HSTRING::from(&body), &handler)
                    })
                };
                if let Err(err) = called {
                    answer(&tx, Err(err.message()));
                }
            })
            .map_err(|e| format!("the browser is gone: {e}"))?;

        let json = match tokio::time::timeout(LIMIT, rx).await {
            Ok(Ok(answer)) => answer?,
            Ok(Err(_)) => return Err("the browser is gone".into()),
            Err(_) => return Err(format!("the browser did not answer {method} in {} s", LIMIT.as_secs())),
        };
        serde_json::from_str(&json).map_err(|e| format!("{method}: unreadable answer: {e}"))
    }

    /// Hands every event of `names` to `sink`, from now until the webview goes.
    pub fn subscribe(&self, names: &'static [&'static str], sink: mpsc::UnboundedSender<Event>) -> Result<()> {
        self.webview
            .with_webview(move |platform| {
                // SAFETY: as in `call`: the UI thread, and COM objects that
                // the webview keeps alive for as long as it lives.
                let core: ICoreWebView2 = match unsafe { platform.controller().CoreWebView2() } {
                    Ok(core) => core,
                    Err(_) => return,
                };
                for &name in names {
                    let sink = sink.clone();
                    let handler = DevToolsProtocolEventReceivedEventHandler::create(Box::new(
                        move |_, args: Option<ICoreWebView2DevToolsProtocolEventReceivedEventArgs>| {
                            if let Some(args) = args {
                                let mut json = PWSTR::null();
                                // SAFETY: WebView2 allocates the string and
                                // `take_pwstr` frees it.
                                unsafe { args.ParameterObjectAsJson(&mut json)? };
                                let text = take_pwstr(json);
                                if let Ok(params) = serde_json::from_str(&text) {
                                    let _ = sink.send((name, params));
                                }
                            }
                            Ok(())
                        },
                    ));
                    let mut token = 0i64;
                    // SAFETY: see above.
                    let _ = unsafe {
                        core.GetDevToolsProtocolEventReceiver(&HSTRING::from(name))
                            .and_then(|receiver| receiver.add_DevToolsProtocolEventReceived(&handler, &mut token))
                    };
                }
            })
            .map_err(|e| format!("the browser is gone: {e}"))
    }
}

/// The protocol's own error, when its answer carries one.
fn failure(json: &str, err: &windows::core::Error) -> String {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|value| value["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| err.message())
}
