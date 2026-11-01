//! The phone client's server, as the window runs it: when it starts, when it stops,
//! and the commands that report and change it.
//!
//! The server is off unless the human turns it on, and its settings and paired
//! devices live in `<config>/remote` beside the certificate. Both are read from
//! disk on every command, so the window never reports a copy older than the file.
//! The window has no switch for the server yet: the commands below exist and the
//! screen that calls them does not (see the "Phone client" section of the README).

mod host;

use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;
use yaca_remote::devices::{DeviceSummary, Devices};
use yaca_remote::host::Host;
use yaca_remote::pairing::PairingOffer;
use yaca_remote::protocol::DeviceId;
use yaca_remote::settings::{remote_dir, RemoteSettings};
use yaca_remote::{RemoteServer, ServerOptions};

use crate::config_dir;

/// Every address, so a phone on the same network can reach the desktop. A server
/// bound to loopback would be reachable from this machine only, which is no use
/// to the phone it is for.
const BIND: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);

/// A bind that finds its port taken is retried this many times, this far apart.
/// The listener of a server that was just stopped is closed by its accept loop,
/// which notices the stop a moment after `stop` returns; a bind right after it
/// would otherwise fail on a port the server itself is letting go of.
const BIND_ATTEMPTS: u32 = 10;
const BIND_RETRY: Duration = Duration::from_millis(100);

/// The server the shell runs, if any, and why the last start failed.
///
/// One lock for both, held across a start: two commands restarting the server at
/// once would both bind the port, and the loser would report a port it could
/// never have had.
#[derive(Default)]
pub struct RemoteState(tokio::sync::Mutex<Running>);

#[derive(Default)]
struct Running {
    server: Option<RemoteServer>,
    /// Why the server the settings ask for is not running. Cleared by the next
    /// start, whatever its outcome.
    error: Option<String>,
}

/// What the window shows about the server: its settings, whether it runs, where
/// it listens, and the paired devices. The devices are listed while the server is
/// off, so a phone can be revoked without turning it on.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemoteStatus {
    pub enabled: bool,
    pub port: u16,
    pub running: bool,
    /// The address the server listens on, as the socket reports it.
    pub address: Option<String>,
    /// The SHA-256 of the certificate, which the pairing code carries.
    pub fingerprint: Option<String>,
    /// Why the server is not running although it is enabled.
    pub error: Option<String>,
    pub devices: Vec<DeviceSummary>,
}

/// The directory of the server's files, `<config>/remote`.
fn directory() -> Result<PathBuf, String> {
    Ok(remote_dir(&config_dir()?))
}

/// Starts the server on `port`, retrying a bind that finds the port taken for a
/// moment (see [`BIND_ATTEMPTS`]).
async fn listen(app: &AppHandle, dir: &Path, port: u16) -> Result<RemoteServer, String> {
    let options = ServerOptions {
        dir: dir.to_path_buf(),
        port,
        bind: BIND,
    };
    let host: Arc<dyn Host> = Arc::new(host::DesktopHost::new(app.clone()));
    let mut attempts = 0;
    loop {
        match RemoteServer::start(options.clone(), Arc::clone(&host)).await {
            Ok(server) => return Ok(server),
            Err(yaca_remote::Error::Io(e))
                if e.kind() == std::io::ErrorKind::AddrInUse && attempts < BIND_ATTEMPTS =>
            {
                attempts += 1;
                tokio::time::sleep(BIND_RETRY).await;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// Stops the running server, if any, and starts the one `settings` ask for. The
/// outcome is kept in `running`: the server, or the reason it did not start. A
/// failure does not fail the caller; the window reads it from the status.
async fn apply(running: &mut Running, app: &AppHandle, dir: &Path, settings: &RemoteSettings) {
    if let Some(old) = running.server.take() {
        old.stop();
    }
    running.error = None;
    if !settings.enabled {
        return;
    }
    match listen(app, dir, settings.port).await {
        Ok(server) => running.server = Some(server),
        Err(e) => {
            eprintln!("remote server did not start: {e}");
            running.error = Some(e);
        }
    }
}

/// Starts the server the saved settings ask for. Called from the window's setup
/// and not awaited there: the start is async, and the window comes up either way.
pub fn start_saved(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Without a configuration directory there are no saved settings to start
        // from, and the window's own calls report the same missing directory.
        let Ok(dir) = directory() else {
            return;
        };
        let settings = RemoteSettings::load(&dir);
        if !settings.enabled {
            return;
        }
        let state = app.state::<RemoteState>();
        let mut running = state.0.lock().await;
        apply(&mut running, &app, &dir, &settings).await;
    });
}

fn status(dir: &Path, settings: &RemoteSettings, running: &Running) -> RemoteStatus {
    let server = running.server.as_ref();
    RemoteStatus {
        enabled: settings.enabled,
        port: settings.port,
        running: server.is_some(),
        address: server.map(|s| s.local_addr().to_string()),
        fingerprint: server.map(|s| s.fingerprint().to_string()),
        error: running.error.clone(),
        devices: Devices::load(dir).list(),
    }
}

#[tauri::command]
pub async fn remote_status(state: State<'_, RemoteState>) -> Result<RemoteStatus, String> {
    let dir = directory()?;
    let running = state.0.lock().await;
    Ok(status(&dir, &RemoteSettings::load(&dir), &running))
}

/// Turns the server on or off, on a port, and saves that choice. A running server
/// is stopped first, so a changed port takes effect at once.
#[tauri::command]
pub async fn set_remote(
    app: AppHandle,
    state: State<'_, RemoteState>,
    enabled: bool,
    port: u16,
) -> Result<RemoteStatus, String> {
    // Port 0 binds some free port on every start, and the saved setting would never
    // name the one a phone paired with.
    if port == 0 {
        return Err("the port must be between 1 and 65535".to_string());
    }
    let dir = directory()?;
    let settings = RemoteSettings { enabled, port };
    let mut running = state.0.lock().await;
    settings.save(&dir).map_err(|e| e.to_string())?;
    apply(&mut running, &app, &dir, &settings).await;
    Ok(status(&dir, &settings, &running))
}

/// A one-time code for a phone to pair with, as a URI and a QR code. Each call
/// replaces the previous code, so only the one on screen works.
#[tauri::command]
pub async fn remote_pairing(state: State<'_, RemoteState>) -> Result<PairingOffer, String> {
    let running = state.0.lock().await;
    let server = running
        .server
        .as_ref()
        .ok_or("the remote server is not running")?;
    server.offer_pairing().map_err(|e| e.to_string())
}

/// Unpairs a device. Its open connections are closed at once when the server is
/// running; with the server off, the list alone is changed.
#[tauri::command]
pub async fn revoke_device(
    state: State<'_, RemoteState>,
    id: String,
) -> Result<RemoteStatus, String> {
    let dir = directory()?;
    let device = DeviceId(id.clone());
    let running = state.0.lock().await;
    let removed = match &running.server {
        Some(server) => server.revoke(&device),
        None => Devices::load(&dir).revoke(&device),
    }
    .map_err(|e| e.to_string())?;
    if !removed {
        return Err(format!("no paired device {id}"));
    }
    Ok(status(&dir, &RemoteSettings::load(&dir), &running))
}
