//! Providers and their keys (§10), and the models the bottom row offers (§7.1).

use std::collections::HashMap;

use yaca_core::provider::models;
use yaca_core::provider::registry::{ProviderSpec, Providers};
use yaca_core::secrets::{self, KeySource};
use yaca_core::usage::ModelInfo;

use crate::config_dir;

/// Список провайдеров (§10).
#[tauri::command]
pub fn providers() -> Result<Providers, String> {
    Providers::load(&config_dir()?).map_err(|e| e.to_string())
}

/// Добавляет провайдера или заменяет одноимённого.
#[tauri::command]
pub fn save_provider(spec: ProviderSpec) -> Result<Providers, String> {
    let dir = config_dir()?;
    let mut providers = Providers::load(&dir).map_err(|e| e.to_string())?;
    providers.put(spec);
    providers.save(&dir).map_err(|e| e.to_string())?;
    Ok(providers)
}

#[tauri::command]
pub fn delete_provider(id: String) -> Result<Providers, String> {
    let dir = config_dir()?;
    let mut providers = Providers::load(&dir).map_err(|e| e.to_string())?;
    providers.remove(&id).map_err(|e| e.to_string())?;
    providers.save(&dir).map_err(|e| e.to_string())?;
    Ok(providers)
}

/// Спросить у провайдера, какие модели он обслуживает (§10).
///
/// Берёт описание из формы, а не из реестра: адрес и заголовки могли только
/// что поправить, а спрашивать надо у того, кого настраивают. Ключ — из
/// кейчейна под тем же id; провайдеру, который ключа не просит, он и не нужен.
///
/// Список возвращается, а не сохраняется: пока человек не нажал «Сохранить»,
/// в реестре ничего меняться не должно.
#[tauri::command]
pub async fn discover_models(spec: ProviderSpec) -> Result<Vec<ModelInfo>, String> {
    models::discover(&spec, key_for(&spec).as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// §10: ключ уезжает в системный кейчейн, а не в файл и не в память процесса.
///
/// Провайдер называется всегда: угаданный записал бы ключ не под тем именем.
#[tauri::command]
pub fn set_api_key(provider: String, key: String) -> Result<(), String> {
    secrets::store_api_key(&provider, &key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn forget_api_key(provider: String) -> Result<(), String> {
    secrets::forget_api_key(&provider).map_err(|e| e.to_string())
}

/// Откуда взялся ключ: кейчейн, переменная окружения или ниоткуда.
///
/// Не сам ключ. Экрану настроек нужно показать, что ключ есть и откуда он, —
/// а ключ, уехавший в вебвью, оттуда уже не вернёшь.
///
/// Провайдер, которому ключ не нужен вовсе, отвечает `not_needed`: «ключа нет»
/// у локального шлюза было бы предупреждением ни о чём.
#[tauri::command]
pub fn api_key_source(provider: String) -> Result<&'static str, String> {
    let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;
    let spec = providers
        .get(&provider)
        .ok_or_else(|| format!("провайдера «{provider}» нет в списке"))?;
    if !spec.needs_key() {
        return Ok("not_needed");
    }
    Ok(match secrets::key_source(&spec.id) {
        KeySource::Keychain => "keychain",
        KeySource::Environment => "environment",
        KeySource::None => "none",
    })
}

/// Модели всех провайдеров (§7.1).
#[tauri::command]
pub fn models() -> Result<Vec<ModelInfo>, String> {
    let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;
    Ok(providers.all_models())
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
        .ok_or_else(|| format!("провайдера «{provider}» нет в списке"))?;
    models::residency(spec, key_for(spec).as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// The key a request to this provider goes with: none for one that does not
/// ask for it.
fn key_for(spec: &ProviderSpec) -> Option<String> {
    spec.needs_key()
        .then(|| secrets::api_key(&spec.id).0)
        .flatten()
}
