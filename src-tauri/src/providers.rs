//! Providers and their keys (§10), and the models the bottom row offers (§7.1).

use std::collections::HashMap;

use yaca_core::provider::models;
use yaca_core::provider::registry::{ProviderSpec, Providers};
use yaca_core::secrets::{self, KeySource};
use yaca_core::usage::ModelInfo;

use crate::config_dir;

/// Канал «список провайдеров изменился».
///
/// Правят их в окне настроек §10, а показывает их переключатель моделей §7.1
/// в главном окне. Без этого канала список там остался бы прежним до
/// перезапуска — то есть окно врало бы про состояние (§0).
pub const PROVIDERS_CHANNEL: &str = "yaca://providers";

/// Говорит окнам, что список изменился. Ошибка рассылки не роняет запись:
/// на диске уже правда, а недоехавшее событие — это устаревший список, а не
/// потерянная настройка.
fn announce(app: &tauri::AppHandle) {
    use tauri::Emitter;
    let _ = app.emit(PROVIDERS_CHANNEL, ());
}

/// Список провайдеров (§10).
#[tauri::command]
pub fn providers() -> Result<Providers, String> {
    Providers::load(&config_dir()?).map_err(|e| e.to_string())
}

/// Добавляет провайдера или заменяет одноимённого.
#[tauri::command]
pub fn save_provider(app: tauri::AppHandle, spec: ProviderSpec) -> Result<Providers, String> {
    let dir = config_dir()?;
    let mut providers = Providers::load(&dir).map_err(|e| e.to_string())?;
    providers.put(spec);
    providers.save(&dir).map_err(|e| e.to_string())?;
    announce(&app);
    Ok(providers)
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

/// Откуда взялся ключ веб-поиска (§10).
///
/// Отдельная команда, а не `api_key_source("tavily")`: Tavily не провайдер,
/// в реестре его нет, и общая команда ответила бы «нет в списке» про ключ,
/// который преспокойно лежит в кейчейне. Записывается и забывается он теми же
/// `set_api_key` и `forget_api_key` — хранилище одно на всех.
#[tauri::command]
pub fn search_key_source() -> &'static str {
    match secrets::key_source(secrets::TAVILY_ITEM) {
        KeySource::Keychain => "keychain",
        KeySource::Environment => "environment",
        KeySource::None => "none",
    }
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
