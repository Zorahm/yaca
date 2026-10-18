//! Update check: by hand from the About screen and on its own in the
//! background.
//!
//! The question and the choice of the answer are in `yaca_core::update`; this
//! is the shell part: when to ask, where the answer is kept and who is told.
//!
//! The answer is kept here and broadcast, as preferences are, because the About
//! screen lives in the settings window and the notice bar in the main one.
//! They must show the same thing (§0), and neither can ask the other.
//!
//! Nothing is downloaded or installed. A new release is shown with its
//! changelog and a link; the person decides what to do with it.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use yaca_core::update::{self, UpdateCheck};

use crate::state::AppState;

/// The channel a fresh answer arrives on. Both windows listen.
pub const UPDATE_CHANNEL: &str = "yaca://update";

/// The version of this build: the workspace version, the one the installer and
/// the About screen show.
const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Wait after start before the first background check: the window is busy
/// opening chats, and a request to GitHub is not worth competing for it.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(20);

/// Gap between background checks. GitHub allows sixty anonymous requests an
/// hour per address; one every six hours leaves the rest to everything else
/// behind the same address.
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// Asks GitHub, keeps the answer, tells every window.
async fn ask(app: &AppHandle) -> Result<UpdateCheck, String> {
    let check = update::check(CURRENT).await.map_err(|e| e.to_string())?;
    app.state::<AppState>().set_update(check.clone());
    let _ = app.emit(UPDATE_CHANNEL, check.clone());
    Ok(check)
}

/// The button. Always asks, whatever the automatic-check setting says; the
/// error is a code for the window to put in words (`UpdateError`).
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateCheck, String> {
    ask(&app).await
}

/// What the last check found, or `None` before the first one finished.
#[tauri::command]
pub fn update_status(state: tauri::State<'_, AppState>) -> Option<UpdateCheck> {
    state.update()
}

/// The background check. The setting is read on every round, so turning it
/// off takes effect without a restart, and turning it on does not wait for one.
/// A failed round is silent: nobody asked, and the next round tries again.
pub fn watch(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_AFTER).await;
        loop {
            if app.state::<AppState>().prefs().check_updates {
                let _ = ask(&app).await;
                tokio::time::sleep(EVERY).await;
            } else {
                // Off: look again soon, so switching it on is noticed.
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }
    });
}
