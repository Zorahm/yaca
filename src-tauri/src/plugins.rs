//! Plugin review and copy selection for the settings screen.
use yaca_core::plugin::{self, install::PluginPreview, PluginReport, PluginSummary};

#[tauri::command]
pub(crate) async fn preview_plugin(id: String) -> Result<PluginPreview, String> {
    super::installing(move |i| i.preview(&id)).await
}

#[tauri::command]
pub(crate) async fn install_reviewed_plugin(token: String) -> Result<PluginSummary, String> {
    super::installing(move |i| i.install_preview(&token)).await
}

#[tauri::command]
pub(crate) async fn discard_plugin_preview(token: String) -> Result<(), String> {
    super::installing(move |i| i.discard_preview(&token)).await
}

#[tauri::command]
pub(crate) fn prefer_plugin_copy(name: String, path: String) -> Result<PluginReport, String> {
    let config = super::config_dir()?;
    plugin::prefer_copy(&config, &name, &path).map_err(|e| e.to_string())?;
    Ok(plugin::PluginSources::standard(&config).load().report())
}

#[tauri::command]
pub(crate) fn ignore_plugin_duplicates(
    name: String,
    ignored: bool,
) -> Result<PluginReport, String> {
    let config = super::config_dir()?;
    plugin::ignore_duplicates(&config, &name, ignored).map_err(|e| e.to_string())?;
    Ok(plugin::PluginSources::standard(&config).load().report())
}

#[tauri::command]
pub(crate) fn open_plugins_folder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = super::config_dir()?.join(plugin::DIR);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| e.to_string())
}
