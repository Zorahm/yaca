//! Settings that outlive any one project (§10).
//!
//! Not `.yaca/settings.json`. That file lives inside a project and the core
//! reads it; this one answers questions asked before any project is known —
//! which projects the window knows about, what the window looks like — so it
//! lives in the OS config directory instead. The provider list is at this
//! level too, in its own file, because the core reads that one (§10).
//!
//! A file that does not parse is dropped rather than fatal, and that is the
//! opposite of what the core does with project settings. The core stops a
//! session on a broken `.yaca/settings.json` because a typo there silently
//! cancels a permission rule the human believes is in force. Nothing here is
//! load-bearing: losing this file costs a list of remembered paths and a
//! theme, and both are one click to set again.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::prefs::Preferences;

const FILE: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    /// Projects the sidebar knows about, most recently opened first.
    projects: Vec<PathBuf>,
    /// What the field above used to be: one remembered project, back when the
    /// shell held one at a time. Read once on load and folded into the list,
    /// never written again — an update should not cost the human the project
    /// they were working in.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_project: Option<PathBuf>,
    /// Theme, density, language, notifications (§10).
    prefs: Preferences,
}

impl AppSettings {
    pub fn load(dir: &Path) -> Self {
        let mut settings: AppSettings = fs::read_to_string(dir.join(FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        if let Some(one) = settings.last_project.take() {
            settings.remember_project(one);
        }
        settings
    }

    /// The remembered projects that are still on disk, freshest first.
    ///
    /// A path that vanished is not a project to reopen: the sidebar would name
    /// a folder that no longer answers, which is the interface lying about
    /// state (§0). Forgetting it is cheaper than explaining it.
    pub fn projects(&self) -> Vec<&Path> {
        self.projects
            .iter()
            .map(PathBuf::as_path)
            .filter(|path| path.is_dir())
            .collect()
    }

    /// Puts the project first, which is also how it is added.
    ///
    /// Order is «last opened», not «first added»: the list is read top down
    /// and what was worked on today belongs at the top of it.
    pub fn remember_project(&mut self, path: PathBuf) {
        self.projects.retain(|known| known != &path);
        self.projects.insert(0, path);
    }

    /// Drops the project from the list. The folder itself is not touched:
    /// forgetting a project is a window action, not a change to the disk.
    pub fn forget_project(&mut self, path: &Path) {
        self.projects.retain(|known| known != path);
    }

    pub fn prefs(&self) -> &Preferences {
        &self.prefs
    }

    pub fn set_prefs(&mut self, prefs: Preferences) {
        self.prefs = prefs;
    }

    /// Written through a temporary file: a half-written settings file is a
    /// file that does not parse, and the next launch would forget the list.
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        let temp = dir.join(format!("{FILE}.tmp"));
        fs::write(&temp, text)?;
        fs::rename(temp, dir.join(FILE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU32, Ordering};

    /// Own temp directory rather than a crate for it, as in the core's tests.
    /// The counter is what makes the name unique: tests run in parallel and a
    /// Windows clock is coarse enough to hand two of them the same instant.
    fn temp_dir() -> PathBuf {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("yaca-settings-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn project(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn missing_file_is_defaults() {
        let dir = temp_dir();
        assert_eq!(AppSettings::load(&dir), AppSettings::default());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn broken_file_is_defaults() {
        let dir = temp_dir();
        fs::write(dir.join(FILE), "{ not json").unwrap();
        assert_eq!(AppSettings::load(&dir), AppSettings::default());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn remembered_projects_survive_a_save_freshest_first() {
        let dir = temp_dir();
        let one = project(&dir, "one");
        let two = project(&dir, "two");

        let mut settings = AppSettings::default();
        settings.remember_project(one.clone());
        settings.remember_project(two.clone());
        settings.save(&dir).unwrap();

        let loaded = AppSettings::load(&dir);
        assert_eq!(loaded.projects(), vec![two.as_path(), one.as_path()]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reopening_a_known_project_moves_it_up_without_doubling_it() {
        let dir = temp_dir();
        let one = project(&dir, "one");
        let two = project(&dir, "two");

        let mut settings = AppSettings::default();
        settings.remember_project(one.clone());
        settings.remember_project(two.clone());
        settings.remember_project(one.clone());

        assert_eq!(settings.projects(), vec![one.as_path(), two.as_path()]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn vanished_project_is_forgotten() {
        let dir = temp_dir();
        let mut settings = AppSettings::default();
        settings.remember_project(dir.join("gone"));
        assert!(settings.projects().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn forgetting_takes_out_one_and_leaves_the_rest() {
        let dir = temp_dir();
        let one = project(&dir, "one");
        let two = project(&dir, "two");

        let mut settings = AppSettings::default();
        settings.remember_project(one.clone());
        settings.remember_project(two.clone());
        settings.forget_project(&two);

        assert_eq!(settings.projects(), vec![one.as_path()]);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Preferences survive a save, and a file written before they existed
    /// loads as the defaults rather than as everything switched off.
    #[test]
    fn preferences_survive_a_save_and_an_older_file() {
        use crate::prefs::Theme;

        let dir = temp_dir();
        let mut settings = AppSettings::load(&dir);
        assert_eq!(settings.prefs(), &Preferences::default());

        let prefs = Preferences {
            theme: Theme::Night,
            notifications: false,
            ..Preferences::default()
        };
        settings.set_prefs(prefs.clone());
        settings.save(&dir).unwrap();

        assert_eq!(AppSettings::load(&dir).prefs(), &prefs);

        // A file from before the field: the list survives, the look defaults.
        fs::write(dir.join(FILE), r#"{ "projects": [] }"#).unwrap();
        assert_eq!(AppSettings::load(&dir).prefs(), &Preferences::default());

        fs::remove_dir_all(&dir).unwrap();
    }

    /// The one project of the previous version arrives as the first of the
    /// list, and leaves the file on the next save.
    #[test]
    fn a_single_remembered_project_migrates_into_the_list() {
        let dir = temp_dir();
        let one = project(&dir, "one");
        let json = serde_json::json!({ "lastProject": one });
        fs::write(dir.join(FILE), json.to_string()).unwrap();

        let settings = AppSettings::load(&dir);
        assert_eq!(settings.projects(), vec![one.as_path()]);

        settings.save(&dir).unwrap();
        let text = fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(!text.contains("lastProject"), "{text}");
        fs::remove_dir_all(&dir).unwrap();
    }
}
