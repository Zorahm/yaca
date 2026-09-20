//! OS notification delivery when the agent asks for permission while the
//! window is not focused (§15).
//!
//! The core is headless and knows nothing about notifications: it only emits
//! `PermissionRequested`, and the shell decides on its own whether to reach
//! the human who is currently looking somewhere else.
//!
//! `notify-rust` directly, not `tauri-plugin-notification`: on desktop the
//! plugin only shows the toast and throws the interaction away — its action
//! API lives on mobile. A toast you cannot click does not solve the problem.

use yaca_core::event::AgentEvent;
use yaca_core::permission::PermissionRequest;

use tauri::{AppHandle, Manager};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;

/// Toast summary.
///
/// Text a human reads is Russian: the shell has no access to the `src/i18n`
/// modules (they live in the frontend), and every human-readable string in
/// `src-tauri` is already Russian — command errors included.
const TITLE: &str = "Yaca";

/// Toast body. `target` of a request is short by construction — a file name,
/// the first word of a command or a domain (§5.7) — exactly one toast line.
const BODY: &str = "Агент запрашивает разрешение";

/// Watcher of a single chat: subscribed to the same broadcast stream as the
/// bridge — a second subscriber, not a fork inside the bridge, which is
/// about transport.
///
/// Window focus is queried at the moment of the event, not tracked: going
/// through `WindowEvent::Focused` would accumulate a race between the
/// window switch and a tool call, while asking the window itself at event
/// time always answers with the current truth.
pub async fn watch(app: AppHandle, mut events: Receiver<AgentEvent>, stop: CancellationToken) {
    loop {
        tokio::select! {
            _ = stop.cancelled() => break,

            received = events.recv() => match received {
                Ok(event) => {
                    let request = unattended(focused(&app), &event);
                    if let Some(request) = request {
                        push(&app, &request.target);
                    }
                }
                // A lagged subscriber, same as in the bridge: losing the
                // questions of one session is better than living with a
                // delay that can never be caught up.
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            },
        }
    }
}

/// Decides whether an event warrants a notification: a pure function of two
/// pieces of data — window focus and the kind of event. Separated from
/// delivery so the test argues with it, not with the OS.
///
/// `Some` carries the request itself: its `target` is what the human reads
/// on the toast.
fn unattended(focused: bool, event: &AgentEvent) -> Option<&PermissionRequest> {
    if focused {
        return None;
    }
    match event {
        AgentEvent::PermissionRequested { request } => Some(request),
        _ => None,
    }
}

/// Whether the main window is focused. The window answers for itself; not
/// open — the human cannot see it, and the toast is justified.
fn focused(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false)
}

/// Shows the toast and waits for the OS to report back about it.
///
/// `show()` talks to the OS synchronously, hence a blocking thread: a slow
/// notification daemon must not press down the asynchronous runtime pool.
fn push(app: &AppHandle, target: &str) {
    let app = app.clone();
    let body = format!("{BODY}{target}");
    // Identifier from `tauri.conf.json`: on Windows the NSIS and WiX
    // installers stamp it onto the Start-menu shortcut, and a toast under
    // the same AppUserModelID wears the app icon instead of the process
    // name. Under `tauri dev` no AUMID is set, so the toast travels under
    // the harness identity — expected, not a defect (§15).
    let identifier = app.config().identifier.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let mut binding = notify_rust::Notification::new();
        let mut toast = binding.summary(TITLE).body(&body);
        #[cfg(windows)]
        {
            toast = toast.app_id(&identifier);
        }
        #[cfg(not(windows))]
        let _ = identifier;

        match toast.show() {
            Ok(handle) => handle.wait_for_action(|action| {
                // Windows names the click on the toast body "default";
                // everything else is dismissal and other outcomes whose
                // window should not be raised for.
                if action == "default" {
                    raise(&app);
                }
            }),
            // No daemon — quiet fallback: a Linux box without a
            // notification daemon must not bring the shell down.
            Err(e) => eprintln!("notification not delivered: {e}"),
        }
    });
}

/// Raises the window above the rest: unminimizes, shows and puts in focus.
fn raise(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaca_core::event::{StopReason, TsMillis};
    use yaca_core::permission::PermissionKind;

    /// A request of the same shape the core emits live.
    fn request(target: &str) -> PermissionRequest {
        PermissionRequest {
            id: "r1".into(),
            call_id: "c1".into(),
            kind: PermissionKind::RunCommand,
            target: target.into(),
            detail: String::new(),
            note: None,
            size_bytes: None,
            files: None,
            scopes: Vec::new(),
            position: 1,
            total: 1,
        }
    }

    fn permission(target: &str) -> AgentEvent {
        AgentEvent::PermissionRequested {
            request: request(target),
        }
    }

    /// Window minimized or unfocused — a permission request makes noise.
    #[test]
    fn unfocused_request_notifies() {
        assert!(unattended(false, &permission("cargo test")).is_some());
    }

    /// The window is being watched — the inline block is seen on its own,
    /// a toast would be a duplicate.
    #[test]
    fn focused_request_stays_silent() {
        assert!(unattended(true, &permission("cargo test")).is_none());
    }

    /// Only a permission request: other events require no noise even
    /// unfocused — including a failed turn the feed already shouts about.
    #[test]
    fn other_events_stay_silent_unfocused() {
        let events = [
            AgentEvent::TurnStarted {
                turn_id: "t".into(),
                at: TsMillis(0),
            },
            AgentEvent::TurnFinished {
                turn_id: "t".into(),
                stop_reason: StopReason::EndTurn,
            },
            AgentEvent::TurnCancelled { turn_id: "t".into() },
            AgentEvent::Error {
                code: "e".into(),
                message: "текст".into(),
                retryable: true,
            },
        ];
        for event in &events {
            assert!(unattended(false, event).is_none(), "{event:?}");
        }
    }

    /// The toast carries what the human will read — the request target.
    #[test]
    fn toast_carries_request_target() {
        assert_eq!(
            unattended(false, &permission("git commit")).unwrap().target,
            "git commit"
        );
    }
}
