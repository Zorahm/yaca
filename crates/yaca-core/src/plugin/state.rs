//! What decides which plugins exist and which are on.
//!
//! Yaca's switches are its own file, `<config dir>/plugins.json`. A plugin it
//! has no switch for takes its default from where it came from: what Claude
//! Code says for a plugin Claude Code installed, on for one put in Yaca's own
//! directory. Claude Code's files are read and never written.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::PluginDiagnostic;
use crate::error::CoreError;
use crate::workspace::roots::plain;

const SWITCHES: &str = "plugins.json";

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct Switches {
    #[serde(default)]
    enabled: BTreeMap<String, bool>,
}

impl Switches {
    /// A file that is missing or broken reads as no switches: every plugin
    /// then takes its default, which is the state before anything was set.
    pub(super) fn load(config_dir: &Path) -> Self {
        std::fs::read_to_string(config_dir.join(SWITCHES))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub(super) fn get(&self, id: &str) -> Option<bool> {
        self.enabled.get(id).copied()
    }

    pub(super) fn set(&mut self, id: &str, on: bool) {
        self.enabled.insert(id.to_string(), on);
    }

    pub(super) fn forget(&mut self, id: &str) {
        self.enabled.remove(id);
    }

    pub(super) fn save(&self, config_dir: &Path) -> Result<(), CoreError> {
        std::fs::create_dir_all(config_dir)
            .map_err(|e| CoreError::Config(format!("не создать {}: {e}", config_dir.display())))?;
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| CoreError::Config(format!("не собрать список плагинов: {e}")))?;
        std::fs::write(config_dir.join(SWITCHES), text)
            .map_err(|e| CoreError::Config(format!("не записать список плагинов: {e}")))
    }
}

/// The directories under Yaca's own plugin directory, in name order.
pub(super) fn own_plugins(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = read
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

/// One plugin Claude Code installed for the human.
pub(super) struct Install {
    pub id: String,
    pub path: PathBuf,
}

/// `plugins/installed_plugins.json`, the installs of `scope: user` only: a
/// project or local install belongs to one project, and project plugins are not
/// something Yaca loads yet. In id order.
pub(super) fn claude_installs(home: &Path, diagnostics: &mut Vec<PluginDiagnostic>) -> Vec<Install> {
    let file = home.join("plugins").join("installed_plugins.json");
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Vec::new();
    };
    let Ok(Value::Object(doc)) = serde_json::from_str::<Value>(&text) else {
        diagnostics.push(PluginDiagnostic {
            path: plain(&file),
            message: "не разбирается, плагины Claude Code не загружены".into(),
        });
        return Vec::new();
    };
    let Some(Value::Object(plugins)) = doc.get("plugins") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (id, entry) in plugins {
        // Version 2 keeps a list of installs per plugin; version 1 kept one.
        let installs: Vec<&Value> = match entry {
            Value::Array(list) => list.iter().collect(),
            single @ Value::Object(_) => vec![single],
            _ => continue,
        };
        let user = installs.into_iter().find(|i| {
            i.get("scope").and_then(Value::as_str).is_none_or(|s| s == "user")
        });
        let Some(path) = user.and_then(|i| i.get("installPath")).and_then(Value::as_str) else {
            continue;
        };
        out.push(Install {
            id: id.clone(),
            path: PathBuf::from(path),
        });
    }
    out
}

/// `enabledPlugins` of Claude Code's own settings.
pub(super) fn claude_enabled(home: &Path) -> BTreeMap<String, bool> {
    std::fs::read_to_string(home.join("settings.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|doc| doc.get("enabledPlugins").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}
