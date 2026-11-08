//! Browser chrome data exposed by WebView2 itself.

use base64::Engine as _;
use serde_json::json;
use tauri::Webview;
use tokio::sync::mpsc;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2_15, COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG,
};
use webview2_com::{
    take_pwstr, FaviconChangedEventHandler, GetFaviconCompletedHandler, ZoomFactorChangedEventHandler,
};
use windows::core::{Interface, PWSTR};

use super::cdp::Event;
use super::Result;

pub(super) fn watch(webview: &Webview, events: mpsc::UnboundedSender<Event>) -> Result<()> {
    webview.with_webview(move |platform| {
        let controller = platform.controller();
        let zoom = events.clone();
        let zoom_changed = ZoomFactorChangedEventHandler::create(Box::new(move |controller, _| {
            if let Some(controller) = controller {
                let mut factor = 1.0;
                // SAFETY: WebView2 invokes this callback on its owning UI thread.
                unsafe { controller.ZoomFactor(&mut factor)? };
                let _ = zoom.send(("yaca.zoom", json!({ "percent": (factor * 100.0).round() as u32 })));
            }
            Ok(())
        }));
        // SAFETY: with_webview runs this closure on the owning UI thread.
        unsafe {
            let _ = controller.add_ZoomFactorChanged(&zoom_changed, &mut 0);
            if let Ok(core) = controller.CoreWebView2().and_then(|core| core.cast::<ICoreWebView2_15>()) {
                let changed = FaviconChangedEventHandler::create(Box::new(move |core, _| {
                    if let Some(core) = core {
                        let mut source = PWSTR::null();
                        core.Source(&mut source)?;
                        let source = take_pwstr(source);
                        let events = events.clone();
                        let read = GetFaviconCompletedHandler::create(Box::new(move |status, stream| {
                            let icon = stream.filter(|_| status.is_ok()).and_then(|stream| {
                                // A fixed ceiling keeps an oversized site icon out of every state event.
                                let mut png = vec![0; 32 * 1024];
                                let mut len = 0;
                                let result = stream.Read(png.as_mut_ptr().cast(), png.len() as u32, Some(&mut len));
                                (result.is_ok() && len > 0 && (len as usize) < png.len()).then(|| {
                                    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png[..len as usize]))
                                })
                            });
                            let _ = events.send(("yaca.favicon", json!({ "url": source, "icon": icon })));
                            Ok(())
                        }));
                        core.cast::<ICoreWebView2_15>()?.GetFavicon(COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, &read)?;
                    }
                    Ok(())
                }));
                let _ = core.add_FaviconChanged(&changed, &mut 0);
            }
        }
    }).map_err(|e| e.to_string())
}
