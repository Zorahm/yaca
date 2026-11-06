//! Global model choices inherited by projects without explicit overrides.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{CoreError, Settings};

const FILE: &str = "model-defaults.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModelDefaults {
    pub names: std::collections::BTreeMap<String, String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub compact_model: Option<String>,
    pub subagent_model: Option<String>,
}

impl ModelDefaults {
    pub fn load(dir: &Path) -> Result<Self, CoreError> {
        let path = dir.join(FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| CoreError::Config(format!("failed to parse {}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(CoreError::Config(format!(
                "failed to read {}: {e}",
                path.display()
            ))),
        }
    }

    pub fn save(&self, dir: &Path) -> Result<(), CoreError> {
        std::fs::create_dir_all(dir).map_err(|e| CoreError::Config(e.to_string()))?;
        let text =
            serde_json::to_string_pretty(self).map_err(|e| CoreError::Config(e.to_string()))?;
        let temp = dir.join(format!("{FILE}.tmp"));
        std::fs::write(&temp, text).map_err(|e| CoreError::Config(e.to_string()))?;
        std::fs::rename(temp, dir.join(FILE)).map_err(|e| CoreError::Config(e.to_string()))
    }

    pub fn apply(&self, settings: &mut Settings) {
        // A project model and its provider are one choice. Never attach the
        // global provider to a project's explicit model (or the reverse).
        if settings.model.is_none() && settings.provider.is_none() {
            settings.model.clone_from(&self.model);
            settings.provider.clone_from(&self.provider);
        }
        if settings.compact_model.is_none() {
            settings.compact_model.clone_from(&self.compact_model);
        }
        if settings.subagent_model.is_none() {
            settings.subagent_model.clone_from(&self.subagent_model);
        }
    }
}

/// Keep raw project reads separate so saving permissions never materializes
/// inherited model choices into the repository's settings file.
pub fn effective_settings(root: &Path) -> Result<Settings, CoreError> {
    effective_settings_in(root, &crate::provider::registry::config_dir()?)
}

fn effective_settings_in(root: &Path, config: &Path) -> Result<Settings, CoreError> {
    let mut settings = Settings::load(root)?;
    ModelDefaults::load(config)?.apply(&mut settings);
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_overrides_are_independent_and_model_pairs_stay_together() {
        let defaults = ModelDefaults {
            provider: Some("global".into()),
            model: Some("main".into()),
            compact_model: Some("small".into()),
            subagent_model: Some("worker".into()),
            ..ModelDefaults::default()
        };
        let mut settings = Settings::default();
        defaults.apply(&mut settings);
        assert_eq!(settings.model, defaults.model);
        assert_eq!(settings.provider, defaults.provider);
        let mut project = Settings {
            model: Some("local".into()),
            compact_model: Some("own".into()),
            ..Settings::default()
        };
        defaults.apply(&mut project);
        assert_eq!(project.provider, None);
        assert_eq!(project.model.as_deref(), Some("local"));
        assert_eq!(project.compact_model.as_deref(), Some("own"));
        assert_eq!(project.subagent_model, defaults.subagent_model);
    }

    #[test]
    fn defaults_round_trip_and_broken_files_are_reported() {
        let dir = crate::testutil::TempDir::new();
        assert_eq!(
            ModelDefaults::load(dir.path()).unwrap(),
            ModelDefaults::default()
        );
        let defaults = ModelDefaults {
            model: Some("main".into()),
            ..ModelDefaults::default()
        };
        defaults.save(dir.path()).unwrap();
        assert_eq!(ModelDefaults::load(dir.path()).unwrap(), defaults);
        std::fs::write(dir.path().join(FILE), "{").unwrap();
        assert!(ModelDefaults::load(dir.path()).is_err());
    }

    #[test]
    fn inheritance_never_materializes_into_project_files() {
        let root = crate::testutil::TempDir::new();
        let config = crate::testutil::TempDir::new();
        let defaults = ModelDefaults {
            model: Some("global".into()),
            compact_model: Some("small".into()),
            ..ModelDefaults::default()
        };
        defaults.save(config.path()).unwrap();
        let mut raw = Settings::load(root.path()).unwrap();
        assert_eq!(raw.model, None);
        assert_eq!(
            effective_settings_in(root.path(), config.path())
                .unwrap()
                .model,
            defaults.model
        );
        raw.shadow.exclude.push("cache".into());
        raw.save(root.path()).unwrap();
        let loaded = Settings::load(root.path()).unwrap();
        assert_eq!(loaded.model, None);
        assert_eq!(loaded.compact_model, None);
        assert_eq!(loaded.shadow.exclude, vec!["cache"]);
        assert_eq!(
            effective_settings_in(root.path(), config.path())
                .unwrap()
                .compact_model,
            defaults.compact_model
        );
    }
}
