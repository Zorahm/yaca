//! Installing the release the check found.
//!
//! The download, the signature and the install itself belong to
//! `tauri-plugin-updater`: it knows how the manifest names the installer of each
//! system, how to start it so the application comes back, and what a wrong
//! signature looks like. This file is the shell's part — which release, when to
//! refuse, and what the window is told while it happens.
//!
//! # Which release
//!
//! The manifest is addressed by the tag the check chose (`crate::update::manifest_url`),
//! not by `releases/latest`, which is a release without prereleases. The plugin
//! then takes any version the manifest announces, so the comparator is narrowed
//! to exactly the version the check found: a manifest naming an older, validly
//! signed build is refused before anything is downloaded.
//!
//! # What it does to the window
//!
//! On Windows the plugin starts the installer and ends the process; the
//! installer (`/P /UPDATE /R`, the passive mode of `tauri.conf.json`) puts the
//! new build in place and starts it again. Everywhere else the new build is on
//! disk once the call returns, and the restart is this module's.
//!
//! A turn that is running when this starts is interrupted by the restart. The
//! core keeps what was recorded, but the work of the half-finished call is
//! gone, so that is the person's decision: [`install_app_update`] refuses while
//! a chat is busy unless asked twice.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, Url};
use tauri_plugin_updater::{Error, UpdaterExt};
use yaca_core::update::ReleaseInfo;

use crate::state::AppState;

/// The channel the install reports on. Both windows listen.
pub const UPDATE_INSTALL_CHANNEL: &str = "yaca://update-install";

/// A download is a file of tens of megabytes over someone's connection: the ten
/// seconds a check gets would cut it short on anything but a fast line.
const TIMEOUT: Duration = Duration::from_secs(300);

/// Where the install has got. Kept in the shell and broadcast, like the answer
/// of the check: the card in the sidebar and the About screen show one fact.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct UpdateInstall {
    pub phase: InstallPhase,
    /// Bytes downloaded so far, for the bar.
    #[ts(type = "number")]
    pub received: u64,
    /// What the server said the file weighs, when it said it.
    #[ts(type = "number | null")]
    pub total: Option<u64>,
    /// Why the last attempt stopped, as a code the window puts in words
    /// (`offline`, `signature`, `no_artifact`, `install_failed`, `busy`, ...).
    pub error: Option<String>,
}

/// How far the install has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum InstallPhase {
    /// Nothing has been started in this run of the application.
    #[default]
    Idle,
    Downloading,
    /// The file is here and is being checked and put in place.
    Installing,
    /// Stopped, with `error` saying why.
    Failed,
}

/// The button. Downloads the release the last check found, verifies its
/// signature and installs it.
///
/// `force` is the second press of someone who was told that a turn is running.
#[tauri::command]
pub async fn install_app_update(app: AppHandle, force: bool) -> Result<(), String> {
    // A development build has no installed bundle to replace: the installer
    // would overwrite the release one on this machine instead.
    if cfg!(dev) {
        return refuse(&app, "dev_build");
    }

    let release = match app.state::<AppState>().update().and_then(|check| check.latest) {
        Some(release) => release,
        None => return refuse(&app, "nothing"),
    };

    if !force && running_turn(&app) {
        return refuse(&app, "busy");
    }

    let progress = Arc::new(Progress::default());
    publish(&app, progress.install(InstallPhase::Downloading));

    match download_and_install(&app, &release, &progress).await {
        Ok(()) => {}
        Err(code) => {
            let stopped = UpdateInstall {
                error: Some(code.clone()),
                ..progress.install(InstallPhase::Failed)
            };
            publish(&app, stopped);
            return Err(code);
        }
    }

    // Windows: the installer is already running, and the plugin ends this
    // process before it returns. Elsewhere the new build is on disk while the
    // old one is still running, and coming back is ours to do.
    restart(&app)
}

/// Coming back after the install: the installer does it on Windows, this process
/// does it everywhere else.
#[cfg(windows)]
fn restart(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
fn restart(app: &AppHandle) -> Result<(), String> {
    app.restart()
}

/// What the install is doing, for a window that was reloaded in the middle of
/// one — the same reason `update_status` exists.
#[tauri::command]
pub fn install_status(state: tauri::State<'_, AppState>) -> UpdateInstall {
    state.install()
}

/// A refusal is a state too: the window that shows it may be reloaded, and it
/// would otherwise come back offering an update that was just turned down for a
/// reason it no longer knows.
fn refuse(app: &AppHandle, code: &str) -> Result<(), String> {
    publish(
        app,
        UpdateInstall {
            phase: InstallPhase::Failed,
            error: Some(code.to_string()),
            ..UpdateInstall::default()
        },
    );
    Err(code.to_string())
}

/// Whether a chat is in the middle of a turn right now.
fn running_turn(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .live()
        .iter()
        .any(|(chat, _)| chat.state().busy)
}

async fn download_and_install(
    app: &AppHandle,
    release: &ReleaseInfo,
    progress: &Arc<Progress>,
) -> Result<(), String> {
    let expected = release.version.clone();
    let endpoint = Url::parse(&crate::update::manifest_url(&release.tag))
        .map_err(|_| "no_artifact".to_string())?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|_| "no_artifact".to_string())?
        .timeout(TIMEOUT)
        .version_comparator(move |_, remote| remote.version.to_string() == expected)
        .build()
        .map_err(code)?;

    // `None` is the comparator saying no: this manifest does not describe the
    // release the check offered.
    let Some(update) = updater.check().await.map_err(code)? else {
        return Err("no_artifact".into());
    };

    let chunks = progress.clone();
    let chunks_app = app.clone();
    let finished = progress.clone();
    let finished_app = app.clone();

    let mut reported = u64::MAX;
    update
        .download_and_install(
            move |chunk, size| {
                chunks.received.fetch_add(chunk as u64, Ordering::Relaxed);
                if let Some(size) = size {
                    chunks.total.store(size, Ordering::Relaxed);
                }
                // A message per chunk would be thousands for one bar.
                let percent = chunks.percent().unwrap_or(0);
                if percent == reported {
                    return;
                }
                reported = percent;
                publish(&chunks_app, chunks.install(InstallPhase::Downloading));
            },
            move || publish(&finished_app, finished.install(InstallPhase::Installing)),
        )
        .await
        .map_err(code)
}

/// Where the download has got, shared with the two callbacks the plugin calls:
/// one borrows the file's progress, the other reports the end of it.
#[derive(Default)]
struct Progress {
    received: AtomicU64,
    /// 0 while the server has not said how much the file weighs.
    total: AtomicU64,
}

impl Progress {
    fn install(&self, phase: InstallPhase) -> UpdateInstall {
        let total = self.total.load(Ordering::Relaxed);
        UpdateInstall {
            phase,
            received: self.received.load(Ordering::Relaxed),
            total: (total > 0).then_some(total),
            error: None,
        }
    }

    /// Whole percents, or nothing while the size is unknown.
    fn percent(&self) -> Option<u64> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0).then(|| self.received.load(Ordering::Relaxed) * 100 / total)
    }
}

fn publish(app: &AppHandle, install: UpdateInstall) {
    app.state::<AppState>().set_install(install.clone());
    let _ = app.emit(UPDATE_INSTALL_CHANNEL, install);
}

/// The plugin's failure as a code. The enum is `#[non_exhaustive]`, so the last
/// arm takes whatever a newer plugin adds.
fn code(error: Error) -> String {
    match error {
        Error::Reqwest(_) | Error::Network(_) => "offline".into(),
        Error::ReleaseNotFound
        | Error::Serialization(_)
        | Error::TargetNotFound(_)
        | Error::TargetsNotFound(_)
        | Error::EmptyEndpoints => "no_artifact".into(),
        Error::Minisign(_)
        | Error::Base64(_)
        | Error::SignatureUtf8(_)
        | Error::SignedVersionMismatch { .. } => "signature".into(),
        _ => "install_failed".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_without_a_manifest_is_not_a_network_failure() {
        // Releases published before the app could install itself carry no
        // `latest.json`; the card then offers the page, not a retry.
        assert_eq!(code(Error::ReleaseNotFound), "no_artifact");
        assert_eq!(
            code(Error::TargetNotFound("windows-x86_64-nsis".into())),
            "no_artifact"
        );
    }

    #[test]
    fn a_line_that_broke_is_the_network_and_a_bad_signature_is_the_file() {
        assert_eq!(code(Error::Network("timed out".into())), "offline");
        assert_eq!(
            code(Error::SignedVersionMismatch {
                signed: "1.0.0".into(),
                announced: "1.0.1".into(),
            }),
            "signature"
        );
        assert_eq!(code(Error::InvalidUpdaterFormat), "install_failed");
    }

    #[test]
    fn a_download_without_a_known_size_has_no_percent() {
        let progress = Progress::default();
        assert_eq!(progress.percent(), None, "nothing has arrived yet");

        progress.received.fetch_add(10, Ordering::Relaxed);
        assert_eq!(progress.percent(), None, "no total, no percentage");

        progress.total.store(100, Ordering::Relaxed);
        assert_eq!(progress.percent(), Some(10));
        assert_eq!(progress.install(InstallPhase::Downloading).total, Some(100));
    }

    #[test]
    fn an_idle_install_has_nothing_to_draw() {
        let idle = UpdateInstall::default();
        assert_eq!(idle.phase, InstallPhase::Idle);
        assert_eq!(idle.received, 0);
        assert_eq!(idle.total, None);
        assert_eq!(idle.error, None);
    }
}
