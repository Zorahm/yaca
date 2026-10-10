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
//! The core reads this file and never writes it on its own. §5.7 is explicit
//! that a rule granted in a hurry does not outlive the hurry: "Always allow"
//! and "Always deny" are session-lived, and a rule reaches `settings.json`
//! through the settings window (§10) and nowhere else. [`Settings::save`] is
//! what that window calls, so the shape of the file is written down once.
//! Editing it by hand still works — plain JSON with every field optional.
//!
//! A file that does not parse stops the session. Falling back to defaults would
//! be safe in the narrow sense — fewer rules, less trust — but it would leave a
//! human believing a rule applies when a typo has quietly dropped it.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::CoreError;
use crate::intent::Mode;
use crate::permission::{Refusal, ScopeKey};

/// Where the file sits under the project root.
pub const FILE: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
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
    pub permissions: PermissionSettings,
    pub shadow: ShadowSettings,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct PermissionSettings {
    /// Standing rules, in the same shape "Always allow" offers (§5.7). These
    /// are the ones that survive a restart.
    pub rules: Vec<ScopeKey>,
    /// Standing refusals, in the shape "Always deny" offers. Kept apart from
    /// `rules` rather than folded into one list with a sign: a refusal carries
    /// a reason and a grant does not, and the two lists answer different
    /// questions — "what happens without me" and "what never happens".
    ///
    /// A refusal outranks a grant that covers the same call, and it is
    /// consulted before the mode, so bypass means "stop asking me", not "do
    /// what I said no to". That order lives in
    /// [`crate::permission::broker::Broker::standing`]; the file only decides
    /// which rules are in play.
    pub denies: Vec<Refusal>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
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
    /// Nothing in the core calls this — the settings window does (§10). It
    /// lives here so that the window does not have to know the shape of the
    /// file a second time.
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
                denies: vec![
                    Refusal::new(
                        ScopeKey::CommandPrefix {
                            prefix: "git commit".into(),
                        },
                        Some("коммит я делаю сам".into()),
                    ),
                    Refusal::new(ScopeKey::Directory { path: ".yaca".into() }, None),
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

    /// A refusal with no reason survives the round trip, and one written by
    /// hand without the key at all parses too — the file is edited both ways.
    #[test]
    fn a_refusal_without_a_reason_round_trips() {
        let dir = TempDir::new();
        let written = Settings {
            permissions: PermissionSettings {
                // The canonical tool name: reading a `ScopeKey::Tool` runs it
                // through the legacy rename table, so a file naming the old
                // spelling comes back as the new one.
                denies: vec![Refusal::new(
                    ScopeKey::Tool {
                        name: "WebFetch".into(),
                    },
                    None,
                )],
                ..PermissionSettings::default()
            },
            ..Settings::default()
        };
        written.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()).unwrap(), written);

        // And the same refusal written by hand, with no `reason` key at all.
        dir.write(
            ".yaca/settings.json",
            r#"{ "permissions": { "denies": [{ "scope": { "type": "tool", "name": "WebFetch" } }] } }"#,
        );
        assert_eq!(Settings::load(dir.path()).unwrap(), written);
    }

    /// A file written before refusals existed loads with none of them, rather
    /// than refusing to parse.
    #[test]
    fn a_file_without_denies_still_loads() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "permissions": { "rules": [{ "type": "directory", "path": "src" }] } }"#,
        );

        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.permissions.rules.len(), 1);
        assert!(settings.permissions.denies.is_empty());
    }

    #[test]
    fn the_summarizing_model_falls_back_to_the_session_model() {
        let mut settings = Settings::default();
        assert_eq!(settings.compact_model_or("opus"), "opus");
        settings.compact_model = Some("haiku".into());
        assert_eq!(settings.compact_model_or("opus"), "haiku");
    }
}
