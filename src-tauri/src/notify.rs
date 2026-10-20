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
use yaca_core::question::QuestionRequest;

use tauri::{AppHandle, Manager};
use yaca_host::PrefsHandle;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;

/// Toast summary.
///
/// Text a human reads is English: the shell has no access to the `src/i18n`
/// modules (they live in the frontend), so every human-readable string in
/// `src-tauri` is a plain English literal — command errors included.
const TITLE: &str = "Yaca";

/// Toast body up to the target, separator included: the target glues right
/// after it — "The agent asks for permission: git commit". `target` of a
/// request is short by construction — a file name, the first word of a
/// command or a domain (§5.7) — exactly one toast line.
const BODY: &str = "The agent asks for permission: ";

/// The toast's full text: the sentence and the target after the separator.
fn toast_body(target: &str) -> String {
    format!("{BODY}{target}")
}

/// Toast text for a question: the sentence, then the question itself.
const QUESTION_BODY: &str = "The agent asked a question: ";

fn question_toast_body(question: &str) -> String {
    format!("{QUESTION_BODY}{question}")
}

/// Watcher of a single chat: subscribed to the same broadcast stream as the
/// bridge — a second subscriber, not a fork inside the bridge, which is
/// about transport.
///
/// Window focus is queried at the moment of the event, not tracked: going
/// through `WindowEvent::Focused` would accumulate a race between the
/// window switch and a tool call, while asking the window itself at event
/// time always answers with the current truth.
pub async fn watch(app: AppHandle, mut events: Receiver<AgentEvent>, stop: CancellationToken, prefs: PrefsHandle) {
    loop {
        tokio::select! {
            _ = stop.cancelled() => break,

            received = events.recv() => match received {
                Ok(event) => {
                    // Both asked at event time rather than tracked: the
                    // setting can be switched in the other window mid-turn,
                    // and a copy taken at subscribe time would be stale.
                    let asked = unattended(prefs.get().notifications, focused(&app), &event);
                    if let Some(asked) = asked {
                        push(&app, &asked.body());
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

/// Decides whether an event warrants a notification: a pure function of three
/// pieces of data — the setting (§10), window focus and the kind of event.
/// Separated from delivery so the test argues with it, not with the OS.
///
/// `Some` carries the request itself: its target, or its question, is what
/// the human reads on the toast.
fn unattended(enabled: bool, focused: bool, event: &AgentEvent) -> Option<Asked<'_>> {
    if !enabled || focused {
        return None;
    }
    match event {
        AgentEvent::PermissionRequested { request } => Some(Asked::Permission(request)),
        // A question holds the turn until someone answers it, as a permission
        // does, so the same toast is what brings the human back.
        AgentEvent::QuestionAsked { request } => Some(Asked::Question(request)),
        _ => None,
    }
}

/// What the human is being asked, as far as the toast needs to know it.
enum Asked<'a> {
    Permission(&'a PermissionRequest),
    Question(&'a QuestionRequest),
}

impl Asked<'_> {
    fn body(&self) -> String {
        match self {
            Asked::Permission(request) => toast_body(&request.target),
            Asked::Question(request) => question_toast_body(&request.question),
        }
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
fn push(app: &AppHandle, body: &str) {
    let app = app.clone();
    let body = body.to_string();
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
        // Minimizing exists only on desktop; Android has no such window state.
        #[cfg(desktop)]
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
            server: None,
            id: "r1".into(),
            call_id: "c1".into(),
            kind: PermissionKind::RunCommand,
            target: target.into(),
            detail: String::new(),
            note: None,
            size_bytes: None,
            files: None,
            scopes: Vec::new(),
            invariant: None,
            unsaved: None,
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
        assert!(unattended(true, false, &permission("cargo test")).is_some());
    }

    /// The window is being watched — the inline block is seen on its own,
    /// a toast would be a duplicate.
    #[test]
    fn focused_request_stays_silent() {
        assert!(unattended(true, true, &permission("cargo test")).is_none());
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
                at: TsMillis(0),
            },
            AgentEvent::TurnCancelled {
                turn_id: "t".into(),
                at: TsMillis(0),
            },
            AgentEvent::Error {
                code: "e".into(),
                message: "text".into(),
                retryable: true,
            },
        ];
        for event in &events {
            assert!(unattended(true, false, event).is_none(), "{event:?}");
        }
    }

    /// Switched off in §10 — silent even unfocused. The inline block in the
    /// feed still waits; only the toast goes away.
    #[test]
    fn a_switched_off_setting_silences_an_unfocused_request() {
        assert!(unattended(false, false, &permission("cargo test")).is_none());
    }

    /// The toast carries what the human will read — the request target.
    #[test]
    fn toast_carries_request_target() {
        assert_eq!(
            unattended(true, false, &permission("git commit"))
                .unwrap()
                .body(),
            toast_body("git commit")
        );
    }

    /// A question that waits unseen gets a toast that names the question.
    #[test]
    fn an_unfocused_question_notifies_with_its_text() {
        let event = AgentEvent::QuestionAsked {
            request: QuestionRequest {
                id: "q1".into(),
                call_id: "c1".into(),
                question: "Which path?".into(),
                options: Vec::new(),
                allow_other: true,
                multi_select: false,
            },
        };
        let asked = unattended(true, false, &event).expect("no toast for a question");
        assert!(asked.body().ends_with("Which path?"));
    }

    /// The body reads as a sentence and then the target: the separator keeps
    /// them from gluing into one word.
    #[test]
    fn toast_body_separates_sentence_and_target() {
        let body = toast_body("src/main.rs");
        assert!(body.starts_with("The agent asks for permission"));
        assert!(body.ends_with("src/main.rs"));
        assert!(body.contains(": "), "no separator between the parts: {body}");
    }
}
