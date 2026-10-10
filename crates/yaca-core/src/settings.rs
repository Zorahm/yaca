//! Project settings: `.yaca/settings.json` (§9, §10).
//!
//! §9 says every project keeps its own things under `.yaca/` — settings,
//! permission rules, history, the shadow repository. This is the first of
//! those.
//!
//! Only what the core actually consults lives here. Appearance, keyboard and
//! the provider list belong to the application rather than to one project, and
//! a field nothing reads is a setting that lies about having an effect.
//!
//! The core reads this file and never writes it. §5.7 is explicit that a rule
//! granted in a hurry does not outlive the hurry: "Always allow" is
//! session-lived, and a rule reaches `settings.json` through the settings
//! screen and nowhere else. Until that screen exists the file is written by
//! hand, which works — it is plain JSON with every field optional.
//!
//! A file that does not parse stops the session. Falling back to defaults would
//! be safe in the narrow sense — fewer rules, less trust — but it would leave a
//! human believing a rule applies when a typo has quietly dropped it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::intent::Mode;
use crate::permission::ScopeKey;

/// Where the file sits under the project root.
pub const FILE: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The mode a session starts in (§8, §10). Absent means Manual — Yaca runs
    /// with any model, weak local ones included, so the safe behaviour is the
    /// one you get without asking for it.
    pub mode: Option<Mode>,
    /// The provider a session starts with (§10).
    pub provider: Option<String>,
    /// The model a session starts with (§10).
    pub model: Option<String>,
    /// The model that summarizes history (§5.11, §10). Absent means the
    /// session's own model.
    pub compact_model: Option<String>,
    /// The model subagents run on (§10). Absent means the session's own model.
    /// Served by the session's provider, like the one above.
    pub subagent_model: Option<String>,
    pub permissions: PermissionSettings,
    pub shadow: ShadowSettings,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PermissionSettings {
    /// Standing rules, in the same shape "Always allow" offers (§5.7). These
    /// are the ones that survive a restart.
    pub rules: Vec<ScopeKey>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ShadowSettings {
    /// Directory names the snapshot walk does not enter, on top of the
    /// built-in list (§10, "исключения теневого репозитория").
    ///
    /// Added to, never replacing: `.git` and `.yaca` are not negotiable, and a
    /// project that dropped them from its list would be snapshotting its own
    /// snapshots.
    pub exclude: Vec<String>,
}

impl Settings {
    /// Reads the file, or returns the defaults when there is none.
    pub fn load(root: &Path) -> Result<Settings, CoreError> {
        let path = crate::project::dir(root).join(FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
            Err(e) => {
                return Err(CoreError::Config(format!(
                    "не прочитать {}: {e}",
                    path.display()
                )))
            }
        };

        serde_json::from_str(&text)
            .map_err(|e| CoreError::Config(format!("{} разобран не до конца: {e}", path.display())))
    }

    /// Writes the file, creating `.yaca/` if needed.
    ///
    /// Nothing in the core calls this: the settings screen will. It lives here
    /// so that the screen does not have to know the shape of the file twice.
    pub fn save(&self, root: &Path) -> Result<(), CoreError> {
        let dir = crate::project::dir(root);
        std::fs::create_dir_all(&dir)
            .map_err(|e| CoreError::Config(format!("не создать {}: {e}", dir.display())))?;
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| CoreError::Config(format!("не собрать настройки: {e}")))?;
        std::fs::write(dir.join(FILE), text)
            .map_err(|e| CoreError::Config(format!("не записать настройки: {e}")))
    }

    pub fn mode_or_default(&self) -> Mode {
        self.mode.unwrap_or_default()
    }

    /// Which model summarizes history, given the session's own model (§5.11).
    pub fn compact_model_or<'a>(&'a self, session_model: &'a str) -> &'a str {
        self.compact_model.as_deref().unwrap_or(session_model)
    }

    /// Which model a subagent runs on, given the session's own model.
    pub fn subagent_model_or<'a>(&'a self, session_model: &'a str) -> &'a str {
        self.subagent_model.as_deref().unwrap_or(session_model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn a_project_without_a_file_gets_the_defaults() {
        let dir = TempDir::new();
        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.mode_or_default(), Mode::Manual);
    }

    #[test]
    fn a_half_written_file_still_loads() {
        let dir = TempDir::new();
        dir.write(".yaca/settings.json", r#"{ "mode": "auto" }"#);

        // Every field is optional, so a hand-written file may name one thing.
        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.mode, Some(Mode::Auto));
        assert!(settings.permissions.rules.is_empty());
    }

    #[test]
    fn a_broken_file_stops_the_session_instead_of_being_ignored() {
        let dir = TempDir::new();
        dir.write(".yaca/settings.json", "{ mode: auto }");

        // Silently defaulting would leave a human believing a rule applies
        // when a typo has dropped it.
        let err = Settings::load(dir.path()).unwrap_err();
        assert!(matches!(err, CoreError::Config(_)), "got {err:?}");
    }

    #[test]
    fn standing_rules_round_trip() {
        let dir = TempDir::new();
        let written = Settings {
            mode: Some(Mode::Edits),
            permissions: PermissionSettings {
                rules: vec![
                    ScopeKey::CommandPrefix {
                        prefix: "cargo".into(),
                    },
                    ScopeKey::Directory { path: "src".into() },
                ],
            },
            shadow: ShadowSettings {
                exclude: vec!["fixtures".into()],
            },
            ..Settings::default()
        };
        written.save(dir.path()).unwrap();

        assert_eq!(Settings::load(dir.path()).unwrap(), written);
    }

    #[test]
    fn the_summarizing_model_falls_back_to_the_session_model() {
        let mut settings = Settings::default();
        assert_eq!(settings.compact_model_or("opus"), "opus");
        settings.compact_model = Some("haiku".into());
        assert_eq!(settings.compact_model_or("opus"), "haiku");
    }
}
