//! Providers and their keys (§10), and the models the bottom row offers (§7.1).

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use yaca_core::provider::models;
use yaca_core::provider::registry::{ProviderSpec, Providers};
use yaca_core::secrets::{self, KeySource};
use yaca_core::usage::ModelInfo;

use crate::config_dir;

/// Channel "the provider list has changed".
///
/// They are edited in the settings window (§10), and the model switcher (§7.1)
/// shows them in the main window. Without this channel the list there would stay
/// as it was until a restart — that is, the window would lie about the state (§0).
pub const PROVIDERS_CHANNEL: &str = "yaca://providers";

/// Tells the windows that the list has changed. A failed broadcast does not fail the write:
/// the disk already holds the truth, and a missed event is a stale list, not a
/// lost setting.
pub(crate) fn announce(app: &tauri::AppHandle) {
    use tauri::Emitter;
    let _ = app.emit(PROVIDERS_CHANNEL, ());
}

/// The list after a command that also asked a provider for its models.
///
/// The two are answered separately because they fail separately: a key that is
/// in the keychain and a provider that is in the file stay there when the
/// address then refuses, and reporting the refusal as the command's failure
/// would say the key was not stored.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
    providers: Providers,
    /// Why the provider did not answer, when it was asked and did not.
    poll_error: Option<String>,
}

/// Asks the saved provider and writes its answer down (`models::refresh`);
/// `saved` is the list to answer with if it refuses.
async fn ask(dir: &Path, saved: Providers, id: &str) -> Asked {
    match models::refresh(dir, id).await {
        Ok(providers) => Asked {
            providers,
            poll_error: None,
        },
        Err(e) => Asked {
            providers: saved,
            poll_error: Some(e.to_string()),
        },
    }
}

/// Provider list (§10).
#[tauri::command]
pub fn providers() -> Result<Providers, String> {
    Providers::load(&config_dir()?).map_err(|e| e.to_string())
}

/// Adds a provider or replaces one with the same name.
///
/// One saved without models is asked for them on the spot. That is the new
/// provider: its key is stored before it is in the list, so the question
/// `set_api_key` asks had nobody to go to yet.
#[tauri::command]
pub async fn save_provider(app: tauri::AppHandle, spec: ProviderSpec) -> Result<Asked, String> {
    let dir = config_dir()?;
    let id = spec.id.clone();
    let unasked = spec.models.is_empty();

    let mut providers = Providers::load(&dir).map_err(|e| e.to_string())?;
    providers.put(spec);
    providers.save(&dir).map_err(|e| e.to_string())?;

    let asked = if unasked {
        ask(&dir, providers, &id).await
    } else {
        Asked {
            providers,
            poll_error: None,
        }
    };
    announce(&app);
    Ok(asked)
}

#[tauri::command]
pub fn delete_provider(app: tauri::AppHandle, id: String) -> Result<Providers, String> {
    let dir = config_dir()?;
    let mut providers = Providers::load(&dir).map_err(|e| e.to_string())?;
    providers.remove(&id).map_err(|e| e.to_string())?;
    providers.save(&dir).map_err(|e| e.to_string())?;
    announce(&app);
    Ok(providers)
}

/// Asks a provider which models it serves (§10).
///
/// Takes the description from the form, not from the registry: the address and headers could only
/// have just been corrected, and the question has to go to the one being configured. The key
/// comes from the keychain under the same id; a provider that does not ask for a key does not need one.
///
/// The list is returned, not saved: until the person presses "Save",
/// nothing in the registry should change.
#[tauri::command]
pub async fn discover_models(spec: ProviderSpec) -> Result<Vec<ModelInfo>, String> {
    models::discover(&spec, models::stored_key(&spec).as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// §10: the key goes into the system keychain, not into a file and not into process memory.
///
/// The provider is always named: a guessed one would store the key under the wrong name.
///
/// A key stored for a provider in the list is the moment there is something to
/// ask it with, so it is asked here and not by whichever window sent the key.
/// The name may also be no provider at all — the web search key goes through
/// this command — and then there is nobody to ask.
#[tauri::command]
pub async fn set_api_key(
    app: tauri::AppHandle,
    provider: String,
    key: String,
) -> Result<Asked, String> {
    secrets::store_api_key(&provider, &key).map_err(|e| e.to_string())?;

    let dir = config_dir()?;
    let providers = Providers::load(&dir).map_err(|e| e.to_string())?;
    if providers.get(&provider).is_none() {
        return Ok(Asked {
            providers,
            poll_error: None,
        });
    }
    let asked = ask(&dir, providers, &provider).await;
    announce(&app);
    Ok(asked)
}

#[tauri::command]
pub fn forget_api_key(provider: String) -> Result<(), String> {
    secrets::forget_api_key(&provider).map_err(|e| e.to_string())
}

/// Where the key came from: the keychain, an environment variable, or nowhere.
///
/// Not the key itself. The settings screen has to show that the key is there and where it came from —
/// and a key that has gone to the webview cannot be fetched back from there.
///
/// A provider that needs no key at all answers `not_needed`: "no key"
/// would be a warning about nothing for a local gateway.
#[tauri::command]
pub fn api_key_source(provider: String) -> Result<&'static str, String> {
    let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;
    let spec = providers
        .get(&provider)
        .ok_or_else(|| format!("provider \"{provider}\" is not in the list"))?;
    if !spec.needs_key() {
        return Ok("not_needed");
    }
    Ok(match secrets::key_source(&spec.id) {
        KeySource::Keychain => "keychain",
        KeySource::Environment => "environment",
        KeySource::None => "none",
    })
}

/// Where the web search key came from (§10).
///
/// A separate command, not `api_key_source("tavily")`: Tavily is not a provider and is not
/// in the registry, so the shared command would answer "not in the list" about a key that
/// is quietly sitting in the keychain. It is written and forgotten through the same
/// `set_api_key` and `forget_api_key` — the storage is one for everyone.
#[tauri::command]
pub fn search_key_source() -> &'static str {
    match secrets::key_source(secrets::TAVILY_ITEM) {
        KeySource::Keychain => "keychain",
        KeySource::Environment => "environment",
        KeySource::None => "none",
    }
}

/// Models of all providers (§7.1), and the router's model when `routing.json`
/// is usable. A broken file adds nothing here — `routing_status` is where that
/// is said, so the switcher can say why the model is not there.
#[tauri::command]
pub fn models() -> Result<Vec<ModelInfo>, String> {
    let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;
    let mut all = providers.all_models();
    all.extend(crate::routing::offered());
    Ok(all)
}

/// Which of a provider's models are in memory right now (§7.1).
///
/// Asked when the model menu opens, never stored: `yaca_core::provider::models`
/// has the reason. A provider whose server keeps quiet about it answers with
/// an empty map, and the menu stops asking it for the rest of the run.
#[tauri::command]
pub async fn model_residency(provider: String) -> Result<HashMap<String, bool>, String> {
    let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;
    let spec = providers
        .get(&provider)
        .ok_or_else(|| format!("provider \"{provider}\" is not in the list"))?;
    models::residency(spec, models::stored_key(spec).as_deref())
        .await
        .map_err(|e| e.to_string())
}
