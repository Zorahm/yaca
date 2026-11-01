//! What a new chat starts with: the bottom row of §7.1 on the home screen.
//!
//! Two layers. The project's own settings are the base (§10), and the last
//! choice the human made on the home screen goes over them, in any project:
//! picking a model and an effort is a choice about how one works, and asking
//! for it again after every restart would be the window forgetting it.
//!
//! The last choice lives in the shell's `settings.json` ([`AppSettings`]), not
//! in the project's file. The core reads that file, and it belongs to the
//! project: a pick in the window is not an edit of somebody's repository.

use yaca_core::provider::registry::Providers;
use yaca_core::router::ROUTER_ID;
use yaca_core::{Effort, Mode, SessionConfig};

use crate::settings::AppSettings;
use crate::{config_dir, routing};

/// What a session starts with when it does not exist yet.
///
/// `setMode` and its neighbours are session commands, but the home screen
/// composer (§9.1) picks all of this before there is a session. A type of its
/// own, not three arguments: they always travel together and always mean one
/// thing — "the person chose before the first word".
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Startup {
    pub mode: Mode,
    pub provider: String,
    pub model: String,
    pub effort: Effort,
}

/// What a new chat in the project would start with: the base for the bottom
/// row of §7.1 on the home screen.
///
/// Without this the composer would show the built-in defaults and silently
/// overwrite `.yaca/settings.json` with them — §10 requires the reverse order:
/// the project's settings are the base, and what is chosen in the window is
/// laid over them.
///
/// The pair is settled against the provider list here, not only at the first
/// message: a project that names nothing starts on the first provider that
/// answered, and the composer should show that model rather than an empty
/// chip. A remembered pair that no longer settles — the provider was deleted,
/// the router file broke — gives way to the project's. Neither settles — the
/// error says so, and the home screen does not send.
#[tauri::command]
pub fn project_startup(path: String) -> Result<Startup, String> {
    let config = SessionConfig::open(&path).map_err(|e| e.to_string())?;
    let last = AppSettings::load(&config_dir()?).last_startup().cloned();

    let (mode, effort) = match &last {
        Some(last) => (last.mode, last.effort),
        None => (config.mode, config.effort),
    };
    let (provider, model) = match last.and_then(|last| settle(&last.provider, &last.model).ok()) {
        Some(pair) => pair,
        None => settle(&config.provider, &config.model)?,
    };
    Ok(Startup {
        mode,
        provider,
        model,
        effort,
    })
}

/// Remembers the home screen's choice for the next new chat, in any project.
#[tauri::command]
pub fn remember_startup(startup: Startup) -> Result<(), String> {
    let dir = config_dir()?;
    let mut settings = AppSettings::load(&dir);
    settings.set_last_startup(startup);
    settings
        .save(&dir)
        .map_err(|e| format!("could not write settings: {e}"))
}

/// The provider and model a chat would really get for this pair.
fn settle(provider: &str, model: &str) -> Result<(String, String), String> {
    // The router is not in the provider list, it is made of it: a pair that
    // names it is settled against `routing.json`, or the home screen would
    // show an error for a choice that is perfectly valid.
    if provider == ROUTER_ID {
        return Ok((ROUTER_ID.to_string(), routing::require()?.id));
    }
    let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;
    let (spec, model) = providers
        .resolve(provider, model)
        .map_err(|e| e.to_string())?;
    Ok((spec.id.clone(), model))
}
