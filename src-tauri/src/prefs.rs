//! What the human chose about the application itself (§10).
//!
//! Application level, not project level, and for the same reason the provider
//! list is: a project does not get to decide what the window looks like or
//! whether it may raise a toast. It rides in the shell's own `settings.json`
//! next to the remembered projects — see [`crate::settings`].
//!
//! Nothing here reaches the core. Theme, gutter and line height are
//! three attributes on `<html>` and nothing more (`src/ui/tokens.css`); the
//! language picks a dictionary; notifications are read by [`crate::notify`],
//! which is shell code too. The core stays headless.
//!
//! Every field has a default, so a file written by an older build means
//! "the defaults" rather than "everything off". The theme's default is Paper:
//! the window is drawn as a proof on paper (§2), and Night is the edition one
//! asks for.
//!
//! There is no accent to choose. The window has no accent role: the main
//! button is ink and red marks only what needs the human's eye, so a picker
//! would have had nothing to recolour. An `accent` key left in a file by an
//! earlier build is ignored on load and gone on the next save.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use yaca_core::ShellChoice;

/// Which of the two themes, or the one the OS asks for (§2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    Night,
    /// The default (§2).
    #[default]
    Paper,
    System,
}

/// Interface language (§14), or the one the OS asks for.
///
/// Spelled the way the dictionaries are keyed, so the window can hand the
/// value straight to `src/i18n` instead of translating between two spellings
/// of the same choices.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Ru,
    En,
    /// Simplified Chinese.
    Zh,
    Es,
    Ja,
    Ko,
    De,
    Fr,
    /// Brazilian Portuguese.
    Pt,
    /// Ask the webview, which is what happened before this setting existed.
    #[default]
    System,
}

/// The whole of what §10's "Appearance" and "Notifications and language" hold.
///
/// One flat struct rather than one per section: the sections are a way of
/// laying out a window, and the file should not have to be rearranged when
/// the window is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub theme: Theme,
    /// The diff's gutter: 52 px with numbers, 14 without (§2, §6.3).
    pub line_numbers: bool,
    /// Diff line height 20 px instead of 22 (§0, §6.3).
    pub compact: bool,
    pub language: Language,
    /// An OS toast when the agent asks for permission and the window is not
    /// focused (§15). On by default: the question blocks its own tool call
    /// and waits without a timeout, so a human looking elsewhere has to be
    /// told.
    pub notifications: bool,
    /// Honour `Disallow` in a site's `robots.txt` when fetching a page (§10).
    ///
    /// On by default, and off is a real choice rather than a loophole: a
    /// staging site of one's own is often closed with `Disallow: /` merely to
    /// keep it out of search results, and refusing to read one's own page
    /// would be the tool being pious rather than useful.
    ///
    /// Read by the core, not by the shell: it rides in on
    /// `SessionConfig::respect_robots`, the same road as the mode and the
    /// effort. The core stays headless — it is handed a flag, not a settings
    /// file it has no business reading.
    pub respect_robots: bool,
    /// Which shell the agent's commands run through (§10). Windows only:
    /// elsewhere there is one shell and nothing to choose.
    ///
    /// Read by the core on the same road as the flag above
    /// (`SessionConfig::shell`), and settled for a session when it opens: the
    /// system prompt names the shell, so a chat that is already open keeps
    /// the one it started with.
    pub shell: ShellChoice,
    /// Look for a newer release on GitHub on its own: shortly after start and
    /// every few hours after (see [`crate::update`]). Off still leaves the
    /// button in the About screen; it only stops the app from asking by itself.
    pub check_updates: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Preferences {
            theme: Theme::default(),
            line_numbers: true,
            compact: false,
            language: Language::default(),
            notifications: true,
            respect_robots: true,
            shell: ShellChoice::default(),
            check_updates: true,
        }
    }
}

/// The channel preferences arrive on. One name for both windows: whoever
/// changed them, everyone who draws them has to hear.
pub const PREFS_CHANNEL: &str = "yaca://preferences";

/// Preferences as they are now.
///
/// Read from the shell's own state rather than the file: the file is written
/// through [`set_preferences`], which refreshes that state in the same call,
/// and a second reader of the disk would be a second answer to the same
/// question.
#[tauri::command]
pub fn preferences(state: tauri::State<'_, crate::state::AppState>) -> Preferences {
    state.prefs()
}

/// Writes preferences and tells every window at once.
///
/// The broadcast is the point of the command, not a courtesy: §10 is a
/// separate window, and a theme that changed only where it was chosen would be
/// an interface lying about state (§0). `emit` reaches every webview,
/// the settings window included, so both redraw from the same value.
///
/// The state is updated before the write. A failed write is reported, and what
/// the human sees is what they picked — a window that snapped back to the old
/// theme while the error message said "could not write" would be telling two
/// stories at once.
#[tauri::command]
pub fn set_preferences(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    prefs: Preferences,
) -> Result<(), String> {
    use tauri::Emitter;

    state.set_prefs(prefs.clone());
    let _ = app.emit(PREFS_CHANNEL, prefs.clone());

    let dir = crate::config_dir()?;
    let mut settings = crate::settings::AppSettings::load(&dir);
    settings.set_prefs(prefs);
    settings
        .save(&dir)
        .map_err(|e| format!("could not write settings: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_paper_and_everything_on() {
        let prefs = Preferences::default();
        assert_eq!(prefs.theme, Theme::Paper);
        assert!(prefs.line_numbers);
        assert!(!prefs.compact);
        assert!(prefs.notifications);
        assert!(prefs.respect_robots);
        assert_eq!(prefs.shell, ShellChoice::Auto);
        assert!(prefs.check_updates);
    }

    /// A file from a build that knew fewer fields keeps the rest as they were.
    #[test]
    fn a_partial_file_fills_the_rest_with_the_defaults() {
        let prefs: Preferences = serde_json::from_str(r#"{ "theme": "night" }"#).unwrap();
        assert_eq!(prefs.theme, Theme::Night);
        assert!(prefs.line_numbers, "an unnamed field is not an off field");
        assert!(prefs.notifications);
        assert!(prefs.respect_robots);
    }

    /// A file from the build that had an accent picker still loads: the key
    /// is dropped, the rest stands.
    #[test]
    fn an_accent_left_by_an_earlier_build_is_ignored() {
        let prefs: Preferences =
            serde_json::from_str(r#"{ "theme": "night", "accent": "teal" }"#).unwrap();
        assert_eq!(prefs.theme, Theme::Night);
        assert!(prefs.line_numbers);
    }

    /// The language is spelled the way `src/i18n` keys its dictionaries.
    #[test]
    fn the_language_is_spelled_like_the_dictionaries() {
        let json = serde_json::to_string(&Language::Ru).unwrap();
        assert_eq!(json, r#""ru""#);
        assert_eq!(
            serde_json::from_str::<Language>(r#""en""#).unwrap(),
            Language::En
        );
    }
}
