//! The model router in the window: what `routing.json` amounts to, its text for
//! the editor, and the model it adds to the switcher.
//!
//! The rules live in `yaca_core::router`; the CLI reads the same file through
//! the same code, so a router configured here answers from there too.

use serde::Serialize;
use yaca_core::provider::registry::Providers;
use yaca_core::router::{
    attach::{self, Status},
    file,
};
use yaca_core::usage::ModelInfo;

use crate::config_dir;
use crate::providers::announce;

/// What the model switcher and the settings section are told. `Broken` is a
/// state of its own, not `Absent`: a router that quietly leaves the switcher
/// because of a typo is a setting the window lies about (§0).
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum RoutingStatus {
    Absent,
    Ready { model: ModelInfo },
    Broken { error: String },
}

impl From<Status> for RoutingStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Absent => RoutingStatus::Absent,
            Status::Ready(model) => RoutingStatus::Ready { model },
            Status::Broken(error) => RoutingStatus::Broken { error },
        }
    }
}

/// The router as the switcher lists it, when it is usable.
pub fn offered() -> Option<ModelInfo> {
    let dir = config_dir().ok()?;
    let providers = Providers::load(&dir).ok()?;
    match attach::status(&dir, &providers) {
        Status::Ready(model) => Some(model),
        Status::Absent | Status::Broken(_) => None,
    }
}

/// The router's model for a project or chat that names the router, or the
/// reason it cannot be had. Unlike [`offered`] this is an error and not a
/// shrug: a chat that asked for the router must not start on something else.
pub fn require() -> Result<ModelInfo, String> {
    let dir = config_dir()?;
    let providers = Providers::load(&dir).map_err(|e| e.to_string())?;
    match attach::status(&dir, &providers) {
        Status::Ready(model) => Ok(model),
        Status::Broken(error) => Err(error),
        Status::Absent => Err(format!(
            "маршрутизатор выбран, но {} не найден: создайте его в настройках",
            file::FILE
        )),
    }
}

#[tauri::command]
pub fn routing_status() -> Result<RoutingStatus, String> {
    let dir = config_dir()?;
    let providers = Providers::load(&dir).map_err(|e| e.to_string())?;
    Ok(attach::status(&dir, &providers).into())
}

/// The file as it is on disk, for the editor. `None` when there is none.
#[tauri::command]
pub fn routing_text() -> Result<Option<String>, String> {
    let path = config_dir()?.join(file::FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("не прочитать {}: {e}", path.display())),
    }
}

/// Checks the text and writes it down. A text that does not parse, or that
/// points at a provider or model the list does not have, is refused with the
/// words the file would be refused with, and nothing is written: a config that
/// fails the start must not reach the disk from a button.
#[tauri::command]
pub fn save_routing(app: tauri::AppHandle, text: String) -> Result<RoutingStatus, String> {
    let dir = config_dir()?;
    let providers = Providers::load(&dir).map_err(|e| e.to_string())?;

    let cfg = file::parse(&text, file::FILE).map_err(|e| e.to_string())?;
    file::validate(&cfg, &providers).map_err(|e| e.to_string())?;
    file::save(&dir, &cfg).map_err(|e| e.to_string())?;

    // The switcher lists the router from this file: it has to hear about it.
    announce(&app);
    Ok(attach::status(&dir, &providers).into())
}

/// Takes routing away: the file goes, and the router leaves the switcher.
#[tauri::command]
pub fn remove_routing(app: tauri::AppHandle) -> Result<(), String> {
    let path = config_dir()?.join(file::FILE);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("не удалить {}: {e}", path.display())),
    }
    announce(&app);
    Ok(())
}
